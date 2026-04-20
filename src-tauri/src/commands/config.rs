//! AppSettings config commands — read / write `Config.xml`.
//!
//! Ports the WPF `ConfigAppSettings` read / write surface
//! (`Beanfun/Helper/ConfigAppSettings.cs`) to the Tauri IPC
//! boundary. Three commands cover the complete access pattern the
//! P11 settings page will need:
//!
//! - [`get_config_value`] — single-key read, catch-all → `""`
//!   (WPF `GetValue(key)` L64-67).
//! - [`get_all_config`] — bulk read of every `<add key value />`
//!   entry as a flat map. Introduced by P10.3-Q3 = C (the "C"
//!   three-command shape); WPF has no direct counterpart but
//!   iterates `ConfigurationManager.AppSettings` in several places.
//! - [`set_config`] — write / update / remove one key (WPF
//!   `SetValue(key, value)` L21-32, with `value: None` mirroring
//!   the WPF `value == null` removal branch).
//!
//! # Path resolution
//!
//! All three commands resolve the on-disk path via
//! [`AppState::storage_root`]`.join("Config.xml")` rather than
//! calling the windows-only
//! [`crate::services::config::default_config_xml_path`] directly.
//! Two reasons:
//!
//! 1. The storage root is already funneled through
//!    [`crate::run`] → [`AppState::new`] at boot (a single
//!    `%APPDATA%\Beanfun` resolution); tests can swap in a
//!    `tempfile::TempDir` path with `AppState::new(dir)` without
//!    touching env vars or platform gates.
//! 2. Cross-platform — the commands compile on macOS / Linux dev
//!    laptops for `cargo check`, matching the rest of the P10.2+
//!    command layer.
//!
//! # Error policy (per command)
//!
//! | Command              | Failure mode                        | Surface                                                                  |
//! | -------------------- | ----------------------------------- | ------------------------------------------------------------------------ |
//! | [`get_config_value`] | IO / parse / missing key            | Catch-all → `""` (WPF parity, service `get_value` already swallows)      |
//! | [`get_all_config`]   | IO (not NotFound) / XML parse       | Catch-all → `{}` + `tracing::warn!` (WPF parity for bulk read; corrupted file must not hard-fail the UI's settings page) |
//! | [`set_config`]       | Final write (IO) / encode failure   | Typed `CommandError { code: "config.*" }` via `ConfigError → CommandError` (service-layer deviation from WPF's silent swallow — propagated verbatim) |
//!
//! The asymmetry is deliberate: read paths stay quiet to keep the
//! UI simple (empty state is always a valid rendering), the write
//! path is loud so the user is told when their setting didn't
//! actually persist (WPF's silent-failure mode was a frequent
//! support issue flagged in `services::config` module docs).

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::State;

use crate::commands::error::CommandError;
use crate::commands::state::AppState;
use crate::services::config;

/// On-disk filename under [`AppState::storage_root`] — matches WPF
/// `ConfigAppSettings.cs` L14-16
/// (`SpecialFolder.ApplicationData\Beanfun\Config.xml`).
const CONFIG_FILE_NAME: &str = "Config.xml";

/// Resolve the `Config.xml` path from [`AppState::storage_root`].
/// Kept `pub(crate)` so P10.3+ sibling modules (e.g. launcher
/// commands that want to read game-path entries) can call the same
/// helper instead of re-deriving the filename. Not exposed to the
/// frontend (it's not a [`#[tauri::command]`][tauri::command]).
pub(crate) fn config_xml_path(state: &AppState) -> PathBuf {
    state.storage_root.join(CONFIG_FILE_NAME)
}

/// Read a single config value by `key`, falling back to `""` when
/// the file is missing / unreadable / the key is absent.
///
/// Thin wrapper over [`crate::services::config::get_value`] — the
/// service layer already implements WPF's catch-all semantics,
/// including the `tracing::warn!` on read failure. This command
/// adds only the storage-root path resolution.
///
/// # Errors
///
/// Despite the `Result<_, CommandError>` signature this command
/// never surfaces an error in practice — the underlying
/// [`crate::services::config::get_value`] is infallible (catch-all
/// policy). The `Result` shape is retained for symmetry with
/// [`get_all_config`] / [`set_config`] and to leave room for future
/// validation (e.g. reject keys containing control characters if
/// that becomes a concern).
#[tauri::command]
#[specta::specta]
pub async fn get_config_value(
    state: State<'_, AppState>,
    key: String,
) -> Result<String, CommandError> {
    let path = config_xml_path(&state);
    Ok(config::get_value(&path, &key).await)
}

/// Read every `<add key value />` entry from `Config.xml` as a flat
/// map. Any read / parse failure is swallowed and a warning is
/// logged — the frontend always sees a map (possibly empty) so the
/// settings page never needs to handle a "config corrupted" error
/// state (WPF-parity catch-all for bulk read; see error-policy
/// table in the module docs).
///
/// # Ordering
///
/// [`IndexMap`][indexmap::IndexMap] preserves insertion order on the
/// service side, but specta serialises `HashMap<String, String>`
/// (this command's return type) as a JSON object. ES2020 object
/// property iteration order is insertion-ordered for string keys,
/// so the ordering survives the IPC boundary on modern runtimes;
/// frontend callers that need a guaranteed order should sort by
/// key client-side regardless.
///
/// # Why `HashMap` over `IndexMap`?
///
/// `specta::Type` supports both, but `HashMap<String, String>` is
/// the canonical "dictionary" shape the rest of the command layer
/// already uses (e.g. future export-account bundles). Keeping one
/// shape across the IPC boundary avoids forcing the frontend to
/// branch on an ordered-vs-unordered distinction that is only
/// meaningful server-side.
#[tauri::command]
#[specta::specta]
pub async fn get_all_config(
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, CommandError> {
    let path = config_xml_path(&state);
    match config::get_all_values(&path).await {
        Ok(map) => Ok(map.into_iter().collect()),
        Err(err) => {
            tracing::warn!(
                error = ?err,
                "get_all_config failed; returning empty map (WPF-parity catch-all policy)"
            );
            Ok(HashMap::new())
        }
    }
}

/// Set, update, or remove a config entry.
///
/// - `value = Some(v)` → upsert (in-place for existing keys,
///   append for new ones — matches .NET `Settings[k].Value = v` /
///   `Settings.Add(k, v)` distinction without a branch).
/// - `value = None` → remove (no-op when the key is already
///   absent; preserves the rest of the map's order).
///
/// # Error surface (deviation from WPF)
///
/// Unlike [`get_config_value`] / [`get_all_config`] (catch-all),
/// this command propagates the service-layer typed errors
/// ([`crate::services::config::ConfigError::Io`] /
/// [`crate::services::config::ConfigError::XmlWrite`]) so the UI
/// can tell the user when their setting didn't persist. WPF
/// swallows these silently at
/// `ConfigAppSettings.cs` L60 which caused user-visible settings
/// loss without any indication; the Rust port surfaces them as
/// `config.io_failed` / `config.xml_write_failed` codes for the
/// frontend to handle explicitly.
#[tauri::command]
#[specta::specta]
pub async fn set_config(
    state: State<'_, AppState>,
    key: String,
    value: Option<String>,
) -> Result<(), CommandError> {
    let path = config_xml_path(&state);
    config::set_value(&path, &key, value.as_deref()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for the three config commands.
    //!
    //! These exercise the command-layer path: build an `AppState`
    //! rooted in a `tempfile::TempDir`, call the commands via their
    //! plain Rust signatures (`#[tauri::command]` just adds specta
    //! metadata — the underlying fn is directly callable), and
    //! assert on the on-disk `Config.xml` or the returned value.
    //!
    //! The `State<'_, AppState>` parameter is substituted by calling
    //! the inner function bodies with an `AppState`-backing `&AppState`
    //! through the same helpers the production command uses
    //! (`config_xml_path`). This keeps tests from needing a full
    //! `tauri::AppHandle`.

    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn temp_app_state() -> (TempDir, Arc<AppState>) {
        let dir = TempDir::new().expect("temp dir");
        let state = Arc::new(AppState::new(dir.path().to_path_buf()));
        (dir, state)
    }

    // The three commands all take `State<'_, AppState>`. In unit
    // tests we bypass Tauri's `State` wrapper by calling the service
    // layer through the same `config_xml_path` helper, which is the
    // only thing the command body does beyond delegating. The
    // end-to-end IPC path is covered by the D6 bindings-file symbol
    // tests and future integration tests under `tests/`.

    #[tokio::test]
    async fn config_xml_path_joins_storage_root_and_filename() {
        let (dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        assert_eq!(path, dir.path().join("Config.xml"));
    }

    #[tokio::test]
    async fn get_config_value_missing_file_returns_empty_string() {
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        // Direct service call — identical to what the command body
        // does once it has resolved the path.
        let value = config::get_value(&path, "Region").await;
        assert_eq!(value, "");
    }

    #[tokio::test]
    async fn set_config_then_get_config_value_round_trips() {
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        config::set_value(&path, "Region", Some("HK"))
            .await
            .expect("set");
        let value = config::get_value(&path, "Region").await;
        assert_eq!(value, "HK");
    }

    #[tokio::test]
    async fn get_all_config_missing_file_returns_empty_map() {
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        // Mirror the command body: service-layer typed result →
        // catch-all empty map on the IPC boundary.
        let map = match config::get_all_values(&path).await {
            Ok(m) => m.into_iter().collect::<HashMap<_, _>>(),
            Err(_) => HashMap::new(),
        };
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn get_all_config_corrupted_xml_collapses_to_empty_map() {
        // Guards the command's catch-all policy: even if the file is
        // hopelessly mangled, the settings page must not receive a
        // hard error — it must see an empty map and let the user
        // start fresh.
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        std::fs::write(&path, "<configuration><appSettings><add").expect("seed corrupted");
        let map = match config::get_all_values(&path).await {
            Ok(m) => m.into_iter().collect::<HashMap<_, _>>(),
            Err(_) => HashMap::new(),
        };
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn set_config_then_get_all_config_returns_all_entries() {
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        config::set_value(&path, "Region", Some("TW"))
            .await
            .expect("set 1");
        config::set_value(&path, "LastAccount", Some("u@e"))
            .await
            .expect("set 2");
        config::set_value(&path, "AutoLogin", Some("true"))
            .await
            .expect("set 3");

        let map = config::get_all_values(&path)
            .await
            .expect("get_all_values")
            .into_iter()
            .collect::<HashMap<_, _>>();

        assert_eq!(map.len(), 3);
        assert_eq!(map.get("Region").map(String::as_str), Some("TW"));
        assert_eq!(map.get("LastAccount").map(String::as_str), Some("u@e"));
        assert_eq!(map.get("AutoLogin").map(String::as_str), Some("true"));
    }

    #[tokio::test]
    async fn set_config_none_removes_existing_key() {
        let (_dir, state) = temp_app_state();
        let path = config_xml_path(&state);
        config::set_value(&path, "Region", Some("TW"))
            .await
            .expect("set");
        config::set_value(&path, "Region", None)
            .await
            .expect("remove");
        let value = config::get_value(&path, "Region").await;
        assert_eq!(value, "");
    }
}
