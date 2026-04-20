//! Native WebView2 cookie seeding — bypasses wry's `set_cookie` which
//! strips the leading dot from cookie domains.

use std::ops::Deref;

use tauri::WebviewWindow;
use webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
use wv2_windows_core::{Interface, PCWSTR};

use crate::services::beanfun::BeanfunClient;

/// Seed every unexpired cookie from `client`'s reqwest jar into the
/// WebView2 cookie manager of `window` using the native COM API.
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
