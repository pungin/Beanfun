//! Native WebView2 navigation control for the in-app browser's content view.
//!
//! Tauri exposes `navigate` and `url`, but nothing else a toolbar needs: no
//! back, no forward, and — the part that actually matters — no way to ask
//! whether either is possible. Arrows that are always enabled are worse than no
//! arrows, so the state is read from `ICoreWebView2::CanGoBack` /
//! `CanGoForward`.
//!
//! Everything here takes a [`tauri::Webview`] rather than a
//! [`tauri::WebviewWindow`]: the content view shares its window with the
//! toolbar, so it has no window of its own. `WebviewWindow` is
//! `AsRef<Webview>`, so an ordinary single-webview window passes `.as_ref()`.

use serde::Serialize;

/// What the toolbar needs to draw itself: where we are, and where we can go.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NavState {
    pub url: String,
    pub title: String,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// Run `f` against the webview's `ICoreWebView2` and hand back what it returns.
///
/// `with_webview` hops to the main thread and offers no return channel, so the
/// value comes back over a channel. Callers must therefore not be on the main
/// thread themselves — every caller here is an async command handler, which is
/// not.
#[cfg(windows)]
fn with_core<R, T, F>(view: &tauri::Webview<R>, f: F) -> Result<T, String>
where
    R: tauri::Runtime,
    T: Send + 'static,
    F: FnOnce(&webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2) -> T
        + Send
        + 'static,
{
    use std::sync::{Arc, Mutex};

    let (tx, rx) = std::sync::mpsc::channel::<Result<T, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let send = tx.clone();
    let posted = view.with_webview(move |webview| {
        let outcome = unsafe {
            match webview.controller().CoreWebView2() {
                Ok(core) => Ok(f(&core)),
                Err(e) => Err(format!("CoreWebView2 unavailable: {e}")),
            }
        };
        if let Some(sender) = send.lock().unwrap().take() {
            let _ = sender.send(outcome);
        }
    });

    if posted.is_err() {
        return Err("with_webview failed".to_string());
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(inner) => inner,
        Err(_) => Err("webview call timed out".to_string()),
    }
}

/// Take ownership of a COM-allocated string and free the original.
#[cfg(windows)]
unsafe fn take_pwstr(raw: wv2_windows_core::PWSTR) -> String {
    if raw.is_null() {
        return String::new();
    }
    let out = unsafe { raw.to_string() }.unwrap_or_default();
    unsafe { wv2_windows_core::imp::CoTaskMemFree(raw.as_ptr() as _) };
    out
}

/// Read the current URL, title and history availability in one hop.
#[cfg(windows)]
pub fn nav_state<R: tauri::Runtime>(view: &tauri::Webview<R>) -> Result<NavState, String> {
    with_core(view, |core| unsafe {
        let mut source = wv2_windows_core::PWSTR::null();
        let _ = core.Source(&mut source);
        let mut title = wv2_windows_core::PWSTR::null();
        let _ = core.DocumentTitle(&mut title);

        let mut back = wv2_windows_core::BOOL(0);
        let _ = core.CanGoBack(&mut back);
        let mut forward = wv2_windows_core::BOOL(0);
        let _ = core.CanGoForward(&mut forward);

        NavState {
            url: take_pwstr(source),
            title: take_pwstr(title),
            can_go_back: back.as_bool(),
            can_go_forward: forward.as_bool(),
        }
    })
}

/// Go back one entry. A no-op when there is nothing behind us.
#[cfg(windows)]
pub fn go_back<R: tauri::Runtime>(view: &tauri::Webview<R>) -> Result<(), String> {
    with_core(view, |core| unsafe {
        let _ = core.GoBack();
    })
}

/// Go forward one entry. A no-op when there is nothing ahead.
#[cfg(windows)]
pub fn go_forward<R: tauri::Runtime>(view: &tauri::Webview<R>) -> Result<(), String> {
    with_core(view, |core| unsafe {
        let _ = core.GoForward();
    })
}

/// Reload the current page.
///
/// Deliberately not `location.reload()`: a page whose script has already thrown
/// still reloads through the native call.
#[cfg(windows)]
pub fn reload<R: tauri::Runtime>(view: &tauri::Webview<R>) -> Result<(), String> {
    with_core(view, |core| unsafe {
        let _ = core.Reload();
    })
}

/// Navigate to `url`.
///
/// Also native rather than assigning `window.location`, because an injected
/// script is subject to the page's CSP while `ICoreWebView2::Navigate` is not —
/// and beanfun's event pages ship one.
#[cfg(windows)]
pub fn navigate<R: tauri::Runtime>(view: &tauri::Webview<R>, url: &str) -> Result<(), String> {
    // Held for the duration of the call: `PCWSTR` only borrows the buffer.
    let target = wv2_windows_core::HSTRING::from(url);
    with_core(view, move |core| unsafe {
        let _ = core.Navigate(wv2_windows_core::PCWSTR(target.as_ptr()));
    })
}

#[cfg(not(windows))]
pub fn nav_state<R: tauri::Runtime>(_view: &tauri::Webview<R>) -> Result<NavState, String> {
    Ok(NavState::default())
}

#[cfg(not(windows))]
pub fn go_back<R: tauri::Runtime>(_view: &tauri::Webview<R>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn go_forward<R: tauri::Runtime>(_view: &tauri::Webview<R>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn reload<R: tauri::Runtime>(_view: &tauri::Webview<R>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn navigate<R: tauri::Runtime>(_view: &tauri::Webview<R>, _url: &str) -> Result<(), String> {
    Ok(())
}
