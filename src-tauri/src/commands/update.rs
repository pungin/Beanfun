//! Application-update command — thin wrapper over
//! [`crate::services::updater::check_update`].
//!
//! Manual checks return typed failures rather than treating a failed request
//! as an up-to-date installation. A 30-second deadline bounds the entire
//! probe-and-fetch operation, including all fallback proxies.
//!
//! # Local-version source
//!
//! `local_version` defaults to `env!("CARGO_PKG_VERSION")` at
//! compile time — i.e. the `version` field of
//! `src-tauri/Cargo.toml`. Frontend callers can
//! override this to probe for a specific baseline (e.g. diagnostic
//! "what's newer than X.Y.Z?" queries); production callers should
//! pass `None` and let the backend self-report.
//!
//! Note: while the Beanfun crate is still at `0.1.0` and the
//! GitHub releases are tagged under the legacy `v5.8.3.<timestamp>`
//! scheme, the newer-than comparator (`is_newer_version`) will
//! effectively always fire, so the command will always return
//! `Some(_)` when the network path succeeds. This is expected
//! behaviour until the P12 release pipeline aligns crate versions
//! with the upstream tag scheme.

use super::error::CommandError;
use crate::services::updater::{self, Channel, UpdateInfo};

/// Check whether a newer Beanfun release is available on the
/// upstream GitHub releases feed.
///
/// Returns `Ok(Some(UpdateInfo))` for a newer release, `Ok(None)` when
/// up to date, and a typed error when the check fails or exceeds its deadline.
///
/// # Parameters
///
/// - `channel` — `Stable` to filter out prereleases, `Beta` to
///   accept them (matches the WPF `updateChannel` config value).
///   Frontend settings pages can bind to a single string
///   (`"Stable"` / `"Beta"`) thanks to `Channel`'s `Serialize`
///   derive using unit-variant form.
/// - `local_version` — optional override. When `None` the backend
///   self-reports `env!("CARGO_PKG_VERSION")`.
///
/// # Background-refresh caching
///
/// The service layer caches the proxy probe result in an
/// `OnceLock`, so repeated calls within a single process pay the
/// HEAD-probe cost only once. The command does not re-probe; if a
/// forced re-probe ever becomes necessary (e.g. after a network
/// reconfiguration), [`crate::services::updater::check_update_at`]
/// is the escape hatch — exposing that through the command surface
/// is YAGNI until a user-visible feature requests it.
#[tauri::command]
#[specta::specta]
pub async fn check_update(
    channel: Channel,
    local_version: Option<String>,
) -> Result<Option<UpdateInfo>, CommandError> {
    let default_version = env!("CARGO_PKG_VERSION");
    let version = local_version.as_deref().unwrap_or(default_version);
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        updater::check_update(channel, version),
    )
    .await
    .map_err(|_| CommandError::new("update.timeout", "Update check timed out"))?
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_serializes_as_bare_string() {
        // Frontend settings pages bind to `"Stable"` / `"Beta"`
        // strings (matching WPF `updateChannel` config values).
        // This pin guards against an accidental
        // `#[serde(tag = "kind")]` that would wrap the value in an
        // object and silently break the settings form.
        let stable = serde_json::to_string(&Channel::Stable).expect("serialize");
        let beta = serde_json::to_string(&Channel::Beta).expect("serialize");
        assert_eq!(stable, "\"Stable\"");
        assert_eq!(beta, "\"Beta\"");
    }

    #[test]
    fn channel_deserializes_from_bare_string() {
        let stable: Channel = serde_json::from_str("\"Stable\"").expect("deserialize");
        let beta: Channel = serde_json::from_str("\"Beta\"").expect("deserialize");
        assert_eq!(stable, Channel::Stable);
        assert_eq!(beta, Channel::Beta);
    }

    #[test]
    fn update_info_serializes_all_fields() {
        // Guards the IPC contract for `UpdateInfo` — every field
        // the frontend expects must survive the serde round-trip
        // into JSON. We don't round-trip back (no `Deserialize`
        // derive by design — the frontend consumes but never
        // produces `UpdateInfo`).
        let info = UpdateInfo {
            new_version_display: "5.8.3(2604011114)".into(),
            body: "## Changelog\n- fix A".into(),
            download_url: "https://example.com/asset".into(),
            tag_name: "v5.8.3.2604011114".into(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("new_version_display"));
        assert!(json.contains("5.8.3(2604011114)"));
        assert!(json.contains("download_url"));
        assert!(json.contains("tag_name"));
        assert!(json.contains("body"));
    }
}
