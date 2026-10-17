//! Owns native window placement independently of route and remote-page content.
//!
//! Only completed user moves and an explicit reset update the main preference.
//! Native operations run on the UI thread; popup positions are never persisted.

pub mod geometry;

use geometry::{
    centred_visible, fit_visible, midpoint, monitor_for_preference, monitor_for_rect, ApplyState,
    FrameInsets, Placement, Point, Rect,
};
use std::{collections::HashSet, path::PathBuf, sync::Mutex};
use tauri::{Manager, Runtime, Window};

type GeometryResult<T> = Result<T, String>;

#[derive(Default)]
pub struct WindowGeometryManager {
    placement: Mutex<Placement>,
    preferred_monitor: Mutex<Option<Point>>,
    preferred_monitor_name: Mutex<Option<String>>,
    requested_size: Mutex<Option<(f64, f64)>>,
    state_path: Mutex<Option<PathBuf>>,
    applying: Mutex<ApplyState>,
    interactive_windows: Mutex<HashSet<String>>,
    awaiting_first_show: Mutex<HashSet<String>>,
}

fn work_area(monitor: &tauri::Monitor) -> Rect {
    let work = monitor.work_area();
    Rect {
        origin: Point {
            x: work.position.x,
            y: work.position.y,
        },
        width: work.size.width,
        height: work.size.height,
    }
}

fn bounds<R: Runtime>(window: &Window<R>) -> GeometryResult<Rect> {
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    Ok(Rect {
        origin: Point {
            x: position.x,
            y: position.y,
        },
        width: size.width,
        height: size.height,
    })
}

fn saved_point(json: &str, legacy: bool) -> Option<Point> {
    let data: serde_json::Value = serde_json::from_str(json).ok()?;
    let point = if legacy {
        data.get("main")?
    } else {
        data.get("preferred")?
    };
    point_from_json(point)
}

fn point_from_json(point: &serde_json::Value) -> Option<Point> {
    Some(Point {
        x: i32::try_from(point.get("x")?.as_i64()?).ok()?,
        y: i32::try_from(point.get("y")?.as_i64()?).ok()?,
    })
}

impl WindowGeometryManager {
    fn initialise<R: Runtime>(&self, window: &Window<R>) -> GeometryResult<()> {
        let directory = window
            .app_handle()
            .path()
            .app_config_dir()
            .map_err(|e| e.to_string())?;
        let path = directory.join(".window-geometry.json");
        let saved = std::fs::read_to_string(&path).ok();
        let preferred = saved
            .as_deref()
            .and_then(|text| saved_point(text, false))
            .or_else(|| {
                std::fs::read_to_string(directory.join(".window-state.json"))
                    .ok()
                    .and_then(|text| saved_point(&text, true))
            });
        self.placement.lock().unwrap().preferred = preferred;
        *self.preferred_monitor.lock().unwrap() = saved
            .as_deref()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .and_then(|value| value.get("monitor").and_then(point_from_json));
        *self.preferred_monitor_name.lock().unwrap() = saved
            .as_deref()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .and_then(|value| {
                value
                    .get("monitorName")
                    .and_then(|value| value.as_str().map(str::to_string))
            });
        *self.state_path.lock().unwrap() = Some(path);
        Ok(())
    }

    fn persist(&self) -> GeometryResult<()> {
        let Some(point) = self.placement.lock().unwrap().preferred else {
            return Ok(());
        };
        let Some(path) = self.state_path.lock().unwrap().clone() else {
            return Ok(());
        };
        let parent = path
            .parent()
            .ok_or("window preference has no parent directory")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        // Packaged launchers can virtualise an existing AppData directory onto
        // another volume while a new destination name still resolves through
        // the logical alias. Resolve the parent once for BOTH sibling paths so
        // the replacement remains a same-volume atomic rename.
        let parent = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
        let path = parent.join(
            path.file_name()
                .ok_or("window preference has no filename")?,
        );
        // A sibling staging file keeps an interrupted write from truncating the
        // last good preference. rename replaces atomically on Windows and Unix.
        let staging = path.with_extension(format!("{}.tmp", std::process::id()));
        let monitor = self
            .preferred_monitor
            .lock()
            .unwrap()
            .map(|point| serde_json::json!({ "x": point.x, "y": point.y }));
        let bytes = serde_json::to_vec(
            &serde_json::json!({ "version": 1, "preferred": { "x": point.x, "y": point.y }, "monitor": monitor, "monitorName": self.preferred_monitor_name.lock().unwrap().clone() }),
        )
        .map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut file = std::fs::File::create(&staging).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(staging, path).map_err(|e| e.to_string())
    }

    fn monitor<R: Runtime>(
        &self,
        window: &Window<R>,
        popup_initial: bool,
    ) -> GeometryResult<tauri::Monitor> {
        if popup_initial {
            if let Some(main) = window.app_handle().get_window("main") {
                if let Some(monitor) = main.current_monitor().map_err(|e| e.to_string())? {
                    return Ok(monitor);
                }
            }
        }
        if window.label() == "main" {
            if let Some(point) = self.placement.lock().unwrap().preferred {
                let monitors = window.available_monitors().map_err(|e| e.to_string())?;
                let areas: Vec<_> = monitors.iter().map(work_area).collect();
                let anchor = *self.preferred_monitor.lock().unwrap();
                let selected = if let Some(anchor) = anchor {
                    let names = monitors
                        .iter()
                        .map(|monitor| (monitor.name().map(String::as_str), work_area(monitor)))
                        .collect::<Vec<_>>();
                    monitor_for_preference(
                        self.preferred_monitor_name.lock().unwrap().as_deref(),
                        anchor,
                        &names,
                    )
                } else {
                    let mut rectangle = bounds(window)?;
                    rectangle.origin = point;
                    monitor_for_rect(rectangle, &areas)
                };
                if let Some(index) = selected {
                    if anchor.is_none() {
                        self.remember_monitor(&monitors[index]);
                    }
                    return Ok(monitors[index].clone());
                }
            }
        }
        window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .or(window.primary_monitor().map_err(|e| e.to_string())?)
            .ok_or_else(|| "no available monitor work area".into())
    }

    fn remember_monitor(&self, monitor: &tauri::Monitor) {
        *self.preferred_monitor.lock().unwrap() = Some(midpoint(work_area(monitor)));
        *self.preferred_monitor_name.lock().unwrap() = monitor.name().cloned();
    }

    fn apply<R: Runtime>(
        &self,
        window: &Window<R>,
        popup_initial: bool,
        reset: bool,
    ) -> GeometryResult<()> {
        if window.is_minimized().map_err(|e| e.to_string())?
            || window.is_maximized().map_err(|e| e.to_string())?
        {
            return Ok(());
        }
        if self
            .interactive_windows
            .lock()
            .unwrap()
            .contains(window.label())
        {
            return Ok(());
        }
        if !self.applying.lock().unwrap().begin() {
            return Ok(());
        }
        struct Applying<'a, R: Runtime>(&'a WindowGeometryManager, &'a Window<R>);
        impl<R: Runtime> Drop for Applying<'_, R> {
            fn drop(&mut self) {
                let pending = self.0.applying.lock().unwrap().finish();
                for label in pending {
                    if let Some(window) = self.1.app_handle().get_window(&label) {
                        // SetWindowPos can dispatch WM_DPICHANGED synchronously.
                        // Reconcile once after this operation, when Tao's final
                        // DPI rectangle and the actual frame are both available.
                        #[cfg(target_os = "windows")]
                        if let Err(error) = native::post_reconcile(&window) {
                            tracing::warn!(%error, %label, "could not defer display geometry validation");
                        }
                        #[cfg(not(target_os = "windows"))]
                        let _ = window;
                    }
                }
            }
        }
        let _applying = Applying(self, window);
        let monitor = if reset {
            window
                .current_monitor()
                .map_err(|e| e.to_string())?
                .ok_or("no current monitor")?
        } else {
            self.monitor(window, popup_initial)?
        };
        let work = work_area(&monitor);
        let margin = (10.0 * monitor.scale_factor()).round() as u32;
        if work.width == 0 || work.height == 0 {
            return Err("monitor work area is empty".into());
        }

        // Move to the destination monitor first so the actual frame insets and
        // DPI are measured there. The window is still hidden on initial setup.
        let current = bounds(window)?;
        if popup_initial {
            apply_outer(
                window,
                centred_visible(
                    current.width,
                    current.height,
                    work,
                    margin,
                    visible_frame(window, current),
                ),
            )?;
        }
        let current = bounds(window)?;
        let inner = window.inner_size().map_err(|e| e.to_string())?;
        let frame_width = current.width.saturating_sub(inner.width);
        let frame_height = current.height.saturating_sub(inner.height);
        let mut width = current.width;
        let mut height = current.height;
        if window.label() == "main" {
            if let Some((logical_width, logical_height)) = *self.requested_size.lock().unwrap() {
                let scale = monitor.scale_factor();
                width = ((logical_width * scale).round() as u32).saturating_add(frame_width);
                height = ((logical_height * scale).round() as u32).saturating_add(frame_height);
            }
        }
        let frame = visible_frame(window, current);
        let target = if popup_initial || reset {
            centred_visible(width, height, work, margin, frame)
        } else if window.label() == "main" {
            self.placement.lock().unwrap().target_visible(
                current.origin,
                width,
                height,
                work,
                margin,
                frame,
            )
        } else {
            fit_visible(current.origin, width, height, work, margin, frame)
        };
        apply_outer(window, target)?;
        if reset {
            self.placement.lock().unwrap().preferred = Some(bounds(window)?.origin);
            self.remember_monitor(&monitor);
            self.persist()?;
        } else if window.label() == "main" && self.placement.lock().unwrap().preferred.is_none() {
            // Keep the initial position during this session without writing a
            // preference until the user moves the window or requests a reset.
            self.placement.lock().unwrap().preferred = Some(bounds(window)?.origin);
            self.remember_monitor(&monitor);
        }
        Ok(())
    }
}

pub fn managed_label(label: &str) -> bool {
    matches!(
        label,
        "main" | "gamepass-login" | "recaptcha-solve" | "classic-login"
    ) || label.starts_with("web-browser-")
}

/// Schedule a complete operation on the event-loop thread and propagate its
/// result to IPC. Native setters are never raced from Tokio worker threads.
pub async fn resize_main<R: Runtime>(
    window: Window<R>,
    width: f64,
    height: f64,
) -> GeometryResult<()> {
    if !geometry::valid_logical_size(width, height) {
        return Err("invalid logical window dimensions".into());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let task_window = window.clone();
    window
        .run_on_main_thread(move || {
            let manager = task_window.state::<WindowGeometryManager>();
            *manager.requested_size.lock().unwrap() = Some((width, height));
            let _ = sender.send(manager.apply(&task_window, false, false));
        })
        .map_err(|e| e.to_string())?;
    receiver.await.map_err(|e| e.to_string())?
}

pub async fn reset_main<R: Runtime>(window: Window<R>) -> GeometryResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let task_window = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = sender.send(task_window.state::<WindowGeometryManager>().apply(
                &task_window,
                false,
                true,
            ));
        })
        .map_err(|e| e.to_string())?;
    receiver.await.map_err(|e| e.to_string())?
}

/// Called immediately after creation, before the first visible frame.
pub fn initialise<R: Runtime>(window: &Window<R>) -> GeometryResult<()> {
    let result = on_ui_thread(window, |window| {
        let manager = window.state::<WindowGeometryManager>();
        if window.label() == "main" {
            manager.initialise(&window)?;
        }
        #[cfg(target_os = "windows")]
        native::install(&window)?;
        manager.apply(&window, window.label() != "main", false)?;
        if window.label() != "main" {
            manager
                .awaiting_first_show
                .lock()
                .unwrap()
                .insert(window.label().to_string());
        }
        Ok(())
    });
    if result.is_err() && window.label() != "main" {
        let _ = window.close();
    }
    result
}

pub fn initialise_webview<R: Runtime>(window: &tauri::WebviewWindow<R>) -> GeometryResult<()> {
    let webview: &tauri::Webview<R> = window.as_ref();
    initialise(&webview.window())
}

pub fn show<R: Runtime>(window: &Window<R>) -> GeometryResult<()> {
    on_ui_thread(window, |window| {
        let manager = window.state::<WindowGeometryManager>();
        manager.apply(&window, false, false)?;
        window.show().map_err(|e| e.to_string())?;
        // DWM may withhold extended frame bounds until the first show. Validate
        // the visible centre once more in the same UI operation, then retain
        // subsequent user placement on every hide/show cycle.
        if manager
            .awaiting_first_show
            .lock()
            .unwrap()
            .remove(window.label())
        {
            manager.apply(&window, true, false)?;
        }
        Ok(())
    })
}

pub fn show_webview<R: Runtime>(window: &tauri::WebviewWindow<R>) -> GeometryResult<()> {
    let webview: &tauri::Webview<R> = window.as_ref();
    show(&webview.window())
}

/// Re-show validates the current bounds without undoing a popup's manual move.
pub fn ensure_visible<R: Runtime>(window: &Window<R>) -> GeometryResult<()> {
    on_ui_thread(window, |window| {
        window
            .state::<WindowGeometryManager>()
            .apply(&window, false, false)
    })
}

fn on_ui_thread<R: Runtime>(
    window: &Window<R>,
    operation: impl FnOnce(Window<R>) -> GeometryResult<()> + Send + 'static,
) -> GeometryResult<()> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let target = window.clone();
    // Tauri executes this immediately when already on the event-loop thread.
    window
        .run_on_main_thread(move || {
            let _ = sender.send(operation(target));
        })
        .map_err(|e| e.to_string())?;
    receiver.recv().map_err(|e| e.to_string())?
}

pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &tauri::WindowEvent) {
    if !managed_label(window.label()) {
        return;
    }
    if matches!(event, tauri::WindowEvent::Destroyed) {
        window
            .state::<WindowGeometryManager>()
            .applying
            .lock()
            .unwrap()
            .forget(window.label());
        window
            .state::<WindowGeometryManager>()
            .interactive_windows
            .lock()
            .unwrap()
            .remove(window.label());
        window
            .state::<WindowGeometryManager>()
            .awaiting_first_show
            .lock()
            .unwrap()
            .remove(window.label());
    }
    // On Windows the subclass validates DPI only after Tao has applied its
    // suggested bounds. The Tauri event is emitted before that final resize.
    let validate = matches!(event, tauri::WindowEvent::Focused(true))
        || (!cfg!(target_os = "windows")
            && matches!(event, tauri::WindowEvent::ScaleFactorChanged { .. }));
    if validate {
        if let Err(error) = ensure_visible(window) {
            tracing::warn!(%error, label = window.label(), "could not validate window geometry after a display or focus change");
        }
    }
}

#[cfg(target_os = "windows")]
fn visible_frame<R: Runtime>(window: &Window<R>, outer: Rect) -> FrameInsets {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
    };
    let Ok(hwnd) = window.hwnd() else {
        return FrameInsets::default();
    };
    let mut visible = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            HWND(hwnd.0),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut visible as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    };
    let right = i64::from(outer.origin.x) + i64::from(outer.width);
    let bottom = i64::from(outer.origin.y) + i64::from(outer.height);
    if result.is_err()
        || visible.right <= visible.left
        || visible.bottom <= visible.top
        || visible.left < outer.origin.x
        || visible.top < outer.origin.y
        || i64::from(visible.right) > right
        || i64::from(visible.bottom) > bottom
    {
        // Hidden windows do not always have DWM bounds yet. This value is not
        // cached: the first visible frame and every later operation remeasure.
        return FrameInsets::default();
    }
    FrameInsets {
        left: (visible.left - outer.origin.x) as u32,
        top: (visible.top - outer.origin.y) as u32,
        right: (right - i64::from(visible.right)) as u32,
        bottom: (bottom - i64::from(visible.bottom)) as u32,
    }
}

#[cfg(not(target_os = "windows"))]
fn visible_frame<R: Runtime>(_window: &Window<R>, _outer: Rect) -> FrameInsets {
    FrameInsets::default()
}

#[cfg(target_os = "windows")]
fn apply_outer<R: Runtime>(window: &Window<R>, rect: Rect) -> GeometryResult<()> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };
    if bounds(window)? == rect {
        return Ok(());
    }
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    // An outer physical rectangle includes the system title bar and borders.
    unsafe {
        SetWindowPos(
            HWND(hwnd.0),
            HWND::default(),
            rect.origin.x,
            rect.origin.y,
            rect.width as i32,
            rect.height as i32,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "windows"))]
fn apply_outer<R: Runtime>(window: &Window<R>, rect: Rect) -> GeometryResult<()> {
    let current = bounds(window)?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    if current == rect {
        return Ok(());
    }
    window
        .set_size(tauri::PhysicalSize::new(
            rect.width
                .saturating_sub(current.width.saturating_sub(inner.width))
                .max(1),
            rect.height
                .saturating_sub(current.height.saturating_sub(inner.height))
                .max(1),
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(rect.origin.x, rect.origin.y))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use windows::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                PostMessageW, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ENTERSIZEMOVE,
                WM_EXITSIZEMOVE, WM_NCDESTROY, WM_SETTINGCHANGE,
            },
        },
    };

    const SUBCLASS_ID: usize = 0x42464745;
    const WM_RECONCILE_GEOMETRY: u32 = WM_APP + 0x2745;

    pub(super) fn post_reconcile<R: Runtime>(window: &Window<R>) -> GeometryResult<()> {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        unsafe { PostMessageW(HWND(hwnd.0), WM_RECONCILE_GEOMETRY, WPARAM(0), LPARAM(0)) }
            .map_err(|e| e.to_string())
    }

    pub(super) fn install<R: Runtime>(window: &Window<R>) -> GeometryResult<()> {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let pointer = Box::into_raw(Box::new(window.clone()));
        // The box belongs to the native subclass until WM_NCDESTROY. Windows
        // invokes the procedure only on the owning event-loop thread.
        if !unsafe {
            SetWindowSubclass(
                HWND(hwnd.0),
                Some(procedure::<R>),
                SUBCLASS_ID,
                pointer as usize,
            )
        }
        .as_bool()
        {
            unsafe {
                drop(Box::from_raw(pointer));
            }
            return Err("could not install native window geometry observer".into());
        }
        Ok(())
    }

    unsafe extern "system" fn procedure<R: Runtime>(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        data: usize,
    ) -> LRESULT {
        if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(procedure::<R>), id);
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            drop(Box::from_raw(data as *mut Window<R>));
            return result;
        }
        if !matches!(
            message,
            WM_ENTERSIZEMOVE
                | WM_EXITSIZEMOVE
                | WM_DISPLAYCHANGE
                | WM_SETTINGCHANGE
                | WM_DPICHANGED
                | WM_RECONCILE_GEOMETRY
        ) {
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        // A lower subclass may destroy the native window while forwarding a
        // message. Keep an owned Tauri handle rather than borrowing the box
        // that WM_NCDESTROY releases during such a nested call.
        let window = (&*(data as *const Window<R>)).clone();
        let result = DefSubclassProc(hwnd, message, wparam, lparam);
        // Native interactive movement is distinct from SetWindowPos. DPI,
        // display changes, minimise and programmatic corrections never save.
        let manager = window.state::<WindowGeometryManager>();
        match message {
            WM_ENTERSIZEMOVE => {
                manager
                    .interactive_windows
                    .lock()
                    .unwrap()
                    .insert(window.label().to_string());
                if window.label() == "main" {
                    if let Ok(rect) = bounds(&window) {
                        manager
                            .placement
                            .lock()
                            .unwrap()
                            .begin_user_move(rect.origin);
                    }
                }
            }
            WM_EXITSIZEMOVE => {
                if window.label() == "main" {
                    if !window.is_minimized().unwrap_or(true) {
                        if let Ok(rect) = bounds(&window) {
                            let moved = manager
                                .placement
                                .lock()
                                .unwrap()
                                .finish_user_move(rect.origin);
                            if moved {
                                if let Ok(Some(monitor)) = window.current_monitor() {
                                    manager.remember_monitor(&monitor);
                                }
                                if let Err(error) = manager.persist() {
                                    tracing::warn!(%error, "could not save main window preference");
                                }
                            }
                        }
                    }
                    manager.placement.lock().unwrap().dragging = false;
                }
                manager
                    .interactive_windows
                    .lock()
                    .unwrap()
                    .remove(window.label());
                if let Err(error) = ensure_visible(&window) {
                    tracing::warn!(%error, "could not fit window after native movement");
                }
            }
            WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED | WM_RECONCILE_GEOMETRY => {
                let immediate = manager
                    .applying
                    .lock()
                    .unwrap()
                    .request_display_change(window.label());
                if immediate {
                    if let Err(error) = ensure_visible(&window) {
                        tracing::warn!(%error, "could not fit window after display work area changed");
                    }
                }
            }
            _ => {}
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_reads_only_the_main_window() {
        let json = r#"{"main":{"x":-1700,"y":75},"classic-login":{"x":55,"y":66}}"#;
        assert_eq!(saved_point(json, true), Some(Point { x: -1700, y: 75 }));
        assert_eq!(
            saved_point(r#"{"classic-login":{"x":55,"y":66}}"#, true),
            None
        );
    }

    #[test]
    fn invalid_preference_is_not_used() {
        for json in [
            "invalid",
            r#"{"preferred":{"x":1.5,"y":0}}"#,
            r#"{"preferred":{"x":2147483648,"y":0}}"#,
        ] {
            assert_eq!(saved_point(json, false), None);
        }
    }

    #[test]
    fn persistence_contains_only_the_preferred_main_position() {
        let directory = tempfile::tempdir().unwrap();
        let manager = WindowGeometryManager::default();
        let path = directory.path().join(".window-geometry.json");
        *manager.state_path.lock().unwrap() = Some(path.clone());
        manager.placement.lock().unwrap().preferred = Some(Point { x: -950, y: 250 });
        manager.persist().unwrap();
        manager.placement.lock().unwrap().preferred = Some(Point { x: -900, y: 200 });
        manager.persist().unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(saved_point(&text, false), Some(Point { x: -900, y: 200 }));
        assert!(!text.contains("width"));
        assert!(!text.contains("classic-login"));
    }
}
