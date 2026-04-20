//! System tray icon — mirrors WPF `MainWindow.xaml.cs` L81-86 / L845-857.
//!
//! WPF creates a `NotifyIcon` (initially hidden). When the user
//! minimizes the main window **and** the `minimize_to_tray` config
//! checkbox is checked, the window hides and the tray icon appears.
//! Left-clicking the tray icon restores the window and hides the icon.
//!
//! Tauri v2 has no dedicated `WindowEvent::Minimized` variant. On
//! Windows the OS fires `Resized(0, 0)` when a window is minimized,
//! so we detect that as the minimize signal.

use std::path::{Path, PathBuf};

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager, PhysicalSize, WindowEvent, Wry};

use crate::services::config;

const CONFIG_FILE_NAME: &str = "Config.xml";
const MINIMIZE_TO_TRAY_KEY: &str = "minimize_to_tray";

fn config_xml_path(storage_root: &Path) -> PathBuf {
    storage_root.join(CONFIG_FILE_NAME)
}

async fn is_minimize_to_tray_enabled(storage_root: &Path) -> bool {
    let path = config_xml_path(storage_root);
    let val = config::get_value(&path, MINIMIZE_TO_TRAY_KEY).await;
    val.eq_ignore_ascii_case("true")
}

/// Build the tray icon and register the left-click restore handler.
/// Returns the tray icon ID so the caller can wire up the window-event
/// side (which must go on `tauri::Builder`, not on `AppHandle`).
///
/// Returns `None` if tray creation fails (logged, non-fatal).
pub fn build_tray(app: &App<Wry>) -> Option<tauri::tray::TrayIconId> {
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
/// minimize. Checks config, hides window, shows tray icon.
pub fn handle_minimize_to_tray(
    app_handle: tauri::AppHandle<Wry>,
    tray_id: tauri::tray::TrayIconId,
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
            if let Some(win) = app_handle.get_webview_window("main") {
                let _ = win.hide();
            }
            if let Some(tray) = app_handle.tray_by_id(&tray_id) {
                let _ = tray.set_visible(true);
            }
            tracing::info!(
                step = "Tray.MinimizedToTray",
                "main window hidden, tray icon shown"
            );
        });
    }
}
