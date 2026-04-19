//! Game-listing surface: fetch the per-region INI of executable
//! metadata + the JSON list of game services that the launcher's
//! `GameList` dialog and `selectedGameChanged` pipeline both
//! consume.
//!
//! Ports `MainWindow.xaml.cs::reLoadGameInfo` (L682-746) +
//! `class GameService` (L419-518) + `selectedGameChanged` INI
//! lookups (L520-680).
//!
//! | This module                       | WPF reference                                      |
//! |-----------------------------------|----------------------------------------------------|
//! | [`list_games`]                    | `reLoadGameInfo` (atomic INI + ServiceList fetch)  |
//! | [`parse_service_ini`]             | `IniDataParser().Parse(res)`                       |
//! | [`parse_service_list`]            | `Services.ServiceList = (.*);` regex + JSON parse  |
//! | [`GameService`]                   | `class GameService` (sans BitmapImage lazy fields) |
//! | [`GameIniEntry`]                  | implicit (`INIData[gameCode]["..."]` accessors)    |
//!
//! # Wire shape
//!
//! `get_service_ini.ashx` returns a plain INI document keyed by
//! `<service_code>_<service_region>` (e.g. `[610074_T9]`). Every
//! known WPF accessor under `selectedGameChanged` reads exactly the
//! five keys captured by [`GameIniEntry`]; missing keys map to
//! `""` to mirror WPF's `if (sLoginActionType != "")` /
//! `if (dir_reg != "")` guards (which assume an empty string for
//! absent keys, not `null`).
//!
//! `game_zone/` returns an HTML page where a `Services.ServiceList`
//! JS literal is the only piece we care about. Two historical
//! shapes co-exist (WPF L711-723):
//!
//! - **New shape** — bare JSON array `[ {…}, {…} ]` (current TW).
//! - **Old shape** — wrapper object `{ "Rows": [ {…}, {…} ] }`.
//!
//! [`parse_service_list`] tries the new shape first (`^\[ … \]$`
//! match) and falls back to `Rows`, matching WPF byte-for-byte.
//!
//! # WPF deviations (deliberate, documented)
//!
//! - **Lazy `BitmapImage` fields dropped**: WPF's `GameService`
//!   carries three `BitmapImage` getters that lazy-fetch
//!   `XLarge_image` / `Large_image` / `Small_image` via
//!   `WebClient.DownloadData(imageBaseUrl + name)`. The Tauri
//!   webview can render `<img src="https://…">` directly with no
//!   CSP friction (TW image host is HTTPS; HK uses HTTP, which
//!   matches WPF's [`HK image base URL`] verbatim and still
//!   loads in a permissive WebView2). Pushing image loads to the
//!   frontend keeps this module pure-data and avoids a Vec<u8>
//!   round-trip across IPC for every tile. The frontend
//!   constructs URLs via the [`image_base_url`] helper so the
//!   region branch lives in exactly one place.
//!
//! - **`I18n.ToSimplified(name)` skipped here**: WPF normalises
//!   service names to Simplified Chinese inside the constructor
//!   (`new GameService(I18n.ToSimplified(name), …)`). The
//!   beanfun-next i18n strategy localises at the Vue layer via
//!   `vue-i18n` (the user's chosen UI language is a frontend
//!   concern, not a transport concern), so we surface the raw
//!   Traditional-Chinese name and let the UI decide. Frontend
//!   side is free to apply the same simplification via a
//!   pre-existing `simplify-chinese` helper if a future zh-CN
//!   variant requires it.
//!
//! - **Regex `Services\.ServiceList = (.*);` left greedy**:
//!   Mirrors WPF's literal regex (`.*` is greedy). Server output
//!   currently has exactly one `;` after the assignment (the JS
//!   literal is on its own line) so greediness is harmless; we
//!   preserve it so the matcher behaves identically across
//!   futures where servers might reformat surrounding markup.
//!
//! - **Typed [`LoginError::GameListServiceListMissing`] surfaced**:
//!   WPF silently produces an empty list when the regex fails to
//!   match (`if (reg.IsMatch(res))` is the only guard, no
//!   `else`). Empty UI gives the user no recourse on an
//!   upstream regression. Surfacing the error lets the dialog
//!   show a retry banner instead. This is a strictly-additive UX
//!   improvement; behaviourally WPF and beanfun-next both refuse
//!   to advance past this point with no service list.
//!
//! [`HK image base URL`]:
//!   `MainWindow.xaml.cs` L436 hardcodes `http://hk.images.beanfun.com/...`
//!   — the launcher does not upgrade to HTTPS for HK assets.

use std::collections::HashMap;

use regex::Regex;
use serde_json::Value;
use tracing::warn;

use super::client::{BeanfunClient, LoginRegion};
use super::error::LoginError;
use super::login::ensure_success;

// -----------------------------------------------------------------------------
// DTOs
// -----------------------------------------------------------------------------

/// One entry from the per-region INI returned by
/// `get_service_ini.ashx`.
///
/// Every field defaults to the empty string when the corresponding
/// INI key is absent — matching WPF's `INIData[gameCode]["..."]`
/// access pattern, which yields `""` for missing keys via
/// `IniParser`'s `KeyDataCollection` indexer (not `null`, even
/// though WPF L530 still bothers to guard `exe == null`; see the
/// [`GameIniEntry::is_runnable`] doc for how we reconcile both
/// branches).
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct GameIniEntry {
    /// `exe` key — full executable command line (path + arguments
    /// separated by `.exe `). WPF L536-545 splits this with
    /// `(.*).exe` / `.exe (.*)` regexes into `game_exe` and
    /// `game_commandLine`. Empty string means "this gameCode has
    /// no INI definition" (per the [`Self::is_runnable`] gate).
    pub exe: String,

    /// `login_action_type` — short integer encoded as text. Drives
    /// the `tradLogin` / `panel_GetOtp` UI branch in WPF L548-563.
    /// Empty defaults to action type 8 (WPF L547).
    pub login_action_type: String,

    /// `win_class_name` — Win32 class name of the game window the
    /// launcher should focus / paste OTP into. Drives
    /// `accountList.autoPaste.Visibility` (WPF L565-573, only
    /// `"MapleStoryClass"` opts in).
    pub win_class_name: String,

    /// `dir_value_name` — registry value name under `dir_reg` that
    /// stores the game's installation directory (WPF L574-607).
    pub dir_value_name: String,

    /// `dir_reg` — registry key path (with the leading
    /// `HKEY_LOCAL_MACHINE\` stripped before use, WPF L580). The
    /// launcher reads `dir_reg::dir_value_name` and writes it back
    /// into `Config.xml` for future launches.
    pub dir_reg: String,
}

impl GameIniEntry {
    /// `true` when the entry has a non-empty `exe`.
    ///
    /// WPF's `selectedGameChanged` opens the GameList dialog when
    /// `INIData[gameCode]["exe"]` is `null` (L530-535); in our
    /// `String`-based model "absent" is "" (the IniParser default
    /// for missing keys), so this single helper covers both
    /// "key missing" and "key present but empty" with one branch.
    /// Frontend dispatchers (`AccountList.vue` Start Game button)
    /// gate on this.
    pub fn is_runnable(&self) -> bool {
        !self.exe.is_empty()
    }
}

/// One service from the `Services.ServiceList` JS literal returned
/// by `game_zone/`.
///
/// Field naming follows WPF's `class GameService` exactly so the
/// frontend's `GameList.vue` can map field-for-field. Image
/// **bytes** are not loaded here — the frontend constructs an
/// `<img src>` URL via the region-aware base URL (see
/// [`image_base_url`]) and lets the WebView fetch directly.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct GameService {
    /// `ServiceFamilyName` — display name (e.g. `"新楓之谷"`).
    /// Surfaces the raw Traditional-Chinese name; `vue-i18n`
    /// handles localisation at render time.
    pub name: String,
    /// `ServiceCode` — six-digit game id (e.g. `"610074"`).
    pub service_code: String,
    /// `ServiceRegion` — two-character sub-region (e.g. `"T9"`).
    pub service_region: String,
    /// `ServiceWebsiteURL` — official site URL, surfaced verbatim
    /// for the per-account row context menu's "Official Site"
    /// item (P12.2 D-step / P12.4 WebBrowser).
    pub website_url: String,
    /// `ServiceXLargeImageName` — file name for the extra-large
    /// game banner (e.g. `"610074.jpg"`). Resolve to a URL via
    /// [`image_base_url`].
    pub xlarge_image_name: String,
    /// `ServiceLargeImageName` — file name for the large game
    /// banner used by both the GameList grid and the LoginPage
    /// hero illustration in WPF.
    pub large_image_name: String,
    /// `ServiceSmallImageName` — file name for the small game
    /// icon used in the AccountList header bar.
    pub small_image_name: String,
    /// `ServiceDownloadURL` — installer / patcher URL surfaced as
    /// a "Download" affordance when the game executable is
    /// missing locally (P12.3 launcher fallback).
    pub download_url: String,
}

/// Atomic bundle returned by [`list_games`] — the INI map keyed by
/// `<service_code>_<service_region>` plus the ordered service
/// list for the active region.
///
/// Keeping both halves in one round-trip mirrors WPF's
/// `reLoadGameInfo()`, which fetches both inside one method, and
/// avoids an inconsistent intermediate state where the frontend
/// has services but no INI to launch them with.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GameInfoBundle {
    /// INI section name → typed entry. Section names are
    /// `<service_code>_<service_region>` (e.g. `"610074_T9"`).
    pub ini: HashMap<String, GameIniEntry>,
    /// Display-ordered list of services for the current region —
    /// preserves the server's ordering verbatim so any "newest
    /// game first" / "promoted game pinned" curation upstream
    /// flows through to the UI without us re-sorting.
    pub services: Vec<GameService>,
}

// -----------------------------------------------------------------------------
// Image base URL (region-aware)
// -----------------------------------------------------------------------------

/// Region-scoped image base URL — concat with one of the
/// `*_image_name` fields on [`GameService`] to get a fully
/// resolved `<img src>` URL.
///
/// Mirrors `MainWindow.xaml.cs::GameService.imageBaseUrl` (L430-437)
/// **including** the HK `http://` scheme — WPF does not upgrade HK
/// asset traffic to HTTPS. Tauri's WebView2 will load mixed
/// content from `tauri://localhost` (the app origin) without
/// blocking, so the HTTP scheme works at runtime; we preserve it
/// for byte-for-byte URL parity with the legacy launcher.
pub fn image_base_url(region: LoginRegion) -> &'static str {
    match region {
        LoginRegion::TW => "https://tw.images.beanfun.com/uploaded_images/beanfun_tw/game_zone/",
        LoginRegion::HK => "http://hk.images.beanfun.com/uploaded_images/beanfun/game_zone/",
    }
}

// -----------------------------------------------------------------------------
// Public entry point
// -----------------------------------------------------------------------------

/// Fetch + parse the INI + ServiceList atomically for the
/// region the `client` is configured for.
///
/// Mirrors `MainWindow.xaml.cs::reLoadGameInfo` (L682-729) — the
/// two GETs (`get_service_ini.ashx`, `game_zone/`) + the regex
/// extraction + the JSON parse all happen here; nothing else
/// depends on partial state.
///
/// # Errors
///
/// - [`LoginError::Http`] / [`LoginError::InvalidUtf8`] for
///   transport or encoding failures on either GET.
/// - [`LoginError::GameListServiceListMissing`] when the
///   `game_zone/` HTML response did not contain the
///   `Services.ServiceList = …;` JS literal (catastrophic
///   upstream regression — WPF would silently surface an empty
///   list instead).
/// - [`LoginError::Json`] when the literal payload is present
///   but neither shape parses as JSON.
/// - [`LoginError::Unknown`] for non-2xx responses (via
///   [`ensure_success`]).
pub async fn list_games(client: &BeanfunClient) -> Result<GameInfoBundle, LoginError> {
    let ini_url = client.portal_url("beanfun_block/generic_handlers/get_service_ini.ashx")?;
    let ini_resp = client.http().get(ini_url).send().await?;
    ensure_success(&ini_resp, "get_service_ini.ashx")?;
    let ini_body = client.bounded_text(ini_resp).await?;
    let ini = parse_service_ini(&ini_body);

    let zone_url = client.portal_url("game_zone/")?;
    let zone_resp = client.http().get(zone_url).send().await?;
    ensure_success(&zone_resp, "game_zone/")?;
    let zone_body = client.bounded_text(zone_resp).await?;
    let services = parse_service_list(&zone_body)?;

    Ok(GameInfoBundle { ini, services })
}

// -----------------------------------------------------------------------------
// Pure parsers
// -----------------------------------------------------------------------------

/// Parse a `get_service_ini.ashx` body into the typed map.
///
/// Recognises the standard INI subset that the upstream document
/// uses: `[section]` headers, `key=value` pairs, blank lines, and
/// comments starting with `;` or `#` (matches `IniParser`'s
/// default `CommentString` set).
///
/// Lines outside any section, malformed lines (no `=`), and
/// duplicate keys within a section all behave as `IniParser`
/// does: orphan lines are dropped, malformed lines are skipped,
/// duplicates keep the **last** value (`IniParser`'s default
/// `OverrideExistingKeysWhenLoading = true`). Whitespace around
/// keys, values, and section names is trimmed — also matching
/// `IniParser`'s `SkipInvalidLines` + `TrimSections` defaults.
///
/// Pure / sync function — testable without a network round trip.
pub fn parse_service_ini(text: &str) -> HashMap<String, GameIniEntry> {
    let mut out: HashMap<String, GameIniEntry> = HashMap::new();
    let mut current_section: Option<String> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('[') {
            if let Some(name) = rest.strip_suffix(']') {
                let name = name.trim().to_owned();
                out.entry(name.clone()).or_default();
                current_section = Some(name);
            } else {
                warn!(line = raw, "ini: malformed section header (no `]`)");
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        let Some(section) = current_section.as_ref() else {
            continue;
        };
        let entry = out.entry(section.clone()).or_default();
        match key {
            "exe" => entry.exe = value.to_owned(),
            "login_action_type" => entry.login_action_type = value.to_owned(),
            "win_class_name" => entry.win_class_name = value.to_owned(),
            "dir_value_name" => entry.dir_value_name = value.to_owned(),
            "dir_reg" => entry.dir_reg = value.to_owned(),
            _ => {
                // Unknown keys are intentionally dropped — WPF
                // never reads them and surfacing them would
                // bloat the IPC payload + bind us to upstream
                // additions. New launcher behaviour should add
                // a typed field here so the dependency is
                // visible.
            }
        }
    }

    out
}

/// Parse the `game_zone/` HTML body into the ordered service list.
///
/// Steps (mirrors WPF L707-724):
///
/// 1. Apply `Services\.ServiceList = (.*);` regex; capture group
///    1 is the JSON literal. Missing match ⇒
///    [`LoginError::GameListServiceListMissing`].
/// 2. If the literal starts with `[` and ends with `]`, parse as
///    a bare JSON array (new shape, current TW).
/// 3. Otherwise parse as `{ "Rows": [...] }` (old shape).
/// 4. Walk the array and convert each object into a
///    [`GameService`] via [`GameService::from_json`].
///
/// Pure / sync function — testable without a network round trip.
pub fn parse_service_list(html: &str) -> Result<Vec<GameService>, LoginError> {
    let re = service_list_regex();
    let json = re
        .captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim())
        .ok_or(LoginError::GameListServiceListMissing)?;

    let items: Vec<Value> = if json.starts_with('[') && json.ends_with(']') {
        serde_json::from_str(json)?
    } else {
        let outer: Value = serde_json::from_str(json)?;
        match outer.get("Rows").cloned() {
            Some(Value::Array(rows)) => rows,
            _ => Vec::new(),
        }
    };

    Ok(items.iter().map(GameService::from_json).collect())
}

impl GameService {
    /// Convert one JSON object from the `Services.ServiceList`
    /// literal (or its `Rows` wrapper) into a typed
    /// [`GameService`]. Missing fields default to `""`, matching
    /// WPF's `(string)game["…"]` casts which yield `null` and
    /// then concatenate as empty.
    fn from_json(raw: &Value) -> Self {
        Self {
            name: string_field(raw, "ServiceFamilyName"),
            service_code: string_field(raw, "ServiceCode"),
            service_region: string_field(raw, "ServiceRegion"),
            website_url: string_field(raw, "ServiceWebsiteURL"),
            xlarge_image_name: string_field(raw, "ServiceXLargeImageName"),
            large_image_name: string_field(raw, "ServiceLargeImageName"),
            small_image_name: string_field(raw, "ServiceSmallImageName"),
            download_url: string_field(raw, "ServiceDownloadURL"),
        }
    }
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn service_list_regex() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"Services\.ServiceList = (.*);").expect("static regex compiles"))
}

fn string_field(raw: &Value, key: &str) -> String {
    raw.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // -------------------------------------------------------------------------
    // INI parser
    // -------------------------------------------------------------------------

    #[test]
    fn parse_ini_minimal_one_section() {
        let text = "\
[610074_T9]
exe=MapleStory.exe tw.login.maplestory.beanfun.com 8484 BeanFun %s %s
login_action_type=8
win_class_name=MapleStoryClass
dir_value_name=ExecPath
dir_reg=HKEY_LOCAL_MACHINE\\SOFTWARE\\Gamania\\MapleStory
";

        let map = parse_service_ini(text);
        assert_eq!(map.len(), 1);
        let e = &map["610074_T9"];
        assert_eq!(
            e.exe,
            "MapleStory.exe tw.login.maplestory.beanfun.com 8484 BeanFun %s %s"
        );
        assert_eq!(e.login_action_type, "8");
        assert_eq!(e.win_class_name, "MapleStoryClass");
        assert_eq!(e.dir_value_name, "ExecPath");
        assert_eq!(
            e.dir_reg,
            "HKEY_LOCAL_MACHINE\\SOFTWARE\\Gamania\\MapleStory"
        );
    }

    #[test]
    fn parse_ini_skips_blank_and_comment_lines() {
        let text = "\
; this is a comment
# also a comment

[610074_T9]
exe=Maple.exe a b
; mid-section comment
login_action_type=8
";
        let map = parse_service_ini(text);
        let e = &map["610074_T9"];
        assert_eq!(e.exe, "Maple.exe a b");
        assert_eq!(e.login_action_type, "8");
    }

    #[test]
    fn parse_ini_trims_keys_values_and_section_name() {
        let text = "\
[ 610074_T9 ]
  exe   =   Maple.exe a b
\twin_class_name\t=\tMapleStoryClass\t
";
        let map = parse_service_ini(text);
        assert!(map.contains_key("610074_T9"));
        let e = &map["610074_T9"];
        assert_eq!(e.exe, "Maple.exe a b");
        assert_eq!(e.win_class_name, "MapleStoryClass");
    }

    #[test]
    fn parse_ini_duplicate_keys_keep_last_value() {
        let text = "\
[610074_T9]
exe=first.exe
exe=second.exe
";
        let map = parse_service_ini(text);
        assert_eq!(map["610074_T9"].exe, "second.exe");
    }

    #[test]
    fn parse_ini_orphan_lines_before_any_section_dropped() {
        let text = "\
key=value
[610074_T9]
exe=Maple.exe
";
        let map = parse_service_ini(text);
        assert_eq!(map.len(), 1);
        assert_eq!(map["610074_T9"].exe, "Maple.exe");
    }

    #[test]
    fn parse_ini_unknown_keys_dropped_silently() {
        let text = "\
[610074_T9]
exe=Maple.exe
some_future_key=ignored
login_action_type=8
";
        let map = parse_service_ini(text);
        let e = &map["610074_T9"];
        assert_eq!(e.exe, "Maple.exe");
        assert_eq!(e.login_action_type, "8");
    }

    #[test]
    fn parse_ini_multiple_sections() {
        let text = "\
[610074_T9]
exe=Maple.exe args
[610153_TN]
exe=Unconnected.exe
win_class_name=UnconnectedClass
";
        let map = parse_service_ini(text);
        assert_eq!(map.len(), 2);
        assert_eq!(map["610074_T9"].exe, "Maple.exe args");
        assert_eq!(map["610153_TN"].exe, "Unconnected.exe");
        assert_eq!(map["610153_TN"].win_class_name, "UnconnectedClass");
    }

    #[test]
    fn parse_ini_section_header_without_closing_bracket_skipped() {
        let text = "\
[broken
exe=ignored.exe
[610074_T9]
exe=Maple.exe
";
        let map = parse_service_ini(text);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("610074_T9"));
    }

    #[test]
    fn parse_ini_missing_keys_default_to_empty_string() {
        let text = "[610074_T9]\nexe=Maple.exe\n";
        let map = parse_service_ini(text);
        let e = &map["610074_T9"];
        assert_eq!(e.exe, "Maple.exe");
        assert_eq!(e.login_action_type, "");
        assert_eq!(e.win_class_name, "");
        assert_eq!(e.dir_value_name, "");
        assert_eq!(e.dir_reg, "");
    }

    #[test]
    fn ini_entry_is_runnable_helper() {
        let mut e = GameIniEntry::default();
        assert!(!e.is_runnable());
        e.exe = "Maple.exe".to_owned();
        assert!(e.is_runnable());
    }

    #[test]
    fn parse_ini_section_header_with_trailing_garbage_treated_as_invalid() {
        let text = "[610074_T9] extra\nexe=Maple.exe\n";
        let map = parse_service_ini(text);
        assert!(map.is_empty());
    }

    #[test]
    fn parse_ini_value_with_equals_sign_kept_intact() {
        let text = "[610074_T9]\nexe=Maple.exe a=b c=d\n";
        let map = parse_service_ini(text);
        assert_eq!(map["610074_T9"].exe, "Maple.exe a=b c=d");
    }

    // -------------------------------------------------------------------------
    // ServiceList parser
    // -------------------------------------------------------------------------

    #[test]
    fn parse_service_list_new_shape_array() {
        // Real upstream HTML emits the JSON literal on one line — WPF
        // regex `.` does not span newlines (no Singleline flag) so we
        // mirror that constraint in the fixture.
        let html = r#"<html><body><script>var Services = {}; Services.ServiceList = [{"ServiceFamilyName":"新楓之谷","ServiceCode":"610074","ServiceRegion":"T9","ServiceWebsiteURL":"https://maplestory.beanfun.com/","ServiceXLargeImageName":"610074_xl.jpg","ServiceLargeImageName":"610074_l.jpg","ServiceSmallImageName":"610074_s.jpg","ServiceDownloadURL":"https://download/maple"}];</script></body></html>"#;

        let services = parse_service_list(html).unwrap();
        assert_eq!(services.len(), 1);
        let s = &services[0];
        assert_eq!(s.name, "新楓之谷");
        assert_eq!(s.service_code, "610074");
        assert_eq!(s.service_region, "T9");
        assert_eq!(s.website_url, "https://maplestory.beanfun.com/");
        assert_eq!(s.xlarge_image_name, "610074_xl.jpg");
        assert_eq!(s.large_image_name, "610074_l.jpg");
        assert_eq!(s.small_image_name, "610074_s.jpg");
        assert_eq!(s.download_url, "https://download/maple");
    }

    #[test]
    fn parse_service_list_old_shape_rows_wrapper() {
        let html = r#"<html><body><script>Services.ServiceList = {"Rows":[{"ServiceFamilyName":"A","ServiceCode":"610074","ServiceRegion":"T9","ServiceWebsiteURL":"","ServiceXLargeImageName":"","ServiceLargeImageName":"","ServiceSmallImageName":"","ServiceDownloadURL":""},{"ServiceFamilyName":"B","ServiceCode":"610153","ServiceRegion":"TN","ServiceWebsiteURL":"","ServiceXLargeImageName":"","ServiceLargeImageName":"","ServiceSmallImageName":"","ServiceDownloadURL":""}]};</script></body></html>"#;
        let services = parse_service_list(html).unwrap();
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "A");
        assert_eq!(services[1].name, "B");
        assert_eq!(services[1].service_code, "610153");
        assert_eq!(services[1].service_region, "TN");
    }

    #[test]
    fn parse_service_list_missing_assignment_yields_typed_error() {
        let html = "<html><body>no service list here</body></html>";
        let err = parse_service_list(html).unwrap_err();
        assert!(matches!(err, LoginError::GameListServiceListMissing));
    }

    #[test]
    fn parse_service_list_missing_field_defaults_to_empty_string() {
        let html = r#"<script>Services.ServiceList = [{"ServiceCode":"610074"}];</script>"#;
        let services = parse_service_list(html).unwrap();
        assert_eq!(services.len(), 1);
        let s = &services[0];
        assert_eq!(s.service_code, "610074");
        assert_eq!(s.name, "");
        assert_eq!(s.service_region, "");
        assert_eq!(s.large_image_name, "");
    }

    #[test]
    fn parse_service_list_empty_array_yields_empty_vec() {
        let html = r#"<script>Services.ServiceList = [];</script>"#;
        let services = parse_service_list(html).unwrap();
        assert!(services.is_empty());
    }

    #[test]
    fn parse_service_list_old_shape_without_rows_yields_empty_vec() {
        let html = r#"<script>Services.ServiceList = {"NotRows":[]};</script>"#;
        let services = parse_service_list(html).unwrap();
        assert!(services.is_empty());
    }

    #[test]
    fn parse_service_list_invalid_json_in_literal_surfaces_json_error() {
        let html = r#"<script>Services.ServiceList = [not, valid, json];</script>"#;
        let err = parse_service_list(html).unwrap_err();
        assert!(matches!(err, LoginError::Json(_)));
    }

    // -------------------------------------------------------------------------
    // image_base_url
    // -------------------------------------------------------------------------

    #[test]
    fn image_base_url_tw_uses_https() {
        assert_eq!(
            image_base_url(LoginRegion::TW),
            "https://tw.images.beanfun.com/uploaded_images/beanfun_tw/game_zone/"
        );
    }

    #[test]
    fn image_base_url_hk_uses_http_matches_wpf() {
        // WPF MainWindow.xaml.cs L436 hardcodes http:// for HK assets;
        // strict parity required.
        assert_eq!(
            image_base_url(LoginRegion::HK),
            "http://hk.images.beanfun.com/uploaded_images/beanfun/game_zone/"
        );
    }
}
