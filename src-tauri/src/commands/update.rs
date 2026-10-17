//! Application update commands: keep release selection, verification and execution in Rust.
//!
//! Manual checks have a 30-second deadline. Automatic updates authenticate a
//! keyless release manifest before downloading and executing its exact package.

use std::path::Path;
use std::time::{Duration, Instant};

use super::error::CommandError;
use super::state::AppState;
use crate::services::updater::download::{download_bytes, verify_digest, DownloadError};
use crate::services::updater::install::{
    self, InstallError, InstallationKind, PreparedNsis, PreparedPortable,
};
use crate::services::updater::manifest::{
    request_for_release, validate_signed_package, ManifestError, MAX_BUNDLE_BYTES,
    MAX_EXECUTABLE_BYTES, MAX_MANIFEST_BYTES,
};
use crate::services::updater::verification::{self, VerificationError, VerifiedManifest};
use crate::services::updater::{self, Channel, UpdateInfo};
use tauri::{Emitter, WebviewWindow};
use tokio::sync::Mutex;

/// A single backend-owned prepared package prevents arbitrary renderer URLs or file paths.
#[derive(Default)]
pub struct UpdateState(Mutex<Option<PreparedUpdate>>);

enum PreparedUpdate {
    Portable(PreparedPortable),
    Nsis(PreparedNsis),
}

#[derive(Clone, serde::Serialize)]
struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

fn progress<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
) {
    let _ = window.emit_to(
        "main",
        "app-update-progress",
        UpdateProgress {
            phase,
            downloaded,
            total,
        },
    );
}

fn trusted_update_window(label: &str, url: &url::Url, development: bool) -> bool {
    if label != "main" {
        return false;
    }
    let local_app = match (url.scheme(), url.host_str(), url.port()) {
        ("tauri", Some("localhost"), None) => true,
        ("http" | "https", Some("tauri.localhost"), None) => true,
        ("http", Some("localhost" | "127.0.0.1"), Some(1420)) if development => true,
        _ => false,
    };
    local_app && url.username().is_empty() && url.password().is_none()
}

fn require_update_window<R: tauri::Runtime>(window: &WebviewWindow<R>) -> Result<(), CommandError> {
    let url = window
        .url()
        .map_err(|e| CommandError::new("update.not_allowed", e.to_string()))?;
    if !trusted_update_window(window.label(), &url, cfg!(debug_assertions)) {
        return Err(CommandError::new(
            "update.not_allowed",
            "Only the local main window can install updates",
        ));
    }
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err(CommandError::new(
            "update.platform_unsupported",
            "Automatic updates require Windows x64",
        ));
    }
    Ok(())
}

fn installation_error(error: InstallError) -> CommandError {
    let message = error.to_string();
    match error {
        InstallError::OtherInstances => CommandError::new("update.multiple_instances", message),
        InstallError::Signature(_) => CommandError::new("update.signature_invalid", message),
        InstallError::Trust(_) => CommandError::new("update.trust_unavailable", message),
        InstallError::UnsupportedPlatform => {
            CommandError::new("update.platform_unsupported", message)
        }
        _ => CommandError::new("update.install_failed", message),
    }
}

fn manifest_error(error: ManifestError) -> CommandError {
    let message = error.to_string();
    match error {
        ManifestError::ReleaseChanged => CommandError::new("update.release_changed", message),
        ManifestError::InvalidManifest => CommandError::new("update.invalid_manifest", message),
        ManifestError::UnauthenticatedPackage => {
            CommandError::new("update.signature_invalid", message)
        }
    }
}

fn verification_error(error: VerificationError) -> CommandError {
    if error.code == "update.trust_unavailable" {
        CommandError::new("update.trust_unavailable", error.to_string())
    } else {
        CommandError::new("update.signature_invalid", error.to_string())
    }
}

fn download_error(error: DownloadError) -> CommandError {
    let message = error.to_string();
    match error {
        DownloadError::InvalidUrl => CommandError::new("update.invalid_manifest", message),
        DownloadError::TooLarge { .. } => CommandError::new("update.body_too_large", message),
        DownloadError::SizeMismatch | DownloadError::DigestMismatch => {
            CommandError::new("update.signature_invalid", message)
        }
        DownloadError::Network(_) => CommandError::new("update.download_failed", message),
    }
}

/// Recheck the selected release and prepare only a package bound to its keyless attestation.
#[tauri::command]
#[specta::specta]
pub async fn prepare_update<R: tauri::Runtime>(
    window: WebviewWindow<R>,
    app_state: tauri::State<'_, AppState>,
    state: tauri::State<'_, UpdateState>,
    channel: Channel,
    tag_name: String,
) -> Result<(), CommandError> {
    require_update_window(&window)?;
    let mut prepared = state
        .0
        .try_lock()
        .map_err(|_| CommandError::new("update.busy", "Another update operation is running"))?;
    *prepared = None;
    let trust_cache = app_state.storage_root.join("updater-trust");
    let package = tokio::time::timeout(
        Duration::from_secs(1200),
        prepare_package(&window, &trust_cache, channel, &tag_name),
    )
    .await
    .map_err(|_| CommandError::new("update.timeout", "Update preparation timed out"))??;
    *prepared = Some(package);
    Ok(())
}

async fn prepare_package<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    trust_cache: &Path,
    channel: Channel,
    tag_name: &str,
) -> Result<PreparedUpdate, CommandError> {
    install::ensure_single_instance().map_err(installation_error)?;
    let kind = install::detect_installation().map_err(installation_error)?;
    // Installation uses official HTTPS metadata, never the manual check's proxy chain.
    let releases =
        updater::fetch_releases_at(updater::GH_API_RELEASES_URL, "Beanfun automatic updater")
            .await?;
    let release = updater::select_release(&releases, channel).ok_or_else(|| {
        CommandError::new(
            "update.release_changed",
            "No release is available on this channel",
        )
    })?;
    let request = request_for_release(release, tag_name, env!("CARGO_PKG_VERSION"), kind)
        .map_err(manifest_error)?;
    let (manifest_bytes, bundle_bytes) = tokio::try_join!(
        download_bytes(
            &request.endpoint,
            MAX_MANIFEST_BYTES,
            None,
            Duration::from_secs(30),
            |_, _| {}
        ),
        download_bytes(
            &request.bundle_url,
            MAX_BUNDLE_BYTES,
            None,
            Duration::from_secs(30),
            |_, _| {}
        ),
    )
    .map_err(download_error)?;
    let trust_root = verification::fetch_trust_root(trust_cache)
        .await
        .map_err(verification_error)?;
    progress(window, "verifying", 0, None);
    let (verified, trust_root, manifest_bytes, bundle_bytes) =
        tauri::async_runtime::spawn_blocking(move || {
            let verified = VerifiedManifest::verify(&manifest_bytes, &bundle_bytes, &trust_root)?;
            Ok::<_, VerificationError>((verified, trust_root, manifest_bytes, bundle_bytes))
        })
        .await
        .map_err(|e| CommandError::new("update.prepare_failed", e.to_string()))?
        .map_err(verification_error)?;
    // Keep the authenticated document tied to the exact tag the user approved.
    request
        .validate(&manifest_bytes, verified.source_commit())
        .map_err(manifest_error)?;
    let package =
        validate_signed_package(&manifest_bytes, &verified, kind, env!("CARGO_PKG_VERSION"))
            .map_err(manifest_error)?;
    let mut last_progress = Instant::now();
    progress(window, "downloading", 0, Some(package.size));
    let bytes = download_bytes(
        &package.url,
        MAX_EXECUTABLE_BYTES,
        Some(package.size),
        Duration::from_secs(1200),
        |downloaded, total| {
            if last_progress.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                progress(window, "downloading", downloaded, total);
                last_progress = Instant::now();
            }
        },
    )
    .await
    .map_err(download_error)?;
    progress(window, "verifying", bytes.len() as u64, Some(package.size));
    verify_digest(&bytes, &package.sha256).map_err(download_error)?;
    let length = bytes.len() as u64;
    let trust_cache = trust_cache.to_owned();
    let prepared = tauri::async_runtime::spawn_blocking(move || match kind {
        InstallationKind::Portable => install::prepare_portable(
            &bytes,
            &manifest_bytes,
            &bundle_bytes,
            &trust_root,
            &trust_cache,
        )
        .map(PreparedUpdate::Portable),
        InstallationKind::Nsis => install::prepare_nsis(
            &bytes,
            &manifest_bytes,
            &bundle_bytes,
            &trust_root,
            &trust_cache,
        )
        .map(PreparedUpdate::Nsis),
    })
    .await
    .map_err(|e| CommandError::new("update.prepare_failed", e.to_string()))?
    .map_err(installation_error)?;
    progress(window, "ready", length, Some(length));
    Ok(prepared)
}

/// Launch only a verified package retained by this process, then exit after a successful hand-off.
#[tauri::command]
#[specta::specta]
pub async fn install_update<R: tauri::Runtime>(
    window: WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, UpdateState>,
) -> Result<(), CommandError> {
    require_update_window(&window)?;
    let mut prepared = state
        .0
        .try_lock()
        .map_err(|_| CommandError::new("update.busy", "Another update operation is running"))?;
    install::ensure_single_instance().map_err(installation_error)?;
    let package = prepared.take().ok_or_else(|| {
        CommandError::new(
            "update.prepare_failed",
            "No verified update is ready to install",
        )
    })?;
    progress(&window, "installing", 0, None);
    tauri::async_runtime::spawn_blocking(move || match package {
        PreparedUpdate::Portable(portable) => portable.launch(),
        PreparedUpdate::Nsis(nsis) => nsis.launch(),
    })
    .await
    .map_err(|e| CommandError::new("update.install_failed", e.to_string()))?
    .map_err(installation_error)?;
    app.exit(0);
    Ok(())
}

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
    fn authentication_failures_are_not_reported_as_network_failures() {
        for error in [DownloadError::SizeMismatch, DownloadError::DigestMismatch] {
            assert_eq!(download_error(error).code, "update.signature_invalid");
        }
        assert_eq!(
            download_error(DownloadError::InvalidUrl).code,
            "update.invalid_manifest"
        );
        assert_eq!(
            download_error(DownloadError::TooLarge { limit: 100 }).code,
            "update.body_too_large"
        );
        assert_eq!(
            manifest_error(ManifestError::UnauthenticatedPackage).code,
            "update.signature_invalid"
        );
        assert_eq!(
            verification_error(VerificationError {
                code: "update.trust_unavailable",
                message: "offline".into()
            })
            .code,
            "update.trust_unavailable"
        );
        assert_eq!(
            verification_error(VerificationError {
                code: "update.signature_invalid",
                message: "untrusted signer".into()
            })
            .code,
            "update.signature_invalid"
        );
    }

    #[test]
    fn update_installation_is_limited_to_the_local_main_window() {
        for url in [
            "tauri://localhost/",
            "http://tauri.localhost/#/about",
            "https://tauri.localhost/",
        ] {
            assert!(trusted_update_window("main", &url.parse().unwrap(), false));
            assert!(!trusted_update_window(
                "login",
                &url.parse().unwrap(),
                false
            ));
        }
        for url in [
            "https://github.com/",
            "https://tauri.localhost.evil.test/",
            "file:///tmp/index.html",
            "http://localhost:1420/",
        ] {
            assert!(!trusted_update_window("main", &url.parse().unwrap(), false));
        }
        assert!(trusted_update_window(
            "main",
            &"http://localhost:1420/".parse().unwrap(),
            true
        ));
    }

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
