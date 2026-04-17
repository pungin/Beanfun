//! AppSettings XML reader / writer + IO-bearing async APIs.
//!
//! Wire format mirrors .NET `ConfigurationManager`'s
//! `<configuration><appSettings><add key value /></appSettings></configuration>`
//! schema, preserving insertion order via [`IndexMap`] for byte-for-
//! byte round-trip with WPF-written `Config.xml` files.

use indexmap::IndexMap;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::io::Cursor;
use std::path::Path;

#[cfg(target_os = "windows")]
use std::path::PathBuf;

use crate::services::config::error::ConfigError;

const CONFIG_ROOT: &str = "configuration";
const APP_SETTINGS: &str = "appSettings";
const ADD_ELEMENT: &str = "add";
const ATTR_KEY: &str = "key";
const ATTR_VALUE: &str = "value";

/// Parse a `Config.xml` document into an ordered [`IndexMap`] of
/// `<add key value />` entries.
///
/// - The reader is **lenient**: anything outside
///   `<configuration><appSettings>` (including unknown sibling
///   sections like `<startup>` or `<connectionStrings>`) is silently
///   skipped to match .NET `ConfigurationManager`'s section-scoped
///   behaviour.
/// - Empty input parses to an empty map (not an error), matching the
///   .NET behaviour for newly-created config files.
/// - Returns [`ConfigError::XmlParse`] for truly malformed XML
///   (mismatched tags, invalid attribute syntax, broken declaration).
/// - Insertion order is preserved exactly as it appears on disk.
pub fn parse_app_settings(xml: &str) -> Result<IndexMap<String, String>, ConfigError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut map = IndexMap::new();
    let mut in_configuration = false;
    let mut in_app_settings = false;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(ConfigError::XmlParse)?;
        match event {
            Event::Eof => break,

            Event::Start(e) => {
                let name = local_name_string(e.local_name().as_ref());
                match name.as_str() {
                    CONFIG_ROOT if !in_configuration => in_configuration = true,
                    APP_SETTINGS if in_configuration && !in_app_settings => {
                        in_app_settings = true;
                    }
                    _ => {
                        // Unknown nested element — skip its subtree
                        // entirely so that whatever XML tags appear
                        // there cannot poison the map.
                        let end_owned = e.to_end().into_owned();
                        reader
                            .read_to_end_into(end_owned.name(), &mut buf)
                            .map_err(ConfigError::XmlParse)?;
                    }
                }
            }

            Event::Empty(e) => {
                let name = local_name_string(e.local_name().as_ref());
                if name == ADD_ELEMENT && in_app_settings {
                    let mut key = None;
                    let mut value = None;
                    for attr in e.attributes() {
                        let attr =
                            attr.map_err(|err| ConfigError::XmlParse(quick_xml::Error::from(err)))?;
                        let attr_name = local_name_string(attr.key.local_name().as_ref());
                        let attr_value = attr
                            .unescape_value()
                            .map_err(ConfigError::XmlParse)?
                            .into_owned();
                        match attr_name.as_str() {
                            ATTR_KEY => key = Some(attr_value),
                            ATTR_VALUE => value = Some(attr_value),
                            _ => {}
                        }
                    }
                    if let (Some(k), Some(v)) = (key, value) {
                        // IndexMap::insert is in-place update for
                        // existing keys, append for new ones — this
                        // matches .NET `Settings.Add` semantics for
                        // duplicate keys (last write wins, position
                        // of first occurrence preserved).
                        map.insert(k, v);
                    }
                }
            }

            Event::End(e) => {
                let name = local_name_string(e.local_name().as_ref());
                match name.as_str() {
                    APP_SETTINGS => in_app_settings = false,
                    CONFIG_ROOT => in_configuration = false,
                    _ => {}
                }
            }

            _ => {}
        }
        buf.clear();
    }

    Ok(map)
}

fn local_name_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Serialize `map` into the .NET-compatible
/// `<configuration><appSettings>` schema with a leading
/// `<?xml version="1.0" encoding="utf-8"?>` declaration and
/// 2-space indentation.
///
/// XML attribute values are escaped automatically by quick-xml
/// (`<` `>` `&` `"` `'` are all replaced with their entity forms),
/// so callers can pass arbitrary `String` values without pre-
/// encoding.
///
/// Returns [`ConfigError::XmlWrite`] only on truly unreachable
/// failures — the underlying `Cursor<Vec<u8>>` writes never fail.
pub fn serialize_app_settings(map: &IndexMap<String, String>) -> Result<String, ConfigError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(ConfigError::XmlWrite)?;
    writer
        .write_event(Event::Start(BytesStart::new(CONFIG_ROOT)))
        .map_err(ConfigError::XmlWrite)?;

    if map.is_empty() {
        // Match .NET ConfigurationManager's self-closing form for an
        // empty section (`<appSettings />`) rather than quick-xml's
        // default open + close pair (`<appSettings>\n  </appSettings>`).
        // Both are valid XML and parse-equivalent, but the self-closing
        // shape is what WPF actually writes, keeping diffs against
        // upstream-produced files clean.
        writer
            .write_event(Event::Empty(BytesStart::new(APP_SETTINGS)))
            .map_err(ConfigError::XmlWrite)?;
    } else {
        writer
            .write_event(Event::Start(BytesStart::new(APP_SETTINGS)))
            .map_err(ConfigError::XmlWrite)?;
        for (k, v) in map {
            let mut elem = BytesStart::new(ADD_ELEMENT);
            elem.push_attribute((ATTR_KEY, k.as_str()));
            elem.push_attribute((ATTR_VALUE, v.as_str()));
            writer
                .write_event(Event::Empty(elem))
                .map_err(ConfigError::XmlWrite)?;
        }
        writer
            .write_event(Event::End(BytesEnd::new(APP_SETTINGS)))
            .map_err(ConfigError::XmlWrite)?;
    }

    writer
        .write_event(Event::End(BytesEnd::new(CONFIG_ROOT)))
        .map_err(ConfigError::XmlWrite)?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })
}

// =================================================================
// IO-bearing async APIs
// =================================================================

/// Read `key` from `path` and return its value, falling back to `""`.
///
/// Catch-all sibling of [`get_value_or`]; matches WPF
/// `ConfigAppSettings.GetValue(key)` at
/// `Beanfun/Helper/ConfigAppSettings.cs` L64-67. Any failure in the
/// read/parse pipeline (file missing, IO error, malformed XML,
/// non-UTF-8 bytes, key missing) yields `""`. Errors are logged at
/// `WARN` level via [`tracing`].
pub async fn get_value(path: &Path, key: &str) -> String {
    get_value_or(path, key, "").await
}

/// Read `key` from `path` and return its value, falling back to
/// `default`.
///
/// Matches WPF `ConfigAppSettings.GetValue(key, def)` at
/// `Beanfun/Helper/ConfigAppSettings.cs` L69-93: any failure in the
/// read/parse pipeline is logged at `WARN` level and `default` is
/// returned — there is no typed error surface (the deviation called
/// out in [`crate::services::config`] only applies to `set_value`).
pub async fn get_value_or(path: &Path, key: &str, default: &str) -> String {
    let path_owned = path.to_owned();
    let key_owned = key.to_owned();
    let default_owned = default.to_owned();
    let key_for_log = key.to_owned();

    let result = spawn_blocking_config(move || read_value_blocking(&path_owned, &key_owned)).await;
    match result {
        Ok(Some(v)) => v,
        Ok(None) => default_owned,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                key = %key_for_log,
                "config get_value failed; returning default"
            );
            default_owned
        }
    }
}

fn read_value_blocking(path: &Path, key: &str) -> Result<Option<String>, ConfigError> {
    let map = read_map_blocking(path)?;
    Ok(map.get(key).cloned())
}

fn read_map_blocking(path: &Path) -> Result<IndexMap<String, String>, ConfigError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File-missing is the expected first-time-run signal,
            // matching .NET `ConfigurationManager` which silently
            // returns an empty `Settings` collection when the file
            // does not exist yet.
            return Ok(IndexMap::new());
        }
        Err(e) => return Err(ConfigError::Io(e)),
    };
    let xml = std::str::from_utf8(&bytes).map_err(|_| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "config file is not valid UTF-8",
        ))
    })?;
    parse_app_settings(xml)
}

/// Set / update / remove `key` in the AppSettings store at `path`.
///
/// Mirrors the four-way truth table of WPF
/// `ConfigAppSettings.SetValue` at
/// `Beanfun/Helper/ConfigAppSettings.cs` L21-32:
///
/// | existing | value      | action                                     |
/// |----------|------------|--------------------------------------------|
/// | absent   | `None`     | no-op (no file write)                      |
/// | present  | `None`     | remove key (preserves remaining order)     |
/// | absent   | `Some(v)`  | append at end                              |
/// | present  | `Some(v)`  | update in place (preserves slot position)  |
///
/// # Self-healing
///
/// If the existing file cannot be read or parsed (corrupted XML,
/// non-UTF-8 bytes, IO error other than `NotFound`), it is deleted
/// best-effort and the modification proceeds against an empty map.
/// This collapses WPF's recursive retry pattern into a single
/// linear flow without needing a retry counter.
///
/// # Errors (deviation from WPF)
///
/// Surfaces [`ConfigError::Io`] / [`ConfigError::XmlWrite`] on the
/// final write/encode step. WPF silently swallows these via an
/// empty `catch{}` block at L60, which means user settings can be
/// lost without any signal. The Rust port surfaces them so the P10
/// service layer can decide whether to prompt the user. See the
/// [`crate::services::config`] module documentation for details.
pub async fn set_value(path: &Path, key: &str, value: Option<&str>) -> Result<(), ConfigError> {
    let path_owned = path.to_owned();
    let key_owned = key.to_owned();
    let value_owned = value.map(str::to_owned);

    spawn_blocking_config(move || set_value_blocking(&path_owned, &key_owned, value_owned)).await
}

fn set_value_blocking(path: &Path, key: &str, value: Option<String>) -> Result<(), ConfigError> {
    let mut map = match read_map_blocking(path) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                path = %path.display(),
                "config read/parse failed during set_value; deleting and starting from empty map"
            );
            let _ = std::fs::remove_file(path);
            IndexMap::new()
        }
    };

    match value {
        Some(v) => {
            // IndexMap::insert keeps the existing slot when key is
            // present (in-place update) and appends when absent —
            // exactly matching .NET `Settings[key].Value = v` /
            // `Settings.Add(key, v)` distinction without branching.
            map.insert(key.to_owned(), v);
        }
        None => {
            if map.shift_remove(key).is_none() {
                // No-op: WPF L21-25 explicitly skips writing when
                // the key was already absent and value is null.
                return Ok(());
            }
        }
    }

    let xml = serialize_app_settings(&map)?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
    }
    std::fs::write(path, xml).map_err(ConfigError::Io)?;
    Ok(())
}

async fn spawn_blocking_config<F, R>(f: F) -> Result<R, ConfigError>
where
    F: FnOnce() -> Result<R, ConfigError> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|join_err| ConfigError::Io(std::io::Error::other(join_err)))?
}

/// Resolve the production `Config.xml` path —
/// `%APPDATA%\Beanfun\Config.xml` — matching WPF
/// `Environment.GetFolderPath(SpecialFolder.ApplicationData)` at
/// `Beanfun/Helper/ConfigAppSettings.cs` L14-16.
///
/// Returns [`ConfigError::AppDataMissing`] when the `APPDATA`
/// environment variable is unset or empty (should never happen on
/// Windows under normal user contexts).
#[cfg(target_os = "windows")]
pub fn default_config_xml_path() -> Result<PathBuf, ConfigError> {
    let appdata = std::env::var_os("APPDATA")
        .filter(|s| !s.is_empty())
        .ok_or(ConfigError::AppDataMissing)?;
    let mut path = PathBuf::from(appdata);
    path.push("Beanfun");
    path.push("Config.xml");
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Standard .NET-shaped WPF `Config.xml`, verbatim shape that
    /// `ConfigurationManager` writes (declaration + 2-space indent +
    /// `<configuration><appSettings><add ... />` schema).
    const WPF_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="Region" value="TW" />
    <add key="LastAccount" value="user@example.com" />
    <add key="AutoLogin" value="false" />
  </appSettings>
</configuration>"#;

    #[test]
    fn parse_wpf_fixture_round_trips_three_entries() {
        let map = parse_app_settings(WPF_FIXTURE).expect("WPF fixture parses");
        let entries: Vec<(&str, &str)> =
            map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(
            entries,
            vec![
                ("Region", "TW"),
                ("LastAccount", "user@example.com"),
                ("AutoLogin", "false"),
            ]
        );
    }

    #[test]
    fn parse_empty_app_settings_returns_empty_map() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings />
</configuration>"#;
        let map = parse_app_settings(xml).expect("empty appSettings parses");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_skips_unknown_sibling_sections() {
        // .NET `App.config` files often carry sections like `<startup>`
        // or `<connectionStrings>` alongside `<appSettings>`. Anything
        // we don't recognise must be silently skipped — including
        // nested `<add>` elements which would otherwise leak into the
        // result map.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <startup>
    <supportedRuntime version="v4.0" />
  </startup>
  <connectionStrings>
    <add name="db" connectionString="Server=." />
  </connectionStrings>
  <appSettings>
    <add key="Region" value="TW" />
  </appSettings>
  <runtime>
    <gcServer enabled="true" />
  </runtime>
</configuration>"#;
        let map = parse_app_settings(xml).expect("config with unknown sections parses");
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("Region").map(String::as_str), Some("TW"));
    }

    #[test]
    fn parse_decodes_xml_attribute_escapes() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="Quote" value="he said &quot;hi&quot;" />
    <add key="Amp" value="A &amp; B" />
    <add key="Lt" value="x &lt; y" />
    <add key="Apos" value="it&apos;s" />
  </appSettings>
</configuration>"#;
        let map = parse_app_settings(xml).expect("escape parses");
        assert_eq!(
            map.get("Quote").map(String::as_str),
            Some(r#"he said "hi""#)
        );
        assert_eq!(map.get("Amp").map(String::as_str), Some("A & B"));
        assert_eq!(map.get("Lt").map(String::as_str), Some("x < y"));
        assert_eq!(map.get("Apos").map(String::as_str), Some("it's"));
    }

    #[test]
    fn parse_malformed_xml_returns_xml_parse_error() {
        let xml = "<configuration><appSettings><add key=\"a\" value=\"b\"></appSettings>";
        let err = parse_app_settings(xml).expect_err("mismatched tag should fail");
        assert!(matches!(err, ConfigError::XmlParse(_)));
    }

    #[test]
    fn parse_preserves_insertion_order_across_many_keys() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="z" value="1" />
    <add key="a" value="2" />
    <add key="m" value="3" />
    <add key="b" value="4" />
  </appSettings>
</configuration>"#;
        let map = parse_app_settings(xml).expect("ordered parse");
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["z", "a", "m", "b"]);
    }

    #[test]
    fn serialize_empty_map_writes_self_closing_app_settings() {
        let map = IndexMap::new();
        let xml = serialize_app_settings(&map).expect("empty map serializes");
        // Self-closing form matches what .NET ConfigurationManager
        // writes for an empty section. Locking the exact bytes here
        // catches any regression in the writer's empty-section path.
        let expected =
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<configuration>\n  <appSettings/>\n</configuration>";
        assert_eq!(xml, expected);
    }

    #[test]
    fn serialize_then_parse_round_trips_arbitrary_map() {
        let mut map = IndexMap::new();
        map.insert("Region".to_string(), "TW".to_string());
        map.insert("LastAccount".to_string(), "user@example.com".to_string());
        map.insert("AutoLogin".to_string(), "false".to_string());

        let xml = serialize_app_settings(&map).expect("serialize");
        let parsed = parse_app_settings(&xml).expect("parse");
        assert_eq!(parsed, map);
    }

    #[test]
    fn serialize_escapes_special_xml_characters() {
        let mut map = IndexMap::new();
        map.insert("Quote".to_string(), r#"he said "hi""#.to_string());
        map.insert("Amp".to_string(), "A & B".to_string());
        map.insert("Lt".to_string(), "x < y".to_string());
        let xml = serialize_app_settings(&map).expect("escape serialize");
        // Round-trip is the strongest guarantee: parse the serialized
        // bytes back and confirm the original strings come out intact.
        let parsed = parse_app_settings(&xml).expect("escape round-trip");
        assert_eq!(parsed, map);
        // Sanity-check the wire format actually contains escape entities
        // (otherwise the round-trip would still pass with broken-but-
        // symmetric encoding).
        assert!(xml.contains("&quot;"));
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&lt;"));
    }

    #[test]
    fn serialize_preserves_insertion_order() {
        let mut map = IndexMap::new();
        map.insert("z".to_string(), "1".to_string());
        map.insert("a".to_string(), "2".to_string());
        map.insert("m".to_string(), "3".to_string());

        let xml = serialize_app_settings(&map).expect("serialize");
        let z_idx = xml.find("\"z\"").expect("z present");
        let a_idx = xml.find("\"a\"").expect("a present");
        let m_idx = xml.find("\"m\"").expect("m present");
        assert!(z_idx < a_idx, "z should appear before a");
        assert!(a_idx < m_idx, "a should appear before m");
    }

    #[test]
    fn config_error_display_messages_are_distinct() {
        let io = ConfigError::Io(std::io::Error::other("disk full"));
        let app_data = ConfigError::AppDataMissing;
        let io_msg = io.to_string();
        let app_data_msg = app_data.to_string();
        assert!(io_msg.contains("disk full"));
        assert!(app_data_msg.contains("APPDATA"));
        assert_ne!(io_msg, app_data_msg);
    }
}
