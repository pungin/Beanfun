//! System tray icon — mirrors WPF `MainWindow.xaml.cs` L81-86 / L845-857.
//!
//! WPF creates a `NotifyIcon` (initially hidden). When the user
//! minimizes the main window **and** the `minimize_to_tray` config
//! checkbox is checked, the window hides and the tray icon appears.
//! Left-clicking the tray icon restores the window and hides the icon.
//!
//! # Two entry points into "hide to tray"
//!
//! 1. **Frontend command** ([`crate::commands::system::minimize_main_window`])
//!    — primary path. The custom `TitleBar` minimize button calls
//!    this command, which reads the `minimize_to_tray` config and
//!    either hides+shows-tray or falls through to a normal
//!    `window.minimize()`. This path is reliable regardless of
//!    `decorations` / `transparent` / `resizable` settings.
//! 2. **Window event listener** ([`handle_minimize_to_tray`]) —
//!    defensive fallback. Tauri v2 has no dedicated
//!    `WindowEvent::Minimized` variant; on Windows the OS *usually*
//!    fires `Resized(0, 0)` when a window is minimized, but this
//!    signal is unreliable for borderless / transparent windows
//!    (the reason path 1 was added). Kept so any external trigger
//!    that does end up minimizing the window (e.g., taskbar
//!    interaction) still respects the config.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId};
use tauri::{App, AppHandle, Manager, PhysicalSize, WindowEvent, Wry};

use crate::services::config;

const CONFIG_FILE_NAME: &str = "Config.xml";
const MINIMIZE_TO_TRAY_KEY: &str = "minimize_to_tray";

/// Tauri-managed handle to the tray icon ID.
///
/// Wraps the same `Arc<Mutex<Option<TrayIconId>>>` the boot path in
/// [`crate::run`] hands to the `setup` and `on_window_event`
/// closures, so commands that need to drive the tray (e.g.
/// [`crate::commands::system::minimize_main_window`]) can reach the
/// ID via `State<'_, TrayState>` without re-discovering it from the
/// `AppHandle`.
///
/// `Option<TrayIconId>` because tray creation is allowed to fail
/// non-fatally — see [`build_tray`].
pub struct TrayState(pub Arc<Mutex<Option<TrayIconId>>>);

fn config_xml_path(storage_root: &Path) -> PathBuf {
    storage_root.join(CONFIG_FILE_NAME)
}

/// Read the `minimize_to_tray` checkbox state from `Config.xml`.
///
/// `pub(crate)` so command-layer code (e.g.
/// [`crate::commands::system::minimize_main_window`]) shares the
/// same parsing logic as the window-event fallback path
/// ([`handle_minimize_to_tray`]).
pub(crate) async fn is_minimize_to_tray_enabled(storage_root: &Path) -> bool {
    let path = config_xml_path(storage_root);
    let val = config::get_value(&path, MINIMIZE_TO_TRAY_KEY).await;
    val.eq_ignore_ascii_case("true")
}

/// Hide the main window and reveal the tray icon — the actual
/// "minimize to tray" side effect.
///
/// Extracted from [`handle_minimize_to_tray`] so the new
/// command-driven entry point (driven by the custom TitleBar
/// minimize button) and the legacy window-event fallback share one
/// implementation. `pub(crate)` because the command layer is the
/// only external caller; UI / domain code never touches this
/// directly.
///
/// Generic over `R: tauri::Runtime` so the
/// [`crate::commands::system::minimize_main_window`] command can
/// stay generic too — `tauri-specta`'s `collect_commands!` macro
/// requires every command's `AppHandle` parameter to be generic
/// even though production only ever instantiates the builder with
/// [`Wry`]. See the `auth::login_gamepass_start` doc comment for
/// the full rationale.
pub(crate) fn hide_to_tray<R: tauri::Runtime>(app_handle: &AppHandle<R>, tray_id: &TrayIconId) {
    if let Some(win) = app_handle.get_webview_window("main") {
        let _ = win.hide();
    }
    if let Some(tray) = app_handle.tray_by_id(tray_id) {
        let _ = tray.set_visible(true);
    }
    tracing::info!(
        step = "Tray.MinimizedToTray",
        "main window hidden, tray icon shown"
    );
}

/// Build the tray icon and register the left-click restore handler.
/// Returns the tray icon ID so the caller can wire up the window-event
/// side (which must go on `tauri::Builder`, not on `AppHandle`).
///
/// Returns `None` if tray creation fails (logged, non-fatal).
pub fn build_tray(app: &App<Wry>) -> Option<TrayIconId> {
    let tray: tauri::tray::TrayIcon<Wry> = match TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .expect("default window icon must exist")
                .clone(),
        )
        .tooltip("Beanfun")
        .build(app)
    {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(error = ?err, "failed to create tray icon; minimize-to-tray disabled");
            return None;
        }
    };

    let _ = tray.set_visible(false);
    let tray_id = tray.id().clone();

    tray.on_tray_icon_event(|tray: &tauri::tray::TrayIcon<Wry>, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            let _ = tray.set_visible(false);
            tracing::info!(
                step = "Tray.RestoredFromTray",
                "main window restored, tray icon hidden"
            );
        }
    });

    tracing::info!(
        step = "Tray.Initialized",
        "system tray icon created (hidden until minimize-to-tray)"
    );
    Some(tray_id)
}

/// Handle `WindowEvent::Resized(0, 0)` — the Windows signal for
/// minimize on a decorated window. Checks config, hides window,
/// shows tray icon.
///
/// **Defensive fallback**: with the post-PR-228 borderless +
/// transparent + non-resizable window config, Windows no longer
/// reliably fires `Resized(0, 0)` for `window.minimize()` calls,
/// which is why
/// [`crate::commands::system::minimize_main_window`] is the primary
/// path now. This listener stays so that any *other* trigger that
/// does end up minimizing the window still respects the config.
pub fn handle_minimize_to_tray(
    app_handle: AppHandle<Wry>,
    tray_id: TrayIconId,
    storage_root: PathBuf,
    event: &WindowEvent,
) {
    if let WindowEvent::Resized(PhysicalSize {
        width: 0,
        height: 0,
    }) = event
    {
        tauri::async_runtime::spawn(async move {
            if !is_minimize_to_tray_enabled(&storage_root).await {
                return;
            }
            hide_to_tray(&app_handle, &tray_id);
        });
    }
}
