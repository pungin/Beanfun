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
//! - **Q6 (allowlist)** — superseded by Q11; see below.
//! - **Q11 (allowlist policy, P12.4-followup-B-fix)** — relaxed from
//!   the original Q6 three-host literal list to a `*.beanfun.com`
//!   suffix policy after smoke-test surfaced that
//!   `event.beanfun.com` (the WPF MapleTools PlayerReport target)
//!   was missing from the literal list; the original Q6 decision
//!   was made before that consumer was audited. The suffix policy
//!   is `host == "beanfun.com" || host.ends_with(".beanfun.com")`
//!   — both checks are required (`ends_with(".beanfun.com")`
//!   alone misses the apex `beanfun.com` because that string
//!   doesn't begin with a dot; the leading dot prevents
//!   `evil-beanfun.com` slipping through). Trade-off: any
//!   subdomain of `beanfun.com` (including future ones) opens in
//!   the in-app webview without code change. This is intentional
//!   — WPF `new WebBrowser(uri).Show()` itself has zero allowlist
//!   so any trusted-by-Beanfun host should work without a code
//!   change here. The non-beanfun rejection path is preserved as
//!   the security guard (frontend then falls back to system
//!   browser via `commands.openUrl`).
//! - **Q9 (target=_blank)** — Tauri's default
//!   `WebviewWindow::navigate` behaviour for in-page link clicks is
//!   to navigate within the same window (no popup), which already
//!   matches WPF `CoreWebView2_NewWindowRequested` L78-85 (re-routes
//!   to `Navigate(e.Uri)`). No additional hook required.
//! - **Q12 (P12.4-followup-B-fix F7 — black-flash UX)** — smoke
//!   test surfaced that opening the in-app browser briefly flashed
//!   a white/black window before `navigate(target_url)` rendered
//!   the first frame. Root cause is WebView2's default white
//!   background showing during the [`WebviewWindowBuilder::build`]
//!   → `set_cookie` → `navigate` window: the OS shows the new
//!   window as soon as `build` returns, but the webview hasn't
//!   painted any content yet (still on `about:blank`), and
//!   WebView2 has no native "set background before instantiation"
//!   API (upstream issue [tauri#14831], [tauri#1564]). The fix
//!   builds the window with `visible(false)`, registers an
//!   [`tauri::webview::PageLoadEvent::Finished`] callback that
//!   `show()`s the window on the first non-`about:blank` paint,
//!   and spawns a tokio safety-net task that force-shows after
//!   [`SAFETY_SHOW_AFTER_SECS`] in case the page never loads (slow
//!   network, broken site). An [`AtomicBool`] gates which side
//!   wins the race so `show()` only fires once. The
//!   [`background_color`] alternative was rejected because it's
//!   newer than the `tauri = "2"` floor we pin (added Nov 2024 in
//!   tauri PR #11486) and `visible(false)` + page-load callback is
//!   the recommended pattern across both WebView2 issues anyway.
//!   Trade-off: an extra ~50-200 ms perceived "open delay" before
//!   the window appears (vs. the previous "appear immediately,
//!   then flash"). User-tested as a clear net positive.
//!
//! # Internal helper extraction (P12.4-followup-B-fix F9)
//!
//! The window-build / cookie-seed / navigate / show-on-load chain
//! lives in [`open_url_in_webview`], a private async helper used by
//! both [`open_in_app_browser`] (frontend-supplied URL) and
//! [`open_member_center_browser`] (backend-built URL containing
//! the `web_token` server-side secret — never crosses IPC, see
//! that command's docblock for the security rationale). The two
//! commands diverge only in URL provenance and session-lookup;
//! sharing the rest avoids the typical "two near-identical window
//! builders that drift" anti-pattern (see WPF
//! `Pages/AccountList.xaml.cs::BF_btnMember_Click` vs.
//! `btn_Customerservice_Click` for the original WPF symmetry we
//! mirror).
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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{webview::PageLoadEvent, AppHandle, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use crate::commands::{error::CommandError, session::require_auth, state::AppState};
use crate::services::beanfun::{
    client::{BeanfunClient, LoginRegion},
    session::Session,
};

/// Force-show the in-app browser window after this many seconds if
/// the [`PageLoadEvent::Finished`] callback never fires (very slow
/// page, network failure, or the navigated host returning a hung
/// connection). Without this safety net a broken target URL would
/// leave the window invisible forever and the user would think the
/// click did nothing.
///
/// Five seconds is a deliberate over-shoot of the 95th-percentile
/// page-load time we observed for `tw.beanfun.com` and
/// `bfweb.hk.beanfun.com` (both load comfortably under 1.5 s on a
/// warm cache). The trade-off bias is: the common case fires
/// `Finished` long before the timer, and the fallback only matters
/// when something is wrong — at which point showing an empty
/// webview is strictly better than showing nothing.
const SAFETY_SHOW_AFTER_SECS: u64 = 5;

/// Apex domain whose hierarchy is routed through the in-app
/// browser window. See [`is_allowed_host`] for the suffix-match
/// rule and the file-level Q11 docblock for why this replaced the
/// original three-host literal list.
const ALLOWED_DOMAIN: &str = "beanfun.com";

/// Suffix-match a parsed URL's `host_str()` against
/// [`ALLOWED_DOMAIN`].
///
/// Returns `true` for `beanfun.com` itself and any subdomain
/// (`tw.beanfun.com`, `event.beanfun.com`, `bfweb.hk.beanfun.com`,
/// hypothetical future hosts, …); `false` for everything else.
///
/// The `==` arm handles the apex domain (`ends_with(".beanfun.com")`
/// would miss it because the apex string has no leading dot). The
/// `ends_with(".beanfun.com")` arm — note the leading dot — is
/// what stops `evil-beanfun.com` from passing as a "suffix": a
/// substring suffix like `ends_with("beanfun.com")` would have
/// allowed `evil-beanfun.com`, which is owned by an attacker.
///
/// Inputs from [`url::Url::host_str`] are already normalised
/// (lowercase, no port, IDN punycoded) so a plain ASCII
/// case-sensitive comparison is correct.
fn is_allowed_host(host: &str) -> bool {
    host == ALLOWED_DOMAIN || host.ends_with(&format!(".{ALLOWED_DOMAIN}"))
}

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

/// Parse `url` and assert it points at a host accepted by
/// [`is_allowed_host`] over `http` / `https`.
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

    if !is_allowed_host(host) {
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

/// Build the in-app browser window, seed the supplied
/// [`BeanfunClient`]'s session cookies, navigate to `target_url`,
/// and arrange for the window to be shown only once the target's
/// first paint completes (Q12 black-flash mitigation).
///
/// Pre-condition: `target_url` MUST already have passed
/// [`parse_and_validate`] (the helper does not re-check the host
/// allowlist — callers that take a frontend-supplied URL string
/// validate first; callers that build the URL themselves are
/// expected to feed it through `parse_and_validate` as defence in
/// depth before invoking this helper).
///
/// `client_opt` is `None` for the unauthenticated path
/// (Q5 — pre-login RegisterAccount / ForgotPassword). When `Some`,
/// every unexpired cookie in the client jar is best-effort seeded
/// via [`tauri::WebviewWindow::set_cookie`] before navigation
/// (per-cookie failures log + continue, matching WPF's no-try/catch
/// `AddOrUpdateCookie` loop in `WebBrowser.xaml.cs` L56-67).
///
/// # Show-on-load race
///
/// The window is built with `visible(false)`. Two paths can fire
/// `show()`:
///
/// 1. The [`PageLoadEvent::Finished`] callback for any URL other
///    than `about:blank` (the success path — happens on every
///    non-pathological navigation).
/// 2. The [`SAFETY_SHOW_AFTER_SECS`] tokio timer (the safety net —
///    fires only if the page never finishes loading).
///
/// Whoever swaps the [`AtomicBool`] from `false` to `true` first
/// owns the `show()` call; the loser short-circuits and logs
/// nothing (the absence of one log line is intentional — we don't
/// want noise when the common case wins the race).
///
/// # Errors
///
/// - `ui.window_create_failed` — [`WebviewWindowBuilder::build`] or
///   the post-seed [`tauri::WebviewWindow::navigate`] call failed
///   (rare; usually a label collision or WebView2 runtime
///   regression). The window — if any — is closed before returning.
async fn open_url_in_webview<R: tauri::Runtime>(
    app: &AppHandle<R>,
    target_url: Url,
    client_opt: Option<BeanfunClient>,
) -> Result<(), CommandError> {
    let host_label = target_url.host_str().unwrap_or("").to_string();
    let label = make_window_label();

    // Cookie seeding + navigation strategy:
    // 1. Build window on `about:blank` (hidden)
    // 2. Seed cookies via native WebView2 COM API (bypasses Tauri's
    //    broken `set_cookie` wrapper)
    // 3. Navigate to the real target URL
    let about_blank: Url = "about:blank".parse().expect("about:blank is a valid URL");

    let already_shown = Arc::new(AtomicBool::new(false));
    let already_shown_for_callback = already_shown.clone();

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(about_blank))
        .title(format!("Beanfun - {host_label}"))
        .inner_size(850.0, 550.0)
        .resizable(true)
        .visible(false)
        // Without this every `target="_blank"` control on the page is
        // dead — the opener plugin cancels the click and its `open_url`
        // is denied here. See `commands::remote_page`.
        .initialization_script(crate::commands::remote_page::KEEP_LINKS_IN_WINDOW)
        .on_page_load(move |window, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let url = payload.url().as_str();
            if url == "about:blank" {
                return;
            }

            // Idempotent: only the first page-load callback wins
            // the race against the safety timer. Subsequent
            // navigations within the same window (in-page link
            // clicks, redirects after first paint) are no-op
            // because `swap` returns `true` from the second hit
            // onwards.
            if already_shown_for_callback.swap(true, Ordering::SeqCst) {
                return;
            }
            if let Err(err) = window.show() {
                tracing::warn!(
                    step = "InAppBrowser.PageReadyShowFailed",
                    error = ?err,
                    "first-paint show() failed; window remains hidden"
                );
                return;
            }
            // Best-effort focus — failure is non-fatal (e.g. user
            // already tabbed away). Log nothing.
            let _ = window.set_focus();
            tracing::info!(
                step = "InAppBrowser.PageReadyShown",
                url = %crate::core::redact::redact_uri(payload.url().as_str()),
                "in-app browser shown after first non-about:blank Finished event"
            );
        })
        .build()
        .map_err(|e| {
            CommandError::new(
                "ui.window_create_failed",
                format!("Failed to create in-app browser window: {e}"),
            )
        })?;

    // No separate cookie seeding or navigate() needed — the window
    // was built with the target URL directly. Cookie seeding + reload
    // happens inside the on_page_load callback above.
    let _ = &client_opt; // consumed by the closure

    // Seed cookies via native WebView2 COM API, then navigate.
    #[cfg(target_os = "windows")]
    {
        // Register NewWindowRequested handler — redirect popups to
        // navigate within the same window (WPF parity).
        super::cookie_native::register_new_window_handler(&window);

        if let Some(ref client) = client_opt {
            let seeded = super::cookie_native::seed_cookies_native(&window, client);
            tracing::info!(
                step = "InAppBrowser.NativeSeed",
                seeded = seeded,
                "native cookie seeding complete; navigating to target"
            );
        }
    }

    // Brief yield to let the cookie manager flush.
    tokio::time::sleep(Duration::from_millis(200)).await;

    if let Err(err) = window.navigate(target_url.clone()) {
        let _ = window.close();
        return Err(CommandError::new(
            "ui.window_create_failed",
            format!("Failed to navigate in-app browser to {target_url}: {err}"),
        ));
    }

    // Safety net for Q12: if `Finished` never fires (broken target,
    // network error, slow server) we still want the user to see
    // *something* rather than be left waiting for an invisible
    // window. Spawning on tokio (not std::thread) keeps the
    // shutdown story consistent with the rest of the command —
    // tauri tears down the runtime on app exit and the task is
    // dropped cleanly.
    let window_for_safety = window.clone();
    let already_shown_for_safety = already_shown.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(SAFETY_SHOW_AFTER_SECS)).await;
        if already_shown_for_safety.swap(true, Ordering::SeqCst) {
            // Page-load callback already showed the window — common case.
            return;
        }
        if let Err(err) = window_for_safety.show() {
            tracing::warn!(
                step = "InAppBrowser.SafetyShowFailed",
                error = ?err,
                "safety-net show() failed; window remains hidden — likely already closed"
            );
            return;
        }
        let _ = window_for_safety.set_focus();
        tracing::warn!(
            step = "InAppBrowser.SafetyShown",
            "in-app browser force-shown after safety timeout (page-load Finished never fired)"
        );
    });

    tracing::info!(
        step = "InAppBrowser.Opened",
        label = %label,
        // Member-centre and Gash URLs carry `web_token` in the query.
        url = %crate::core::redact::redact_uri(target_url.as_str()),
        "in-app browser window opened (hidden until first paint)"
    );

    Ok(())
}

/// Open `url` in a dedicated in-app webview window with the active
/// [`BeanfunClient`] session cookies pre-seeded.
///
/// # Successful flow
///
/// 1. Validate `url` against the [`is_allowed_host`] suffix policy
///    ([`parse_and_validate`]).
/// 2. Snapshot [`crate::commands::state::AuthContext::client`] from
///    [`AppState::auth`] under a read-lock (`None` when no login is
///    active — see the Q5 docblock note).
/// 3. Delegate to [`open_url_in_webview`] for the build / seed /
///    navigate / show-on-load chain (shared with
///    [`open_member_center_browser`]).
///
/// # Errors
///
/// - [`INVALID_URL_CODE`] — URL malformed / wrong scheme / host
///   outside [`is_allowed_host`]. Frontend (`useInAppBrowser`)
///   intercepts this code and falls back to the system browser via
///   `commands.openUrl`.
/// - `ui.window_create_failed` — see [`open_url_in_webview`].
#[tauri::command]
#[specta::specta]
pub async fn open_in_app_browser<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), CommandError> {
    let target_url = parse_and_validate(&url)?;

    // Snapshot the client (if logged in) BEFORE building the window
    // so cookie seeding can run synchronously between `build` and
    // `navigate`. `None` matches the LoginPage / pre-login Q5 path:
    // public registration / password-recovery pages render fine
    // without session cookies.
    let client_opt = {
        let guard = state.auth.read().await;
        guard.as_ref().map(|ctx| ctx.client.clone())
    };

    open_url_in_webview(&app, target_url, client_opt).await
}

/// Build the [WPF-equivalent](https://github.com/.../Pages/AccountList.xaml.cs#L167-L188)
/// member-center URL for the given session and return it as a
/// `String`. Mirrors WPF `BF_btnMember_Click` byte-for-byte:
///
/// ```text
/// TW: https://tw.beanfun.com/TW/auth.aspx?channel=member
///       &page_and_query=index_new.aspx
///       &web_token={WebToken}
/// HK: https://bfweb.hk.beanfun.com/HK/auth.aspx?channel=member
///       &page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0
///       &web_token={WebToken}
/// ```
///
/// `web_token` is interpolated raw (no percent-encoding) — same as
/// WPF, which uses C# string interpolation. Beanfun-issued tokens
/// are URL-safe in practice; if a future token format introduces
/// reserved characters we'll hear about it as a 401 from
/// `auth.aspx` long before any silent mis-routing happens, and the
/// fix would be on this single helper.
///
/// # Why a free function
///
/// Pure (no I/O, no state) so it's trivially testable without
/// spinning a tokio runtime, a tauri app, or a fake `AppState`.
/// The full IPC path through [`open_member_center_browser`] still
/// needs WebView2 runtime → deferred to E2E (P13), same as
/// [`open_in_app_browser`]. Coverage focuses on the URL-shape
/// invariants WPF parity depends on (channel, page_and_query,
/// web_token presence + position).
fn build_member_center_url(session: &Session) -> String {
    match session.region {
        LoginRegion::TW => format!(
            "https://tw.beanfun.com/TW/auth.aspx?channel=member&page_and_query=index_new.aspx&web_token={}",
            session.web_token
        ),
        LoginRegion::HK => format!(
            "https://bfweb.hk.beanfun.com/HK/auth.aspx?channel=member&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={}",
            session.web_token
        ),
    }
}

/// Open the Beanfun **Member Center** in a dedicated in-app webview
/// window. Mirrors WPF
/// `Pages/AccountList.xaml.cs::BF_btnMember_Click`.
///
/// # Why a dedicated command (not just a `commands.openInAppBrowser` call)
///
/// The member-center URL embeds the session's `web_token` as a
/// query parameter (WPF's design — the `auth.aspx` endpoint
/// consumes it server-side to issue a session-bearing redirect to
/// the actual member portal). `web_token` is a server-side secret
/// (`commands::dto` enforces the `web_token must not leak through
/// IPC` invariant via the sentinel-value test in
/// `commands/dto.rs::session_dto_redacts_*`); shipping it across
/// IPC just to interpolate it back into a URL would violate that
/// invariant. The dedicated command keeps the secret confined to
/// the backend: the frontend invokes `openMemberCenterBrowser()`
/// with no arguments, and the URL is built + dispatched entirely
/// in Rust.
///
/// # Successful flow
///
/// 1. [`require_auth`] resolves `(client, session)`. If no session
///    is active, returns `auth.session_required` — frontend toasts
///    via the standard error pipeline. (This branch should be
///    unreachable in practice because the AccountList page is
///    behind the auth route guard, but the guard is defence in
///    depth.)
/// 2. [`build_member_center_url`] interpolates the WPF URL shape
///    for the session's region.
/// 3. [`parse_and_validate`] re-checks the URL against the host
///    allowlist as defence in depth (catches any future drift
///    where the URL builder yields a non-`*.beanfun.com` host).
/// 4. [`open_url_in_webview`] runs the shared build / seed /
///    navigate / show-on-load chain.
///
/// # Errors
///
/// - `auth.session_required` — no active session.
/// - [`INVALID_URL_CODE`] — `build_member_center_url` produced a
///   URL outside the allowlist (defensive — should be unreachable
///   for valid `LoginRegion` variants).
/// - `ui.window_create_failed` — see [`open_url_in_webview`].
#[tauri::command]
#[specta::specta]
pub async fn open_member_center_browser<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let url_str = build_member_center_url(&session);
    let target_url = parse_and_validate(&url_str)?;
    open_url_in_webview(&app, target_url, Some(client)).await
}

/// Build the WPF-equivalent Gash recharge URL for the given session.
/// Mirrors WPF `Pages/AccountList.xaml.cs::bfb_Gash_Click`:
///
/// ```text
/// TW: https://tw.beanfun.com/TW/auth.aspx?channel=gash
///       &page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0
///       &web_token={WebToken}
/// HK: https://bfweb.hk.beanfun.com/HK/auth.aspx?channel=gash
///       &page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0
///       &web_token={WebToken}
/// ```
///
/// Both regions share the same `page_and_query` (unlike Member Center
/// where TW uses `index_new.aspx`). The only difference is the base
/// host (`tw.beanfun.com` vs `bfweb.hk.beanfun.com`).
/// Follow the `auth.aspx` redirect chain using the reqwest client
fn build_gash_recharge_url(session: &Session) -> String {
    let base = match session.region {
        LoginRegion::TW => "https://tw.beanfun.com/TW/",
        LoginRegion::HK => "https://bfweb.hk.beanfun.com/HK/",
    };
    format!(
        "{base}auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={}",
        session.web_token
    )
}

/// Open the Beanfun **Gash recharge** page in a dedicated in-app
/// webview window. Mirrors WPF
/// `Pages/AccountList.xaml.cs::bfb_Gash_Click`.
///
/// Same security rationale as [`open_member_center_browser`] — the
/// URL embeds `web_token`, so it must be built backend-side to keep
/// the secret confined to Rust.
///
/// # Errors
///
/// - `auth.session_required` — no active session.
/// - [`INVALID_URL_CODE`] — `build_gash_recharge_url` produced a
///   URL outside the allowlist (defensive — should be unreachable).
/// - `ui.window_create_failed` — see [`open_url_in_webview`].
#[tauri::command]
#[specta::specta]
pub async fn open_gash_recharge_browser<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let url_str = build_gash_recharge_url(&session);
    let target_url = parse_and_validate(&url_str)?;
    open_url_in_webview(&app, target_url, Some(client)).await
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
    fn parse_and_validate_accepts_event_beanfun_com() {
        // P12.4-followup-B-fix F1: MapleTools PlayerReport target
        // (`event.beanfun.com`) was missing from the original Q6
        // literal allowlist; suffix policy (Q11) restores parity
        // with WPF's de-facto "any beanfun host" behaviour.
        assert!(parse_and_validate(
            "https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx"
        )
        .is_ok());
    }

    #[test]
    fn parse_and_validate_accepts_apex_beanfun_com() {
        // The `host == ALLOWED_DOMAIN` arm of `is_allowed_host`. A
        // pure `ends_with(".beanfun.com")` would miss this case.
        assert!(parse_and_validate("https://beanfun.com/").is_ok());
    }

    #[test]
    fn parse_and_validate_accepts_arbitrary_beanfun_subdomain() {
        // Q11 explicitly opens the door for future Beanfun-owned
        // subdomains without a code change. `csp.beanfun.com` is
        // the customer-support host referenced by the Beanfun
        // 404-page footer; not currently linked from any button
        // but a representative "future caller" smoke.
        assert!(parse_and_validate("https://csp.beanfun.com/").is_ok());
    }

    #[test]
    fn parse_and_validate_rejects_lookalike_apex() {
        // The leading dot in `ends_with(".beanfun.com")` is what
        // stops attacker-owned domains that *contain* the apex
        // string from passing as a "suffix". Regression guard for
        // the Q11 docblock's stated trade-off.
        for host in ["evil-beanfun.com", "beanfunxcom", "notbeanfun.com"] {
            let url = format!("https://{host}/page");
            let err = parse_and_validate(&url).expect_err("must reject lookalike apex");
            assert_eq!(err.code, INVALID_URL_CODE, "case: {host}");
        }
    }

    #[test]
    fn parse_and_validate_rejects_apex_in_path_only() {
        // `beanfun.com.evil.com` — host is `beanfun.com.evil.com`,
        // which `ends_with(".beanfun.com")` is FALSE for (the
        // suffix `.evil.com` does not match `.beanfun.com`).
        // Regression guard against any future loosening that would
        // accept "the apex appears anywhere in the host" matching.
        let err = parse_and_validate("https://beanfun.com.evil.com/page")
            .expect_err("must reject apex-in-middle");
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

    /// Build a [`Session`] populated with sentinel field values so
    /// the URL-shape assertions can grep for them.
    ///
    /// `web_token` is the only field [`build_member_center_url`]
    /// reads beyond `region`, but populating the rest with stable
    /// strings keeps the helper future-proof against assertions
    /// that grow to cover (e.g.) `account_id` if WPF parity ever
    /// requires it.
    fn sample_session(region: LoginRegion, web_token: &str) -> Session {
        Session::new(region, "SKEY_TEST", web_token, "alice", "610074", "T9")
    }

    #[test]
    fn build_member_center_url_tw_mirrors_wpf_byte_for_byte() {
        // P12.4-followup-B-fix F9: WPF
        // `Pages/AccountList.xaml.cs::BF_btnMember_Click` L172-187
        // for `LoginRegion::TW`. Any drift here breaks the
        // member-center landing page (auth.aspx redirects to the
        // wrong service or 401s on a bad web_token).
        let session = sample_session(LoginRegion::TW, "WTOKEN_TW_HAPPY");
        let url = build_member_center_url(&session);
        assert_eq!(
            url,
            "https://tw.beanfun.com/TW/auth.aspx?channel=member&page_and_query=index_new.aspx&web_token=WTOKEN_TW_HAPPY"
        );
    }

    #[test]
    fn build_member_center_url_hk_mirrors_wpf_byte_for_byte() {
        // P12.4-followup-B-fix F9: WPF
        // `Pages/AccountList.xaml.cs::BF_btnMember_Click` L172-187
        // for `LoginRegion::HK`. Note the doubly-encoded
        // `page_and_query` — `service_code%3D999999` (`?service_code=999999`)
        // and `service_region%3DT0` (`&service_region=T0`) — that's
        // intentional: `auth.aspx` reads `page_and_query` as a
        // single opaque string and emits it verbatim into the
        // redirect target's query string, so it has to be
        // pre-encoded once on our side.
        let session = sample_session(LoginRegion::HK, "WTOKEN_HK_HAPPY");
        let url = build_member_center_url(&session);
        assert_eq!(
            url,
            "https://bfweb.hk.beanfun.com/HK/auth.aspx?channel=member&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token=WTOKEN_HK_HAPPY"
        );
    }

    #[test]
    fn build_member_center_url_passes_allowlist_for_both_regions() {
        // Defence-in-depth coverage for the
        // [`open_member_center_browser`] flow — the URL we hand to
        // [`open_url_in_webview`] must always survive
        // [`parse_and_validate`] (host inside `*.beanfun.com`).
        // Regression guard: if a future region URL constant points
        // outside the allowlist (e.g. a CDN-only host) this fails
        // immediately rather than the user seeing a confusing
        // "fallback to system browser" toast at runtime.
        for region in [LoginRegion::TW, LoginRegion::HK] {
            let session = sample_session(region, "WTOKEN_PARSE_OK");
            let url = build_member_center_url(&session);
            parse_and_validate(&url)
                .unwrap_or_else(|e| panic!("region {region:?} URL must validate: {e:?}"));
        }
    }

    #[test]
    fn build_member_center_url_embeds_web_token_verbatim() {
        // P12.4-followup-B-fix F9 — defensive coverage that the
        // helper never silently swaps in a placeholder. If a
        // future refactor accidentally interpolates a
        // `Default::default()` token for an empty string, the
        // user lands on a 401 page; this test catches the regression
        // before the integration smoke does.
        let session = sample_session(LoginRegion::TW, "VERY_DISTINCT_TOKEN_VALUE_42");
        let url = build_member_center_url(&session);
        assert!(
            url.contains("web_token=VERY_DISTINCT_TOKEN_VALUE_42"),
            "URL must embed the session's actual web_token; got: {url}"
        );
        assert!(
            !url.contains("web_token=&") && !url.ends_with("web_token="),
            "URL must not contain an empty web_token query value; got: {url}"
        );
    }

    #[test]
    fn build_gash_recharge_url_tw_mirrors_wpf_byte_for_byte() {
        let session = sample_session(LoginRegion::TW, "WTOKEN_TW_GASH");
        let url = build_gash_recharge_url(&session);
        assert_eq!(
            url,
            "https://tw.beanfun.com/TW/auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token=WTOKEN_TW_GASH"
        );
    }

    #[test]
    fn build_gash_recharge_url_hk_mirrors_wpf_byte_for_byte() {
        let session = sample_session(LoginRegion::HK, "WTOKEN_HK_GASH");
        let url = build_gash_recharge_url(&session);
        assert_eq!(
            url,
            "https://bfweb.hk.beanfun.com/HK/auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token=WTOKEN_HK_GASH"
        );
    }

    #[test]
    fn build_gash_recharge_url_passes_allowlist_for_both_regions() {
        for region in [LoginRegion::TW, LoginRegion::HK] {
            let session = sample_session(region, "WTOKEN_GASH_PARSE_OK");
            let url = build_gash_recharge_url(&session);
            parse_and_validate(&url)
                .unwrap_or_else(|e| panic!("region {region:?} URL must validate: {e:?}"));
        }
    }

    #[test]
    fn build_gash_recharge_url_embeds_web_token_verbatim() {
        let session = sample_session(LoginRegion::TW, "DISTINCT_GASH_TOKEN_99");
        let url = build_gash_recharge_url(&session);
        assert!(
            url.contains("web_token=DISTINCT_GASH_TOKEN_99"),
            "URL must embed the session's actual web_token; got: {url}"
        );
        assert!(
            !url.contains("web_token=&") && !url.ends_with("web_token="),
            "URL must not contain an empty web_token query value; got: {url}"
        );
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
    fn allowlist_admits_every_known_caller_url() {
        // Defensive lock against drift between this suffix policy
        // and the known frontend caller URLs — every URL the
        // Vue-side actually emits via `commands.openInAppBrowser`
        // must pass `is_allowed_host`. Sourced from:
        // - `src/constants/login.ts::LOGIN_EXTERNAL_URLS`
        //   (RegisterAccount / ForgotPassword in IdPassForm)
        // - `src/windows/MapleTools.vue` (PLAYER_REPORT_URL, VIDEO_REPORT_URL)
        // - `src/windows/KartTools.vue` (KART_TOOLS_ACTIONS)
        for url in [
            "https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx",
            "https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx",
            "https://tw.beanfun.com/member/forgot_pwd.aspx",
            "https://hk.beanfun.com/member/forgot_pwd.aspx",
            "https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx",
            "https://beanfun-event.beanfun.com/EventAD_Mobile/EventAD?eventAdId=3453",
            "https://tw.beanfun.com/KartRider/guild/maneger_data.aspx",
            "https://tw.beanfun.com/kartrider/guild/rank.aspx",
            "https://tw.beanfun.com/KartRider/guild/rank_team_in.aspx",
            "https://tw.beanfun.com/KartRider/guild/search_member.aspx",
            "https://tw.beanfun.com/KartRider/guild/create.aspx",
            "https://tw.beanfun.com/KartRider/guild/leave_guild_Member.aspx",
        ] {
            assert!(
                parse_and_validate(url).is_ok(),
                "known frontend caller URL must pass allowlist: {url}"
            );
        }
    }
}
