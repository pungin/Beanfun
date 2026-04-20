//! In-app web browser command — opens a [`tauri::WebviewWindow`] at
//! the given URL with the logged-in [`BeanfunClient`] session cookies
//! pre-seeded so `beanfun.com` pages render with the user's
//! authenticated state.
//!
//! P12.4-followup-B replaces the `<iframe>` placeholder
//! (`src/windows/WebBrowser.vue`, deleted in B8) with a real
//! `WebviewWindow` mirror of WPF
//! `Beanfun/Windows/WebBrowser.xaml(.cs)`:
//!
//! ```csharp
//! // WPF L46: navigate to the requested URI
//! wb_Main.Source = new Uri(_initialUri);
//! // WPF L56-67: seed every BeanfunClient cookie into WebView2
//! foreach (Cookie cookie in App.MainWnd.bfClient.GetCookies())
//!     wb_Main.CoreWebView2.CookieManager.AddOrUpdateCookie(...);
//! ```
//!
//! # Pre-flight Q matrix decisions (P12.4-followup-B, Todo.md)
//!
//! - **Q1 (multi-open)** — every call mints a fresh window with a
//!   unique label (`web-browser-{nanos}`), mirroring WPF's
//!   `new WebBrowser(uri).Show()` per-click instantiation. Reuse-by-
//!   navigate would couple sequential clicks (e.g. open Register,
//!   then Forgot — second click should not lose the first window).
//! - **Q2 (chrome)** — OS-native chrome (title bar / min / max /
//!   close). Frameless `DragMove` parity is deferred to P13.
//! - **Q3/Q4 (cookie injection)** — use [`tauri::WebviewWindow::set_cookie`]
//!   with the same domain rehydration helper
//!   ([`seed_webview_cookies_from_client`]) the GamePass flow uses
//!   (`commands/auth.rs::open_gamepass_window` L1361-1378). The
//!   helper rehydrates host-only cookies' `Domain` attribute so
//!   WebView2's `ICoreWebView2Cookie` accepts them — without this
//!   `set_cookie` silently fails on Windows for cookies whose
//!   `Set-Cookie` had no `Domain=` (see
//!   `services/beanfun/login/gamepass.rs::seed_webview_cookies_from_client`
//!   L194-219 for the full Windows bug discussion). The eval-based
//!   `document.cookie="..."` alternative was rejected because
//!   `bfWebToken` is `HttpOnly` and the JS cookie API silently
//!   ignores HttpOnly inserts — `set_cookie` operates at the
//!   CookieManager level and bypasses that restriction.
//! - **Q5 (no session)** — when [`AppState::auth`] is `None`
//!   (LoginPage path: RegisterAccount / ForgotPassword pre-login),
//!   the cookie-seed loop is a no-op (the closure runs zero times).
//!   The public registration / password-recovery pages render
//!   without authentication — no error, no fallback, just open.
//! - **Q6 (allowlist)** — only `tw.beanfun.com` / `hk.beanfun.com`
//!   / `bfweb.hk.beanfun.com` are routed through the in-app
//!   window. Any other host returns [`INVALID_URL_CODE`]; the
//!   frontend `useInAppBrowser` composable detects this code and
//!   falls back to the system browser via `commands.openUrl`. The
//!   allowlist matches WPF's de-facto behaviour (every WPF
//!   construction site uses one of these three hosts) and limits
//!   the backend's webview-creation surface to known origins.
//! - **Q9 (target=_blank)** — Tauri's default
//!   `WebviewWindow::navigate` behaviour for in-page link clicks is
//!   to navigate within the same window (no popup), which already
//!   matches WPF `CoreWebView2_NewWindowRequested` L78-85 (re-routes
//!   to `Navigate(e.Uri)`). No additional hook required.
//!
//! # `about:blank → seed → navigate` pattern
//!
//! [`WebviewWindowBuilder::build`] is async-until-first-navigation —
//! we cannot interpose cookie seeding before the initial
//! `WebviewUrl::External(target)` request fires. The same trick the
//! GamePass flow shipped (build with `about:blank`, seed cookies,
//! then [`WebviewWindow::navigate`] the real URL) ensures the first
//! authenticated request to the target host carries every session
//! cookie.
//!
//! For the unauthenticated path (Q5) the trick is harmless: the
//! about:blank page loads instantly, the seed loop runs zero times,
//! and `navigate(target)` immediately follows — the user sees one
//! navigation, not two.
//!
//! # Why `async fn`
//!
//! Same Windows deadlock guard as
//! `commands::auth::open_gamepass_window` (see that function's "Why
//! async?" docblock for the wry#583 reference): synchronous
//! `WebviewWindowBuilder::build` blocks the WebView2 message pump
//! when invoked from a sync command. `async fn` hands the work to
//! the tokio executor, off the message-pump thread.

use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use crate::commands::{error::CommandError, state::AppState};
use crate::services::beanfun::login::seed_webview_cookies_from_client;

/// Hosts allowed to open in the in-app browser window.
///
/// Mirrors `src/windows/WebBrowser.vue`'s P12.4 D8
/// `URL_NEEDS_COOKIE_HOSTS` constant — every WPF `new WebBrowser(uri)`
/// construction site (id-pass form Register / Forgot, KartTools
/// hyperlink dispatch, MapleTools PlayerReport / VideoReport) targets
/// one of these three hosts. URLs against other hosts are rejected
/// with [`INVALID_URL_CODE`] so the frontend composable can fall
/// back to `commands.openUrl` (system browser).
///
/// Listed inline rather than queried from
/// [`crate::services::beanfun::client::ClientConfig`] because:
/// 1. The list is static (no per-region permutations beyond the
///    hostnames already enumerated here).
/// 2. The check has to run before any state is touched, so taking
///    a `State<'_, AppState>` lock for the lookup would be wasteful.
const ALLOWED_HOSTS: &[&str] = &["tw.beanfun.com", "hk.beanfun.com", "bfweb.hk.beanfun.com"];

/// Error code surfaced when [`open_in_app_browser`] receives a URL
/// that is malformed, uses an unsupported scheme, or targets a host
/// outside [`ALLOWED_HOSTS`].
///
/// Reuses the existing `system.invalid_url` code minted by
/// [`crate::services::system::open_url`] — frontend toast pipelines
/// already key off this code, and the shared semantic ("the URL is
/// not acceptable for this entry point") makes the reuse natural.
/// The frontend `useInAppBrowser` composable special-cases this
/// code as the "fall back to system browser" trigger; any other
/// error code is surfaced as a real failure.
pub(crate) const INVALID_URL_CODE: &str = "system.invalid_url";

/// Parse `url` and assert it points at one of [`ALLOWED_HOSTS`] over
/// `http` / `https`.
///
/// # Errors
///
/// Every failure mode returns [`INVALID_URL_CODE`] so the frontend
/// fallback path has a single condition to check. The `message`
/// disambiguates between "malformed", "wrong scheme" and "wrong
/// host" for log-grepping; the frontend doesn't display it
/// (the toast is generic).
fn parse_and_validate(url: &str) -> Result<Url, CommandError> {
    let parsed = Url::parse(url).map_err(|e| {
        CommandError::new(
            INVALID_URL_CODE,
            format!("In-app browser rejected malformed URL: {e}"),
        )
    })?;

    let scheme = parsed.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(CommandError::new(
            INVALID_URL_CODE,
            format!("In-app browser only accepts http/https; got: {scheme}"),
        ));
    }

    let Some(host) = parsed.host_str() else {
        return Err(CommandError::new(
            INVALID_URL_CODE,
            "In-app browser URL has no host".to_string(),
        ));
    };

    if !ALLOWED_HOSTS.contains(&host) {
        return Err(CommandError::new(
            INVALID_URL_CODE,
            format!("In-app browser host not in allowlist: {host}"),
        ));
    }

    Ok(parsed)
}

/// Build a unique [`WebviewWindow`] label for a fresh in-app browser
/// instance.
///
/// Tauri requires every concurrent window to carry a unique label;
/// because this command can be called multiple times in quick
/// succession (e.g. user clicks Register, then Forgot before closing
/// the first), we mint a fresh label per call. Nanosecond resolution
/// is overkill for human-paced clicks but cheap and keeps the helper
/// pure — no [`AppState`] read needed for an atomic counter.
///
/// On the (theoretically possible) clock-skew case where two calls
/// land on the same nanosecond, [`WebviewWindowBuilder::build`]
/// returns a "label already exists" error which propagates as
/// `ui.window_create_failed`; the frontend can retry. In practice
/// this branch has never been observed.
fn make_window_label() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("web-browser-{ts}")
}

/// Open `url` in a dedicated in-app webview window with the active
/// [`BeanfunClient`] session cookies pre-seeded.
///
/// # Successful flow
///
/// 1. Validate `url` against the [`ALLOWED_HOSTS`] allowlist
///    ([`parse_and_validate`]).
/// 2. Snapshot [`crate::commands::state::AuthContext::client`] from
///    [`AppState::auth`] under a read-lock (`None` when no login is
///    active — see the Q5 docblock note).
/// 3. Build the window pointing at `about:blank` so the first real
///    network request fires *after* the cookie seed.
/// 4. Best-effort seed every unexpired cookie via
///    [`seed_webview_cookies_from_client`] +
///    [`tauri::WebviewWindow::set_cookie`]. Per-cookie failures
///    log a warning and continue (matches WPF's no-try/catch
///    `AddOrUpdateCookie` loop).
/// 5. Navigate to `url`. The seeded cookies travel with the request.
///
/// # Errors
///
/// - [`INVALID_URL_CODE`] — URL malformed / wrong scheme / host
///   outside [`ALLOWED_HOSTS`]. Frontend (`useInAppBrowser`)
///   intercepts this code and falls back to the system browser via
///   `commands.openUrl`.
/// - `ui.window_create_failed` — [`WebviewWindowBuilder::build`] or
///   the post-seed [`tauri::WebviewWindow::navigate`] call failed
///   (rare; usually a label collision or WebView2 runtime
///   regression). The window — if any — is closed before returning.
#[tauri::command]
#[specta::specta]
pub async fn open_in_app_browser<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), CommandError> {
    let target_url = parse_and_validate(&url)?;
    let host_label = target_url.host_str().unwrap_or("").to_string();
    let label = make_window_label();

    // Snapshot the client (if logged in) BEFORE building the window
    // so cookie seeding can run synchronously between `build` and
    // `navigate`. `None` matches the LoginPage / pre-login Q5 path:
    // public registration / password-recovery pages render fine
    // without session cookies.
    let client_opt = {
        let guard = state.auth.read().await;
        guard.as_ref().map(|ctx| ctx.client.clone())
    };

    let about_blank: Url = "about:blank".parse().expect("about:blank is a valid URL");

    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(about_blank))
        .title(format!("Beanfun - {host_label}"))
        .inner_size(850.0, 550.0)
        .resizable(true)
        .build()
        .map_err(|e| {
            CommandError::new(
                "ui.window_create_failed",
                format!("Failed to create in-app browser window: {e}"),
            )
        })?;

    if let Some(client) = client_opt.as_ref() {
        let mut seed_failures = 0usize;
        let seeded = seed_webview_cookies_from_client(client, |cookie| {
            if let Err(err) = window.set_cookie(cookie.clone()) {
                seed_failures += 1;
                tracing::warn!(
                    step = "InAppBrowser.SeedCookieError",
                    cookie_name = %cookie.name(),
                    cookie_domain = ?cookie.domain(),
                    error = ?err,
                    "failed to seed cookie into in-app browser; continuing"
                );
            }
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("seed closure is infallible");

        tracing::info!(
            step = "InAppBrowser.SeedSummary",
            label = %label,
            host = %host_label,
            seeded = seeded - seed_failures,
            failed = seed_failures,
            "cookie seed summary from BeanfunClient jar before navigation"
        );
    }

    if let Err(err) = window.navigate(target_url.clone()) {
        let _ = window.close();
        return Err(CommandError::new(
            "ui.window_create_failed",
            format!("Failed to navigate in-app browser to {target_url}: {err}"),
        ));
    }

    tracing::info!(
        step = "InAppBrowser.Opened",
        label = %label,
        url = %target_url,
        "in-app browser window opened"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Pure-helper tests. The full IPC path through
    //! [`open_in_app_browser`] requires building a real
    //! [`tauri::WebviewWindow`], which transitively depends on the
    //! WebView2 runtime DLL — same constraint that keeps
    //! `commands::auth::open_gamepass_window`'s integration testing
    //! out of the unit suite (see `commands/auth.rs` mod-doc on the
    //! `bindings_file_tests` Windows DLL story). Webdriver E2E
    //! coverage is deferred to P13.
    use super::*;

    #[test]
    fn parse_and_validate_accepts_tw_signup_url() {
        assert!(parse_and_validate(
            "https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx?service=999999_T0"
        )
        .is_ok());
    }

    #[test]
    fn parse_and_validate_accepts_tw_forgot_pwd_url() {
        assert!(parse_and_validate("https://tw.beanfun.com/member/forgot_pwd.aspx").is_ok());
    }

    #[test]
    fn parse_and_validate_accepts_hk_forgot_pwd_url() {
        assert!(parse_and_validate("https://hk.beanfun.com/member/forgot_pwd.aspx").is_ok());
    }

    #[test]
    fn parse_and_validate_accepts_hk_signup_url() {
        assert!(parse_and_validate(
            "https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx?service=999999_T0"
        )
        .is_ok());
    }

    #[test]
    fn parse_and_validate_rejects_disallowed_host() {
        let err = parse_and_validate("https://example.com/page").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
        assert!(
            err.message.contains("allowlist"),
            "message should mention allowlist; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_and_validate_rejects_subdomain_outside_allowlist() {
        // `evil.tw.beanfun.com` is NOT a substring match — exact host
        // equality only. Guard against a future relaxation that would
        // open the surface to attacker-controlled subdomains.
        let err = parse_and_validate("https://evil.tw.beanfun.com/page").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
    }

    #[test]
    fn parse_and_validate_rejects_non_http_scheme() {
        let err = parse_and_validate("ftp://tw.beanfun.com/").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
        assert!(
            err.message.contains("http/https"),
            "message should mention scheme; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_and_validate_rejects_javascript_scheme() {
        let err = parse_and_validate("javascript:alert(1)").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
    }

    #[test]
    fn parse_and_validate_rejects_malformed_url() {
        let err = parse_and_validate("not a url").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
    }

    #[test]
    fn parse_and_validate_rejects_empty_url() {
        let err = parse_and_validate("").expect_err("must reject");
        assert_eq!(err.code, INVALID_URL_CODE);
    }

    #[test]
    fn make_window_label_starts_with_prefix() {
        let label = make_window_label();
        assert!(label.starts_with("web-browser-"));
    }

    #[test]
    fn make_window_label_unique_across_close_calls() {
        // Nanosecond resolution clock; two consecutive calls in a
        // tight loop produce distinct labels in practice.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..16 {
            assert!(seen.insert(make_window_label()), "labels must be unique");
        }
    }

    #[test]
    fn allowed_hosts_match_login_constants_table() {
        // Defensive lock against drift between this allowlist and
        // the Vue-side `LOGIN_EXTERNAL_URLS` table — every URL in
        // that table must point at an allowed host.
        for url in [
            "https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx",
            "https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx",
            "https://tw.beanfun.com/member/forgot_pwd.aspx",
            "https://hk.beanfun.com/member/forgot_pwd.aspx",
        ] {
            assert!(
                parse_and_validate(url).is_ok(),
                "frontend constants table URL must be in backend allowlist: {url}"
            );
        }
    }
}
