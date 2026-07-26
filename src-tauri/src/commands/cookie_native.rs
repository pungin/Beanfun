//! Native WebView2 cookie seeding — bypasses wry's `set_cookie` which
//! strips the leading dot from cookie domains.

use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::WebviewWindow;
use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Profile3, ICoreWebView2_13, ICoreWebView2_2,
    COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE,
};
use wv2_windows_core::{Interface, PCWSTR};

use crate::services::beanfun::BeanfunClient;

/// Disable WebView2 **Tracking Prevention** for `window`'s profile.
///
/// reCAPTCHA (issues #313 / #315 / #318, task spec trap #2) needs
/// third-party storage on google.com / gstatic.com. WebView2's default
/// Tracking Prevention blocks it, so the widget renders but is dead
/// (unclickable) — the direct cause of #318. There is no Chromium
/// command-line flag for this; it must be set through the COM profile API:
/// `ICoreWebView2_13::Profile()` → `ICoreWebView2Profile3::
/// SetPreferredTrackingPreventionLevel(NONE)`.
///
/// Best-effort: returns `true` when the level was set, `false` on any COM
/// hiccup (older runtime without `ICoreWebView2Profile3`, etc.). A `false`
/// only means the reCAPTCHA may fail to render — never a hard error.
pub fn disable_tracking_prevention_native<R: tauri::Runtime>(window: &WebviewWindow<R>) -> bool {
    let done = Arc::new(AtomicBool::new(false));
    let done_inner = done.clone();

    let result = window.with_webview(move |webview| unsafe {
        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => {
                tracing::info!(step = "TrackingPrevention", error = ?e, "CoreWebView2");
                return;
            }
        };

        let core13: ICoreWebView2_13 = match Interface::cast(&core) {
            Ok(c) => c,
            Err(e) => {
                tracing::info!(step = "TrackingPrevention", error = ?e, "cast v13");
                return;
            }
        };

        let profile = match core13.Profile() {
            Ok(p) => p,
            Err(e) => {
                tracing::info!(step = "TrackingPrevention", error = ?e, "Profile");
                return;
            }
        };

        let profile3: ICoreWebView2Profile3 = match Interface::cast(&profile) {
            Ok(p) => p,
            Err(e) => {
                tracing::info!(step = "TrackingPrevention", error = ?e, "cast profile3");
                return;
            }
        };

        match profile3
            .SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE)
        {
            Ok(()) => done_inner.store(true, Ordering::SeqCst),
            Err(e) => {
                tracing::info!(step = "TrackingPrevention", error = ?e, "SetLevel");
            }
        }
    });

    if let Err(e) = result {
        tracing::info!(step = "TrackingPrevention", error = ?e, "with_webview");
        return false;
    }
    done.load(Ordering::SeqCst)
}

/// Seed every unexpired cookie from `client`'s reqwest jar into the
/// WebView2 cookie manager of `window` using the native COM API.
///
/// This function only *adds* cookies. Callers that need a clean slate
/// first (the GamePass re-login flow — issue #296) must call
/// [`clear_all_cookies_native`] in a **separate** pass and wait for it
/// to flush before seeding; see that function's docs for why the
/// clear and seed are deliberately not fused into one native call.
pub fn seed_cookies_native<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    client: &BeanfunClient,
) -> usize {
    let store = client.cookie_store();
    let guard = store
        .lock()
        .expect("cookie store mutex must not be poisoned");

    let cookies: Vec<(String, String, String, String, bool, bool)> = guard
        .iter_unexpired()
        .filter_map(|cookie| {
            let host = cookie.domain.as_cow()?;
            let host = host.into_owned();
            let is_suffix = Deref::deref(cookie).domain().is_some();
            let domain = if is_suffix && !host.starts_with('.') {
                format!(".{host}")
            } else {
                host
            };
            let path = Deref::deref(cookie).path().unwrap_or("/").to_owned();
            let secure = Deref::deref(cookie).secure().unwrap_or(false);
            let http_only = Deref::deref(cookie).http_only().unwrap_or(false);
            Some((
                cookie.name().to_owned(),
                cookie.value().to_owned(),
                domain,
                path,
                secure,
                http_only,
            ))
        })
        .collect();
    drop(guard);

    let total = cookies.len();

    let result = window.with_webview(move |webview| unsafe {
        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(step = "NativeSeed", error = ?e, "CoreWebView2");
                return;
            }
        };

        let core2: ICoreWebView2_2 = match Interface::cast(&core) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(step = "NativeSeed", error = ?e, "cast v2");
                return;
            }
        };

        let manager = match core2.CookieManager() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(step = "NativeSeed", error = ?e, "CookieManager");
                return;
            }
        };

        let mut seeded = 0usize;
        for (name, value, domain, path, secure, http_only) in &cookies {
            let w = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
            let wn = w(name);
            let wv = w(value);
            let wd = w(domain);
            let wp = w(path);

            match manager.CreateCookie(
                PCWSTR(wn.as_ptr()),
                PCWSTR(wv.as_ptr()),
                PCWSTR(wd.as_ptr()),
                PCWSTR(wp.as_ptr()),
            ) {
                Ok(c) => {
                    if *secure {
                        let _ = c.SetIsSecure(true);
                    }
                    if *http_only {
                        let _ = c.SetIsHttpOnly(true);
                    }
                    let _ = manager.AddOrUpdateCookie(&c);
                    seeded += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        step = "NativeSeed.Cookie",
                        name = %name, error = ?e,
                        "CreateCookie failed"
                    );
                }
            }
        }

        tracing::info!(
            step = "NativeSeed.Complete",
            seeded = seeded,
            total = total,
            "native cookie seeding done"
        );
    });

    if let Err(e) = result {
        tracing::warn!(step = "NativeSeed", error = ?e, "with_webview failed");
        return 0;
    }

    total
}

/// Delete **every** cookie in the WebView2 profile of `window` via the
/// native COM `ICoreWebView2CookieManager::DeleteAllCookies`.
///
/// # Why this is separate from [`seed_cookies_native`] (issue #296)
///
/// WebView2 keeps a single cookie store per user-data-folder, shared
/// by every window for the lifetime of the host *process*. After a
/// GamePass logout the server-side session is dead but its
/// `bfWebToken` / `ASP.NET_SessionId` cookies linger in that store, so
/// the next GamePass login (a new window, same process) inherits the
/// stale token, the portal short-circuits the OAuth round-trip, and
/// the harvest lifts a dead session. Restarting the .exe was the only
/// recovery (it ends the WebView2 browser session, dropping the
/// session cookies). Clearing the store before the next login makes
/// every attempt start fresh — equivalent to a process restart.
///
/// # Why a dedicated pass instead of clearing inside the seed
///
/// `DeleteAllCookies` and `AddOrUpdateCookie` are both fire-and-return
/// COM calls that queue work on the browser process; Microsoft does
/// **not** document an ordering guarantee between a delete and an
/// immediately-following add. Fusing them into one `with_webview`
/// closure risks the pending delete wiping the freshly-seeded cookies.
/// The caller therefore runs this clear in its own pass, waits a beat
/// for it to flush, and only then calls [`seed_cookies_native`].
///
/// Returns `true` if the `DeleteAllCookies` call was issued
/// successfully, `false` on any COM failure (logged at WARN). A
/// `false` return is best-effort — the caller still proceeds to seed.
pub fn clear_all_cookies_native<R: tauri::Runtime>(window: &WebviewWindow<R>) -> bool {
    let issued = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let issued_inner = issued.clone();

    let result = window.with_webview(move |webview| unsafe {
        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(step = "NativeClear", error = ?e, "CoreWebView2");
                return;
            }
        };

        let core2: ICoreWebView2_2 = match Interface::cast(&core) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(step = "NativeClear", error = ?e, "cast v2");
                return;
            }
        };

        let manager = match core2.CookieManager() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(step = "NativeClear", error = ?e, "CookieManager");
                return;
            }
        };

        match manager.DeleteAllCookies() {
            Ok(()) => {
                issued_inner.store(true, std::sync::atomic::Ordering::SeqCst);
                tracing::info!(
                    step = "NativeClear.Complete",
                    "DeleteAllCookies issued on WebView2 profile"
                );
            }
            Err(e) => {
                tracing::warn!(step = "NativeClear.Failed", error = ?e, "DeleteAllCookies failed");
            }
        }
    });

    if let Err(e) = result {
        tracing::warn!(step = "NativeClear", error = ?e, "with_webview failed");
        return false;
    }

    issued.load(std::sync::atomic::Ordering::SeqCst)
}

/// Register a `NewWindowRequested` handler on the WebView2 instance
/// that redirects popup requests to navigate within the same window.
pub fn register_new_window_handler<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    let result = window.with_webview(|webview| unsafe {
        use webview2_com::NewWindowRequestedEventHandler;

        let core = webview.controller().CoreWebView2().expect("CoreWebView2");

        let core_clone = core.clone();
        let handler = NewWindowRequestedEventHandler::create(Box::new(move |_sender, args| {
            if let Some(args) = args {
                let mut uri = wv2_windows_core::PWSTR::null();
                args.Uri(&mut uri)?;
                if let Ok(uri_str) = uri.to_string() {
                    if !uri_str.is_empty() {
                        let wide: Vec<u16> =
                            uri_str.encode_utf16().chain(std::iter::once(0)).collect();
                        core_clone.Navigate(PCWSTR(wide.as_ptr()))?;
                    }
                }
                args.SetHandled(true)?;
            }
            Ok(())
        }));

        let mut token: i64 = 0;
        let _ = core.add_NewWindowRequested(&handler, &mut token);

        tracing::info!(
            step = "NativeHandler.NewWindowRequested",
            "registered NewWindowRequested handler"
        );
    });

    if let Err(e) = result {
        tracing::warn!(
            step = "NativeHandler.Failed",
            error = ?e,
            "failed to register NewWindowRequested handler"
        );
    }
}

/// Register a `LaunchingExternalUriScheme` handler that suppresses
/// WebView2's "open <app>?" confirmation prompt and hands the URI to
/// `on_launch` instead.
///
/// The MapleStory Classic portal auto-fires an `ngm://` launch on
/// arrival; without this hook WebView2 pops a confirmation dialog the
/// user must click (and our portal window is hidden, so the launch
/// would silently stall). With it, the prompt is cancelled and the
/// caller starts Nexon Game Manager natively from its registered
/// handler (`commands::classic`).
///
/// Requires `ICoreWebView2_18` (WebView2 Runtime 111+, 2023). Returns
/// an error on older runtimes so the caller can fall back to showing
/// the window and letting the user click the prompt by hand.
pub fn register_external_uri_handler<R, F>(
    window: &WebviewWindow<R>,
    on_launch: F,
) -> Result<(), String>
where
    R: tauri::Runtime,
    F: Fn(&str) + Send + Sync + 'static,
{
    use std::sync::Mutex;

    // `with_webview` runs the closure on the event loop thread — marshal
    // the registration outcome back so the caller knows whether the
    // prompt is actually suppressed (it changes the whole UX strategy).
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let on_launch = Arc::new(on_launch);

    let result = window.with_webview(move |webview| unsafe {
        use webview2_com::LaunchingExternalUriSchemeEventHandler;
        use webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2_18;

        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => {
                if let Some(sender) = tx.lock().unwrap().take() {
                    let _ = sender.send(Err(format!("CoreWebView2 unavailable: {e}")));
                }
                return;
            }
        };

        let core18: ICoreWebView2_18 = match Interface::cast(&core) {
            Ok(c) => c,
            Err(e) => {
                if let Some(sender) = tx.lock().unwrap().take() {
                    let _ = sender.send(Err(format!(
                        "ICoreWebView2_18 unavailable (runtime too old): {e}"
                    )));
                }
                return;
            }
        };

        let cb = on_launch.clone();
        let handler = LaunchingExternalUriSchemeEventHandler::create(Box::new(
            move |_sender, args| -> wv2_windows_core::Result<()> {
                if let Some(args) = args {
                    let mut uri = wv2_windows_core::PWSTR::null();
                    args.Uri(&mut uri)?;
                    let url = uri.to_string().unwrap_or_default();
                    if !uri.is_null() {
                        wv2_windows_core::imp::CoTaskMemFree(uri.as_ptr() as _);
                    }
                    // Cancel the prompt; the callback owns the launch.
                    args.SetCancel(true)?;
                    if !url.is_empty() {
                        tracing::info!(
                            step = "NativeHandler.ExternalUriScheme",
                            "intercepted external-scheme launch: {url}"
                        );
                        cb(&url);
                    }
                }
                Ok(())
            },
        ));

        let mut token: i64 = 0;
        if let Err(e) = core18.add_LaunchingExternalUriScheme(&handler, &mut token) {
            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(Err(format!("add_LaunchingExternalUriScheme failed: {e}")));
            }
            return;
        }
        tracing::info!(
            step = "NativeHandler.ExternalUriScheme",
            token,
            "registered LaunchingExternalUriScheme handler"
        );

        if let Some(sender) = tx.lock().unwrap().take() {
            let _ = sender.send(Ok(()));
        }
    });

    if let Err(e) = result {
        return Err(format!(
            "with_webview failed for LaunchingExternalUriScheme handler: {e}"
        ));
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("LaunchingExternalUriScheme registration timed out".to_string()),
    }
}

/// Register a `WebMessageReceived` handler — the real page→host channel.
///
/// # Why not `document.title` (measured, issue 懷舊服 spec §5.1)
///
/// The classic portal's injected script used to signal the host by
/// setting `document.title` and the host polled `WebviewWindow::title()`.
/// That silently never works: the native window keeps the title it was
/// created with, so the poll saw the constant forever and the signal was
/// lost with no error anywhere. `window.chrome.webview.postMessage(...)`
/// → `add_WebMessageReceived` is the channel WebView2 actually provides
/// for this, and it works on remote origins where Tauri's own IPC is
/// blocked by the page's CSP.
///
/// `on_message` receives the raw string the page posted (we post JSON).
/// Returns an error if the handler can't be attached — which doubles as
/// a **liveness probe**: it round-trips through COM, so a window whose
/// webview failed to initialise (`ERROR_INVALID_STATE`, spec §5.2)
/// fails here instead of looking healthy and silently doing nothing.
pub fn register_web_message_handler<R, F>(
    window: &WebviewWindow<R>,
    on_message: F,
) -> Result<(), String>
where
    R: tauri::Runtime,
    F: Fn(&str) + Send + Sync + 'static,
{
    use std::sync::Mutex;

    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let on_message = Arc::new(on_message);

    let result = window.with_webview(move |webview| unsafe {
        use webview2_com::WebMessageReceivedEventHandler;

        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => {
                if let Some(sender) = tx.lock().unwrap().take() {
                    let _ = sender.send(Err(format!("CoreWebView2 unavailable: {e}")));
                }
                return;
            }
        };

        let cb = on_message.clone();
        let handler = WebMessageReceivedEventHandler::create(Box::new(
            move |_sender, args| -> wv2_windows_core::Result<()> {
                if let Some(args) = args {
                    let mut raw = wv2_windows_core::PWSTR::null();
                    // `TryGetWebMessageAsString` fails for non-string
                    // messages; we only ever post strings.
                    if args.TryGetWebMessageAsString(&mut raw).is_ok() {
                        let message = raw.to_string().unwrap_or_default();
                        if !raw.is_null() {
                            wv2_windows_core::imp::CoTaskMemFree(raw.as_ptr() as _);
                        }
                        if !message.is_empty() {
                            cb(&message);
                        }
                    }
                }
                Ok(())
            },
        ));

        let mut token: i64 = 0;
        if let Err(e) = core.add_WebMessageReceived(&handler, &mut token) {
            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(Err(format!("add_WebMessageReceived failed: {e}")));
            }
            return;
        }
        tracing::info!(
            step = "NativeHandler.WebMessage",
            token,
            "registered WebMessageReceived handler"
        );

        if let Some(sender) = tx.lock().unwrap().take() {
            let _ = sender.send(Ok(()));
        }
    });

    if let Err(e) = result {
        return Err(format!("with_webview failed for WebMessageReceived: {e}"));
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("WebMessageReceived registration timed out".to_string()),
    }
}
