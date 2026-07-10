//! System-level commands — `version` / `ping` / `open_url`.
//!
//! Smoke commands (`version`, `ping`) exercise the full IPC pipeline
//! (frontend → Tauri dispatcher → `#[tauri::command]` → specta
//! binding → serde round-trip) end-to-end without involving any
//! Beanfun-specific domain logic; getting them green at
//! infrastructure commit time is the fastest way to surface wiring
//! regressions. [`open_url`] is the first real functional command in
//! this module — a thin wrapper over
//! [`crate::services::system::open_url()`] that ports three WPF
//! `Process.Start(..., UseShellExecute = true)` sites
//! (`ApplicationUpdater` download link, `About` GitHub/mailto
//! buttons, `MainWindow.runGame` update prompt) under one uniform
//! `system.*` surface.
//!
//! # Design notes
//!
//! - [`version`] is **synchronous + infallible**, the simplest shape
//!   Tauri accepts. It proves the command dispatcher and specta
//!   binding work for struct return types.
//! - [`ping`] is **async + fallible + blocks inside
//!   [`tokio::task::spawn_blocking`]** — the pattern every Win32
//!   wrapper in P10.2+ will use (Win32 APIs are overwhelmingly
//!   synchronous; running them on the async executor directly stalls
//!   the reactor). A 60 ms sleep is enough to prove an `await` point
//!   actually suspends without being a noticeable nuisance during
//!   interactive testing.
//! - [`open_url`] delegates to
//!   [`crate::services::system::open_url()`] which handles scheme
//!   allowlisting + `spawn_blocking` isolation internally; the
//!   command stays a three-line thin wrapper (P10.3-Q1 = A:
//!   "command = IPC boundary, service = business logic"). We do
//!   **not** use `tauri-plugin-opener` from the backend because its
//!   Rust API requires `AppHandle`, which would re-couple the
//!   service layer to the Tauri runtime (see
//!   [`crate::services::system`] module doc).
//!
//! # `system.*` codes introduced here
//!
//! - `system.spawn_blocking_failed` — shared with
//!   [`crate::services::system::SystemError::SpawnBlockingFailed`];
//!   emitted both from the ad-hoc [`ping`] path (no service error
//!   intermediate) and from [`open_url`] via the `SystemError →
//!   CommandError` impl. See
//!   [module-level docs][crate::commands::error] for the full
//!   `system.*` code table.
//! - `system.invalid_url` / `system.open_url_failed` — minted by the
//!   `SystemError → CommandError` conversion in
//!   [`crate::commands::error`]; this module adds no extra codes.

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::error::CommandError;
use crate::commands::state::AppState;
use crate::services::system;
use crate::tray;

/// Compile-time build metadata returned by [`version`].
///
/// `app` is this crate's own version (derived from `Cargo.toml` via
/// the `CARGO_PKG_VERSION` environment variable Cargo sets at build
/// time); `tauri` is the Tauri framework version the binary was
/// compiled against — useful in bug reports to confirm the IPC
/// dispatcher expected by the frontend matches what's running.
#[derive(Debug, Clone, Serialize, Type)]
pub struct VersionInfo {
    /// Our own crate version (`env!("CARGO_PKG_VERSION")` at compile
    /// time).
    pub app: String,
    /// Tauri framework version ([`tauri::VERSION`] at compile time).
    pub tauri: String,
}

/// Runtime visual-environment hints consumed by the frontend before
/// mounting.
///
/// Kept intentionally narrow: the frontend only needs to know when
/// the host cannot safely use the Win11-style translucent glass recipe
/// over a transparent Tauri window.
#[derive(Debug, Clone, Serialize, Type)]
pub struct WindowVisualEnvironment {
    /// `true` on Windows builds whose build number is below 22000
    /// (Windows 10). Those hosts need an opaque CSS fallback because
    /// DWM/WebView2 transparency does not match Windows 11's Mica-like
    /// composition.
    pub is_windows_10: bool,
}

/// Return the static build metadata. Infallible; no parameters; no
/// state.
///
/// Intended as the simplest possible Tauri command — if this
/// doesn't round-trip correctly, nothing else will. Also serves as
/// the canonical example of a sync `#[tauri::command]` with a
/// structured return type for future documentation.
#[tauri::command]
#[specta::specta]
pub fn version() -> VersionInfo {
    VersionInfo {
        app: env!("CARGO_PKG_VERSION").to_string(),
        tauri: tauri::VERSION.to_string(),
    }
}

/// Return host visual-environment flags used by the web UI.
///
/// This is deliberately separate from [`version`]: build metadata is
/// static, while this value depends on the user's OS. It is still
/// synchronous and infallible so the frontend can query it before
/// mounting without adding an error surface to startup.
#[tauri::command]
#[specta::specta]
pub fn window_visual_environment() -> WindowVisualEnvironment {
    WindowVisualEnvironment {
        is_windows_10: crate::is_windows_10(),
    }
}

/// Round-trip an input string through a blocking worker thread.
///
/// Exercises the canonical Win32-wrapping pattern: the closure runs
/// on a [`tokio::task::spawn_blocking`] pool worker (not the reactor
/// thread), sleeps for 60 ms to prove the `await` point genuinely
/// suspends, then returns `"pong: {input}"`.
///
/// Failure path: if the blocking task panics or is cancelled the
/// [`tokio::task::JoinError`] is mapped to
/// `system.spawn_blocking_failed`. Should never happen in steady
/// state — the closure is a sleep + `format!` with no fallible ops —
/// but the code path needs to exist so the pattern is complete for
/// the real Win32 wrappers in P10.2+.
#[tauri::command]
#[specta::specta]
pub async fn ping(message: String) -> Result<String, CommandError> {
    tokio::task::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        format!("pong: {message}")
    })
    .await
    .map_err(|err| {
        CommandError::new(
            "system.spawn_blocking_failed",
            format!("blocking task panicked or was cancelled: {err}"),
        )
    })
}

/// Open `url` in the user's default handler (browser for http/https,
/// mail client for mailto).
///
/// Thin wrapper over [`crate::services::system::open_url()`] — the
/// service layer enforces the scheme allowlist (`http` / `https` /
/// `mailto`) and wraps the synchronous [`open::that`] call in
/// [`tokio::task::spawn_blocking`], so this command stays a
/// single-line delegation (P10.3-Q1 = A decision: command layer is
/// strictly the IPC boundary, business logic lives in `services/`).
///
/// # Errors
///
/// - `system.invalid_url` — URL is empty, missing a scheme, or uses
///   a scheme outside the allowlist. Rejected before any OS call.
/// - `system.open_url_failed` — OS opener (`ShellExecuteW` /
///   `LSOpenCFURLRef` / `xdg-open`) returned an I/O error.
/// - `system.spawn_blocking_failed` — the blocking task hosting
///   [`open::that`] panicked or was cancelled (should not happen in
///   steady state).
#[tauri::command]
#[specta::specta]
pub async fn open_url(url: String) -> Result<(), CommandError> {
    system::open_url(&url).await?;
    Ok(())
}

/// Minimize the main window, honouring the `minimize_to_tray`
/// config setting.
///
/// Driven by the custom `TitleBar` minimize button in the
/// frontend. Replaces the legacy `appWindow.minimize()` direct call
/// because the post-PR-228 borderless + transparent + non-resizable
/// window no longer reliably produces the `WindowEvent::Resized(0, 0)`
/// signal Windows used to fire on minimize, which broke the
/// fallback path in [`crate::tray::handle_minimize_to_tray`].
///
/// Behaviour:
///
/// - If `minimize_to_tray = true` in `Config.xml` **and** the tray
///   icon was created successfully at boot — hide the window and
///   reveal the tray icon (delegated to [`tray::hide_to_tray`] so
///   the side effect stays single-sourced with the window-event
///   fallback path).
/// - Otherwise — fall through to a normal `window.minimize()`.
///
/// # Errors
///
/// - `system.window_not_found` — the `main` webview window is not
///   currently registered with the app handle. Should never happen
///   in steady state (the window is created at boot).
/// - `system.minimize_failed` — Tauri's `window.minimize()` returned
///   an error from the underlying OS call.
#[tauri::command]
#[specta::specta]
pub async fn minimize_main_window<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, AppState>,
    tray_state: State<'_, tray::TrayState>,
) -> Result<(), CommandError> {
    let storage_root = state.storage_root.clone();
    let should_tray = tray::is_minimize_to_tray_enabled(&storage_root).await;

    if should_tray {
        // Snapshot the ID under the lock then drop the guard before
        // any further work — keeps the critical section to a memcpy.
        let tray_id = tray_state.0.lock().unwrap().clone();
        if let Some(tray_id) = tray_id {
            tray::hide_to_tray(&app_handle, &tray_id);
            return Ok(());
        }
        // Tray creation failed at boot (logged there). Fall through
        // to a normal minimize so the button still does *something*
        // useful instead of becoming a silent no-op.
        tracing::warn!(
            step = "Tray.MinimizeFallthrough",
            "minimize_to_tray=true but tray icon unavailable; minimizing normally"
        );
    }

    let win = app_handle.get_webview_window("main").ok_or_else(|| {
        CommandError::new(
            "system.window_not_found",
            "main webview window is not currently registered",
        )
    })?;
    win.minimize().map_err(|err| {
        CommandError::new(
            "system.minimize_failed",
            format!("failed to minimize main window: {err}"),
        )
    })?;
    Ok(())
}

/// Filename the `tauri-plugin-window-state` plugin persists the window
/// position into, under `app_config_dir()` (Windows:
/// `%APPDATA%\tw.beanfun.app\.window-state.json`). The plugin's documented
/// default; kept as a named constant so [`reset_window_position`] and the
/// plugin stay in sync if it is ever customised in `lib.rs`.
const WINDOW_STATE_FILENAME: &str = ".window-state.json";

/// Reset the main window to a safe, on-screen position.
///
/// `tauri-plugin-window-state` persists the window **position** (see
/// `lib.rs`), so a window dragged off-screen — or stranded by a disconnected
/// monitor — restores to that bad spot on the next launch. This re-centers the
/// live window and deletes the saved state file so the next launch also starts
/// centered.
///
/// # Errors
///
/// - `system.window_not_found` — the `main` window is not registered.
/// - `system.reset_window_failed` — `window.center()` returned an OS error.
#[tauri::command]
#[specta::specta]
pub async fn reset_window_position<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
) -> Result<(), CommandError> {
    let win = app_handle.get_webview_window("main").ok_or_else(|| {
        CommandError::new(
            "system.window_not_found",
            "main webview window is not currently registered",
        )
    })?;
    win.center().map_err(|err| {
        CommandError::new(
            "system.reset_window_failed",
            format!("failed to center main window: {err}"),
        )
    })?;

    // Best-effort: drop the persisted position so a future launch starts from
    // the centered spot rather than restoring the old (possibly off-screen)
    // one. Non-fatal — the live window is already centered, and the plugin
    // re-saves the good position on the next clean exit.
    if let Ok(config_dir) = app_handle.path().app_config_dir() {
        let state_file = config_dir.join(WINDOW_STATE_FILENAME);
        match std::fs::remove_file(&state_file) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(
                step = "ResetWindow.RemoveState",
                error = %err,
                "could not delete persisted window-state file (non-fatal)"
            ),
        }
    }
    Ok(())
}

/// Clear the main webview's browsing data (the WebView2 cache on Windows).
///
/// WebView2 persists its cache — including the page **zoom factor** — in the
/// user-data folder (`%APPDATA%\tw.beanfun.app\EBWebView`). A stale/corrupt
/// cache can leave the UI mis-rendered; this clears it. The app's login
/// session lives in the backend HTTP client's cookie jar (not the webview), so
/// this does **not** sign the user out. A restart is recommended so the cleared
/// state fully takes effect.
///
/// # Errors
///
/// - `system.window_not_found` — the `main` window is not registered.
/// - `system.clear_cache_failed` — the WebView2 clear call returned an error.
#[tauri::command]
#[specta::specta]
pub async fn clear_webview2_cache<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
) -> Result<(), CommandError> {
    let win = app_handle.get_webview_window("main").ok_or_else(|| {
        CommandError::new(
            "system.window_not_found",
            "main webview window is not currently registered",
        )
    })?;
    win.clear_all_browsing_data().map_err(|err| {
        CommandError::new(
            "system.clear_cache_failed",
            format!("failed to clear webview browsing data: {err}"),
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_cargo_and_tauri_versions() {
        let info = version();
        assert_eq!(info.app, env!("CARGO_PKG_VERSION"));
        assert!(
            !info.tauri.is_empty(),
            "tauri::VERSION must not be empty at build time"
        );
    }

    #[test]
    fn window_visual_environment_reports_non_windows_as_not_windows_10() {
        let info = window_visual_environment();
        #[cfg(not(target_os = "windows"))]
        assert!(!info.is_windows_10);
        #[cfg(target_os = "windows")]
        let _ = info.is_windows_10;
    }

    #[tokio::test]
    async fn ping_echoes_message_with_pong_prefix() {
        let response = ping("hello".to_string())
            .await
            .expect("ping should succeed under normal conditions");
        assert_eq!(response, "pong: hello");
    }

    #[tokio::test]
    async fn ping_round_trips_unicode_payload() {
        // The blocking worker `format!` should not lose non-ASCII
        // bytes; this guards against accidental ASCII-only handling
        // in the pattern every Win32 wrapper in P10.2+ inherits.
        let response = ping("你好".to_string())
            .await
            .expect("ping should handle unicode");
        assert_eq!(response, "pong: 你好");
    }

    // -----------------------------------------------------------------
    // open_url — error-path symbol tests. The happy path (actually
    // launching the default browser) is covered by the service-layer
    // `services::system::open_url` tests plus future integration
    // tests; here we only assert the command correctly surfaces
    // scheme-allowlist rejection through the `SystemError →
    // CommandError` path so the IPC contract stays intact.
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn open_url_rejects_empty_url_as_system_invalid_url() {
        let err = open_url(String::new())
            .await
            .expect_err("empty URL must be rejected by the service layer");
        assert_eq!(err.code, "system.invalid_url");
    }

    #[tokio::test]
    async fn open_url_rejects_file_scheme_as_system_invalid_url() {
        let err = open_url("file:///C:/Windows/System32/cmd.exe".to_string())
            .await
            .expect_err("file:// must be rejected");
        assert_eq!(err.code, "system.invalid_url");
    }

    #[tokio::test]
    async fn open_url_rejects_javascript_scheme_as_system_invalid_url() {
        let err = open_url("javascript:alert(1)".to_string())
            .await
            .expect_err("javascript: must be rejected");
        assert_eq!(err.code, "system.invalid_url");
    }
}
