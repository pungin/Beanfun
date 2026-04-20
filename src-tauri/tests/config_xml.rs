//! Integration tests for `services::config::xml` covering the
//! IO-bearing async APIs (`get_value` / `get_value_or` / `set_value`)
//! and `default_config_xml_path`.
//!
//! All tests use [`tempfile::TempDir`] for filesystem isolation.
//! `default_config_xml_path` is gated `#[cfg(target_os = "windows")]`
//! to match the production helper's platform scope.

use beanfun_lib::services::config::{
    get_all_values, get_value, get_value_or, parse_app_settings, serialize_app_settings, set_value,
    ConfigError,
};
use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;

/// Standard .NET-shaped WPF `Config.xml` fixture, used to confirm
/// upstream-produced bytes round-trip through our reader / writer.
const WPF_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="Region" value="TW" />
    <add key="LastAccount" value="user@example.com" />
    <add key="AutoLogin" value="false" />
  </appSettings>
</configuration>"#;

fn temp_config_path() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("Config.xml");
    (dir, path)
}

#[tokio::test]
async fn missing_file_get_value_returns_default() {
    let (_dir, path) = temp_config_path();
    let value = get_value_or(&path, "Region", "TW").await;
    assert_eq!(value, "TW");
    let blank = get_value(&path, "Region").await;
    assert_eq!(blank, "");
    assert!(!path.exists(), "get_value must not create the file");
}

#[tokio::test]
async fn missing_file_set_value_creates_file_and_parent() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("nested").join("dirs").join("Config.xml");
    assert!(!path.exists());
    set_value(&path, "Region", Some("HK"))
        .await
        .expect("set_value creates file");
    assert!(path.exists(), "set_value must create the file");
    let value = get_value_or(&path, "Region", "TW").await;
    assert_eq!(value, "HK");
}

#[tokio::test]
async fn set_then_get_round_trips_value() {
    let (_dir, path) = temp_config_path();
    set_value(&path, "Region", Some("HK"))
        .await
        .expect("set Region");
    set_value(&path, "AutoLogin", Some("true"))
        .await
        .expect("set AutoLogin");
    assert_eq!(get_value_or(&path, "Region", "x").await, "HK");
    assert_eq!(get_value_or(&path, "AutoLogin", "x").await, "true");
    assert_eq!(get_value_or(&path, "Missing", "fallback").await, "fallback");
}

#[tokio::test]
async fn set_value_none_removes_existing_key() {
    let (_dir, path) = temp_config_path();
    set_value(&path, "Region", Some("TW")).await.expect("set");
    set_value(&path, "Region", None).await.expect("remove");
    let bytes = std::fs::read(&path).expect("file still exists");
    let xml = std::str::from_utf8(&bytes).expect("utf-8");
    let map = parse_app_settings(xml).expect("parse");
    assert!(!map.contains_key("Region"));
}

#[tokio::test]
async fn set_value_none_for_missing_key_is_a_noop() {
    let (_dir, path) = temp_config_path();
    set_value(&path, "Missing", None)
        .await
        .expect("no-op succeeds");
    // WPF L21-25 explicitly skips the file write when value is null
    // and the key was absent. Mirror that contract: file must not be
    // created.
    assert!(
        !path.exists(),
        "no-op set_value must not create the file (WPF parity)"
    );
}

#[tokio::test]
async fn corrupted_file_self_heals_on_set_value() {
    let (_dir, path) = temp_config_path();
    std::fs::write(&path, "not <valid> xml at all").expect("seed garbage");
    set_value(&path, "Region", Some("TW"))
        .await
        .expect("set heals corruption");
    // After self-heal the on-disk file contains only the new key —
    // the corrupt content is gone.
    let value = get_value_or(&path, "Region", "x").await;
    assert_eq!(value, "TW");
    let bytes = std::fs::read(&path).expect("file rewritten");
    let xml = std::str::from_utf8(&bytes).expect("utf-8");
    let map = parse_app_settings(xml).expect("parse rewrite");
    assert_eq!(map.len(), 1);
}

#[tokio::test]
async fn corrupted_file_get_value_returns_default_without_deletion() {
    let (_dir, path) = temp_config_path();
    let original = b"not <valid> xml at all";
    std::fs::write(&path, original).expect("seed garbage");
    let value = get_value_or(&path, "Region", "TW").await;
    assert_eq!(value, "TW");
    // get_value is non-destructive: it must leave the file untouched
    // so the user (or set_value) can still inspect / overwrite it.
    let still_there = std::fs::read(&path).expect("file still exists");
    assert_eq!(&still_there[..], original);
}

#[tokio::test]
async fn update_preserves_insertion_order() {
    let (_dir, path) = temp_config_path();
    set_value(&path, "z", Some("1")).await.expect("set z");
    set_value(&path, "a", Some("2")).await.expect("set a");
    set_value(&path, "m", Some("3")).await.expect("set m");
    // Updating an existing key must keep its slot — IndexMap::insert
    // is in-place for present keys.
    set_value(&path, "a", Some("UPDATED"))
        .await
        .expect("update a");
    let xml_bytes = std::fs::read(&path).expect("file");
    let xml = std::str::from_utf8(&xml_bytes).expect("utf-8");
    let map = parse_app_settings(xml).expect("parse");
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["z", "a", "m"]);
    assert_eq!(map.get("a").map(String::as_str), Some("UPDATED"));
}

#[tokio::test]
async fn wpf_fixture_round_trips_through_set_value() {
    let (_dir, path) = temp_config_path();
    std::fs::write(&path, WPF_FIXTURE).expect("seed WPF fixture");
    // Mutating one key via the IO API exercises the full
    // read → modify → serialize → write loop against bytes that
    // came from upstream WPF.
    set_value(&path, "Region", Some("HK"))
        .await
        .expect("set on WPF fixture");
    let xml_bytes = std::fs::read(&path).expect("file");
    let xml = std::str::from_utf8(&xml_bytes).expect("utf-8");
    let map = parse_app_settings(xml).expect("parse");
    assert_eq!(map.get("Region").map(String::as_str), Some("HK"));
    assert_eq!(
        map.get("LastAccount").map(String::as_str),
        Some("user@example.com")
    );
    assert_eq!(map.get("AutoLogin").map(String::as_str), Some("false"));
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["Region", "LastAccount", "AutoLogin"]);
}

#[tokio::test]
async fn export_then_import_preserves_arbitrary_map() {
    // Cross-check the pure parser/serializer pair preserves an
    // arbitrary map exactly, covering both special-character escape
    // and a non-trivial number of entries in the same shot.
    let mut map = IndexMap::new();
    map.insert("Region".to_string(), "TW".to_string());
    map.insert("Quote".to_string(), r#"he said "hi""#.to_string());
    map.insert("Amp".to_string(), "A & B".to_string());
    map.insert("Lt".to_string(), "x < y".to_string());
    map.insert("Apos".to_string(), "it's".to_string());
    map.insert("Empty".to_string(), String::new());
    let xml = serialize_app_settings(&map).expect("serialize");
    let parsed = parse_app_settings(&xml).expect("parse");
    assert_eq!(parsed, map);
}

// ---------------------------------------------------------------------
// get_all_values — P10.3 D2 addition. Same IO-bearing async surface
// as `get_value` but returns the full map so the settings page can
// render the whole `Config.xml` in one round-trip.
// ---------------------------------------------------------------------

#[tokio::test]
async fn get_all_values_missing_file_returns_empty_map() {
    let (_dir, path) = temp_config_path();
    let map = get_all_values(&path).await.expect("missing file is Ok");
    assert!(
        map.is_empty(),
        "missing file must collapse to empty map, not fail"
    );
    assert!(
        !path.exists(),
        "get_all_values must not create the file (parity with get_value)"
    );
}

#[tokio::test]
async fn get_all_values_preserves_insertion_order() {
    let (_dir, path) = temp_config_path();
    std::fs::write(&path, WPF_FIXTURE).expect("seed WPF fixture");
    let map = get_all_values(&path)
        .await
        .expect("WPF fixture reads as ordered map");
    let entries: Vec<(&str, &str)> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    // Order must match the on-disk `<add>` sequence — the frontend
    // settings page relies on this to keep the UI stable across
    // save / reload cycles.
    assert_eq!(
        entries,
        vec![
            ("Region", "TW"),
            ("LastAccount", "user@example.com"),
            ("AutoLogin", "false"),
        ]
    );
}

#[tokio::test]
async fn get_all_values_corrupted_xml_surfaces_xml_parse_error() {
    // Unlike `get_value` (WPF-parity catch-all → ""), `get_all_values`
    // surfaces typed errors so the command layer can decide whether
    // to swallow + log (D2 `get_all_config`) or bubble up (future
    // diagnostics). Corrupted XML is the parse-error signal.
    let (_dir, path) = temp_config_path();
    std::fs::write(&path, "<configuration><appSettings><add key=\"a\"")
        .expect("seed corrupted xml");
    let err = get_all_values(&path)
        .await
        .expect_err("corrupted xml must surface typed error");
    assert!(matches!(err, ConfigError::XmlParse(_)));
}

#[tokio::test]
async fn get_all_values_non_utf8_surfaces_io_error() {
    let (_dir, path) = temp_config_path();
    // Write raw bytes that are not valid UTF-8 (`0xFF` is never a
    // valid UTF-8 start byte). `read_map_blocking` maps this to
    // `ConfigError::Io(InvalidData)`.
    std::fs::write(&path, [0xFFu8, 0xFE, 0xFD]).expect("seed non-utf8");
    let err = get_all_values(&path)
        .await
        .expect_err("non-utf8 must surface typed error");
    match err {
        ConfigError::Io(io_err) => {
            assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidData);
        }
        other => panic!("expected Io(InvalidData), got {other:?}"),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn default_config_xml_path_resolves_under_appdata_beanfun() {
    // We don't mutate APPDATA — the standard Windows session always
    // sets it. Only assert that the resolved path lands under
    // %APPDATA%\Beanfun\Config.xml exactly as WPF
    // `SpecialFolder.ApplicationData + "\\Beanfun\\Config.xml"` does.
    use beanfun_lib::services::config::default_config_xml_path;

    let appdata = std::env::var_os("APPDATA").expect("APPDATA must be set on Windows");
    let expected = PathBuf::from(&appdata).join("Beanfun").join("Config.xml");
    let resolved = default_config_xml_path().expect("resolve default path");
    assert_eq!(resolved, expected);
}
