//! GamePass login **finalise** step — the point where the WebView
//! leg's cookie harvest flows back into the [`BeanfunClient`] jar
//! and resolves into a [`Session`].
//!
//! # WPF reference
//!
//! Ports `GamePassBrowser.TryCompleteLogin` (L119-163) and
//! `BeanfunClient.Login.cs::GamePassLogin` (L803-836):
//!
//! - `TryCompleteLogin` reads WebView2 cookies from three portal
//!   domains (`tw.beanfun.com`, `login.beanfun.com`,
//!   `tw.newlogin.beanfun.com`), finds `bfWebToken` scoped to
//!   `tw.beanfun.com`, and — when present — converts every cookie
//!   back to `System.Net.Cookie` and hands them plus the token to
//!   `App.MainWnd.GamePassLoginCompleted`.
//! - `GamePassLogin` then syncs each cookie into the
//!   `BeanfunClient` cookie jar via `SetCookie` (L813-821), stores
//!   `this.webtoken = webToken` (L823), and fires the downstream
//!   `GetAccounts` / `getRemainPoint` queries (L825-829).
//!
//! The WPF sequence crosses a UI boundary (WebView2 → MainWindow →
//! BeanfunClient) that we cleanly separate in the Rust port:
//!
//! 1. Command layer (`commands/auth.rs::open_gamepass_window`) —
//!    owns the [`tauri::WebviewWindow`], the [`tauri::AppHandle`],
//!    and the three `cookies_for_url` calls. Not portable, not
//!    unit-testable without a real WebView.
//! 2. Service layer (this module) — accepts cookies as values
//!    (already harvested from the WebView), writes them into the
//!    [`BeanfunClient`] jar, and builds the [`Session`]. Pure sync
//!    domain logic; unit-testable with plain [`RawCookie`]
//!    fixtures.
//!
//! # Why not an `async fn ... -> Result<Session, LoginError>`?
//!
//! Every other login-tail in this crate (HK Regular / TOTP / QR)
//! funnels through [`super::login_completed`], which does post to
//! `return.aspx` over HTTP. GamePass does **not** — the WebView has
//! already driven the full redirect chain by the time we're called,
//! so the finalise step here is zero-I/O and zero-fallible-work.
//! A sync `fn ... -> Option<Session>` is both the right signature
//! and the closest mirror of WPF `TryCompleteLogin` L143-144
//! (`if string.IsNullOrEmpty(webToken) return;`), which silently
//! early-returns without raising an error when the cookie is still
//! missing — the caller is expected to retry on the next
//! `NavigationCompleted`.
//!
//! # WPF divergences (same stance as [`super::login_completed`])
//!
//! - **No `GetAccounts` / `getRemainPoint` tail.** WPF
//!   `GamePassLogin` L825-829 inlines both, storing the results on
//!   `BeanfunClient`. We keep the finalise step narrowly scoped
//!   ("finalise auth → return `Session`"); account listing and
//!   balance queries live in `services/beanfun/account.rs` and
//!   higher-level orchestrators chain them when the caller actually
//!   needs the data. Matches the SRP stance already documented in
//!   [`super::login_completed`].
//! - **Deferred `account_id`.** The GamePass flow has no
//!   user-typed account id; it resolves on the subsequent
//!   `GetAccounts` call. We build the [`Session`] with
//!   `account_id = ""`, same sentinel the QR flow uses (see
//!   [`super::finalize_qr_login`] docs). The empty value is
//!   surfaced as `<deferred>` in operator-facing logs (see
//!   `completed.rs`'s matching rendering).

use std::ops::Deref;

use reqwest_cookie_store::RawCookie;
use url::Url;

use crate::services::beanfun::{BeanfunClient, Session};

use super::read_bfwebtoken_from_jar;

/// Write WebView-harvested cookies into `client`'s shared jar using
/// `source_url` as the RFC 6265 "request URL" for domain/path
/// attachment.
///
/// # Why three separate calls per flow (SRP)
///
/// The WebView's `cookies_for_url(url)` returns cookies scoped to a
/// single origin. GamePass harvests from **three** origins
/// (`tw.beanfun.com`, `login.beanfun.com`, `tw.newlogin.beanfun.com`)
/// because the redirect chain lands on all three and `bfWebToken`
/// can only be confirmed against the portal origin. Callers invoke
/// this helper once per origin, passing the matching `source_url`,
/// so [`cookie_store::CookieStore::insert_raw`]'s RFC 6265
/// domain-match check runs with the correct reference — exactly
/// mirroring the WPF `ConvertCookies` loop (L147-150, one call per
/// `GetCookiesAsync(domainUrl)` result set).
///
/// Merging the three sets and inserting them under a single origin
/// would silently miscategorise cookies whose `Domain` attribute
/// doesn't domain-match the merged URL; merging into the broadest
/// origin (e.g. `.beanfun.com`) wouldn't match WPF's structure and
/// would risk accepting cookies the real WPF code would have
/// rejected.
///
/// # Parameters
///
/// - `client` — target client whose cookie jar absorbs the
///   inserts. Thread-safe — the jar is behind an
///   `Arc<Mutex<_>>`, so concurrent injectors from parallel
///   WebView callbacks serialise naturally.
/// - `source_url` — the HTTPS origin the cookies were harvested
///   from. Used verbatim as the "request URL" passed to
///   [`cookie_store::CookieStore::store_response_cookies`].
/// - `cookies` — owned [`RawCookie<'static>`] values. The type
///   matches what [`tauri::WebviewWindow::cookies_for_url`] returns
///   (`Vec<cookie::Cookie<'static>>`), so the command layer can
///   pass the webview's output in directly without conversion.
///
/// # Silently-dropped inserts
///
/// `store_response_cookies` silently skips cookies whose `Domain`
/// attribute doesn't domain-match `source_url` (RFC 6265 §5.3).
/// This is the desired behaviour — it's the exact same rule WPF's
/// `CookieContainer.Add(Uri, Cookie)` enforces when the reference
/// URI is set. The dropped cookies never end up in our jar, the
/// outbound HTTP client never sends them, and the gamepass finalise
/// step's subsequent `read_bfwebtoken_from_jar` call only sees
/// portal-visible entries.
pub fn inject_webview_cookies<I>(client: &BeanfunClient, source_url: &Url, cookies: I)
where
    I: IntoIterator<Item = RawCookie<'static>>,
{
    let store = client.cookie_store();
    let mut guard = store
        .lock()
        .expect("cookie store mutex must not be poisoned");
    guard.store_response_cookies(cookies.into_iter(), source_url);
}

/// Seed every unexpired cookie currently in `client`'s jar into the
/// GamePass WebView — the **reverse** direction of
/// [`inject_webview_cookies`]: where `inject_webview_cookies` routes
/// WebView-observed cookies into the `BeanfunClient` jar,
/// `seed_webview_cookies_from_client` replicates the jar's state into
/// the WebView so the WebView inherits every session identifier
/// [`super::session_key::get_session_key`] left behind.
///
/// # WPF reference
///
/// Mirrors `Beanfun/Windows/GamePassBrowser.xaml.cs::OnWebViewReady`
/// L66-77:
///
/// ```csharp
/// if (App.MainWnd.bfClient != null) {
///     foreach (Cookie cookie in App.MainWnd.bfClient.GetCookies())
///         wb_Main.CoreWebView2.CookieManager.AddOrUpdateCookie(
///             wb_Main.CoreWebView2.CookieManager.CreateCookie(
///                 cookie.Name, cookie.Value, cookie.Domain, cookie.Path));
/// }
/// ```
///
/// beanfun's portal is stateful across this two-leg flow:
///
/// 1. `get_session_key` (reqwest) → beanfun plants a per-attempt
///    session id on the `BeanfunClient` jar and mints `pSKey`.
/// 2. WebView navigates `Login/Index?pSKey=…` → OAuth → …
///    `return.aspx` → beanfun needs the **same** session id in the
///    request cookies to match `pSKey` back to step 1's attempt.
///
/// Without step-2 cookies matching step 1's, `return.aspx` emits
/// "Get SecretCode Success(…) but get data fail: (0) No such auth
/// key and secret code." and never lands `bfWebToken`. Observed in
/// live test 2026-04-18 — this helper is the missing link CP3
/// shipped without.
///
/// # SRP split
///
/// The helper stops at the **extract-and-yield** boundary: for each
/// unexpired cookie in `client`'s jar it hands an owned
/// [`RawCookie<'static>`] clone to `sink` and counts it. The caller
/// decides what to do with each cookie (write to a real
/// [`tauri::WebviewWindow`] via `set_cookie` in production, collect
/// into a `Vec` in unit tests, etc.). This keeps the domain-layer
/// helper runtime-agnostic — a pure `BeanfunClient` consumer — while
/// the command layer owns the WebView side-effect.
///
/// # Host-only cookie rehydration
///
/// `cookie_store` distinguishes two scope flavours on the cookies it
/// holds (`cookie_store::CookieDomain`):
///
/// - `Suffix(host)` — the `Set-Cookie` carried an explicit `Domain`
///   attribute. The RawCookie clone already has `.domain() ==
///   Some(host)`.
/// - `HostOnly(host)` — the `Set-Cookie` had **no** `Domain`
///   attribute, so RFC 6265 §5.3 required the UA to pin the cookie
///   to exactly the request host. The RawCookie clone's
///   `.domain()` is `None`.
///
/// Handing a host-only RawCookie to
/// [`tauri::WebviewWindow::set_cookie`] silently fails on Windows —
/// the WebView2 runtime refuses cookies without a `Domain`, because
/// its `ICoreWebView2Cookie` surface requires one and there's no way
/// for `set_cookie` to synthesise "the current navigation host"
/// (the WebView isn't on any page yet — we seed pre-navigate).
/// **Observed 2026-04-18** (`GamepassWebViewSeed.JarDump` showed
/// `domain=None` on both `ASP.NET_SessionId` entries, followed by
/// `return.aspx` rejecting the round-trip with "No such auth key and
/// secret code"); the WPF reference trivially sidesteps this because
/// `System.Net.Cookie.Domain` is a non-empty string in both shapes
/// (`CookieContainer` rehydrates the request host onto host-only
/// entries before returning them), so `CoreWebView2.CookieManager.
/// CreateCookie(name, value, domain, path)` always has a domain to
/// work with.
///
/// To restore parity we rehydrate the `Domain` attribute from the
/// store's `CookieDomain` via [`CookieDomain::as_cow`] (which yields
/// `Some(host)` for both `HostOnly` and `Suffix`) and stamp it onto
/// the cloned `RawCookie` with [`RawCookie::set_domain`] before
/// handing it to the sink. Semantically this widens a host-only
/// cookie into a subdomain-match (the same widening WPF's
/// `CookieContainer` does implicitly) — a scope change the beanfun
/// portal tolerates because the session id it plants is already
/// single-attempt-scoped on the server side and never leaves the
/// `beanfun.com` eTLD+1 within this flow.
///
/// Cookies whose `CookieDomain` is `NotPresent` / `Empty` are not
/// reachable from `iter_unexpired` in practice (the store itself
/// rejects such entries at insert time per `cookie_store`'s
/// `try_from_raw_cookie`, L176-196), but for defensive correctness
/// the helper logs a warning and skips them rather than seeding a
/// domain-less cookie the WebView would silently drop.
///
/// # Error semantics
///
/// `sink` is fallible to let callers propagate the per-cookie error
/// type of their chosen WebView API (`tauri::Error` in our case) but
/// the helper **does not** retry or swallow: the first `Err` aborts
/// the loop and bubbles up. Production callers typically want the
/// opposite — "best effort, log per-cookie failures, keep going" —
/// which they achieve by wrapping `set_cookie` in a closure that
/// logs `Err` and returns `Ok(())`, matching WPF's per-cookie
/// `AddOrUpdateCookie` (no try/catch in the WPF loop; WebView2 just
/// silently skips on failure).
///
/// # Ordering guarantees
///
/// Iteration order matches [`cookie_store::CookieStore::iter_unexpired`]
/// which is not specified. Seeding is additive — order only matters
/// if two cookies collide on `(domain, path, name)`, which the
/// GamePass `get_session_key` flow never emits (each cookie is unique
/// by name on distinct origins).
///
/// # Return value
///
/// `Ok(count)` — number of cookies handed to `sink` (after filtering
/// expired entries). `Err(e)` — the first `sink` error, immediately
/// propagated; later cookies are silently dropped.
pub fn seed_webview_cookies_from_client<F, E>(
    client: &BeanfunClient,
    mut sink: F,
) -> Result<usize, E>
where
    F: FnMut(RawCookie<'static>) -> Result<(), E>,
{
    let store = client.cookie_store();
    let guard = store
        .lock()
        .expect("cookie store mutex must not be poisoned");
    let mut seeded = 0usize;
    for cookie in guard.iter_unexpired() {
        let Some(host) = cookie.domain.as_cow() else {
            tracing::warn!(
                name = cookie.name(),
                "cookie_store entry has no resolved domain; skipping seed"
            );
            continue;
        };
        let mut raw = cookie.deref().clone();

        // WebView2's `ICoreWebView2CookieManager::CreateCookie` treats
        // the domain literally: `"beanfun.com"` matches only the exact
        // host, while `".beanfun.com"` matches the apex **and** every
        // subdomain (`bfweb.hk.beanfun.com`, `tw.beanfun.com`, …).
        //
        // Suffix cookies (from `Set-Cookie` with explicit `Domain=`)
        // must domain-match subdomains per RFC 6265 §5.2.3. The store's
        // `as_cow()` returns the bare host without a leading dot, so
        // WebView2 would pin them to the apex only. We detect suffix
        // cookies by checking whether the *original* `RawCookie` (via
        // `Deref`) already had a `domain()` — suffix entries do,
        // host-only entries don't (their `domain()` is `None`).
        //
        // WPF sidesteps this: `System.Net.CookieContainer` stores the
        // domain with a leading dot for suffix cookies, so
        // `CreateCookie` gets the dot-prefixed form automatically.
        let domain_str = host.into_owned();
        let is_suffix = raw.domain().is_some();
        let webview_domain = if is_suffix && !domain_str.starts_with('.') {
            format!(".{domain_str}")
        } else {
            domain_str
        };
        raw.set_domain(webview_domain);

        sink(raw)?;
        seeded += 1;
    }
    Ok(seeded)
}

/// Assemble a [`Session`] from the cookies already sitting in
/// `client`'s jar, returning `None` when `bfWebToken` is not yet
/// visible from the portal origin.
///
/// # Preconditions
///
/// The caller has already invoked [`inject_webview_cookies`] for
/// every origin the WebView harvested from (at least the
/// `portal_base` one — WPF `TryCompleteLogin` L123-131). This
/// function performs **no** HTTP work and **no** cookie-jar
/// mutation beyond the read.
///
/// # Return value
///
/// - `Some(session)` — `bfWebToken` is present on the portal
///   origin. Callers should use this as the "login success"
///   signal: close the WebView window, populate
///   [`AppState::auth`][st], and emit the
///   `gamepass-login-success` event.
/// - `None` — `bfWebToken` is not yet present. Callers mirror
///   WPF `TryCompleteLogin` L143-144 by silently waiting for the
///   next `on_page_load` callback. The WebView may still be
///   resolving the final redirect, or the user may have navigated
///   to an intermediate page that doesn't carry the token (yet).
///
/// Using `Option` instead of `Result<Session, LoginError>` reflects
/// that a missing token is **not** an error — it's a "try again"
/// signal. Errors that genuinely fail the flow (transport, cookie
/// corruption) are out of scope here because this step does no I/O.
///
/// # `account_id` handling
///
/// The GamePass flow doesn't know the account id at this point
/// (the user authenticated through the portal's GamePass UI, not
/// a username/password form). We pass `""` — the same sentinel
/// the QR flow uses (see [`super::finalize_qr_login`] docs) — so
/// the downstream `GetAccounts` call can resolve it on the server.
/// Operator-facing logs render empty-string account ids as
/// `<deferred>` to make the intent explicit (matches the rendering
/// in [`super::login_completed`]).
///
/// [st]: crate::commands::state::AppState::auth
pub fn try_complete_gamepass_login(
    client: &BeanfunClient,
    session_key: &str,
    service_code: &str,
    service_region: &str,
) -> Option<Session> {
    let web_token = read_bfwebtoken_from_jar(client)?;

    let region = client.config().region;

    tracing::info!(
        step = "GamepassLoginCompleted",
        region = ?region,
        // account_id deferred; matches the "<deferred>" sentinel in
        // `completed.rs` so the two log sites read consistently in
        // post-mortems.
        account_id = "<deferred>",
        "GamePass login completed successfully"
    );

    Some(Session::new(
        region,
        session_key,
        web_token,
        "",
        service_code,
        service_region,
    ))
}

// -----------------------------------------------------------------------------
// Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::beanfun::{ClientConfig, LoginRegion};

    fn tw_client() -> BeanfunClient {
        BeanfunClient::new(ClientConfig::for_region(LoginRegion::TW)).expect("client builds")
    }

    fn raw_cookie(name: &str, value: &str, domain: &str) -> RawCookie<'static> {
        // Parse via the string form so every cookie passes through the
        // same RFC 6265 attribute parser the WebView2 runtime emits
        // cookies under — no hand-crafted struct that could diverge
        // from the on-the-wire shape.
        RawCookie::parse(format!("{name}={value}; Domain={domain}; Path=/"))
            .expect("cookie parses")
            .into_owned()
    }

    // ── inject_webview_cookies ────────────────────────────────────

    #[test]
    fn inject_webview_cookies_writes_portal_scoped_cookie_into_jar() {
        let client = tw_client();
        let source_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        let cookies = vec![raw_cookie("bfWebToken", "WTOKEN_GP", "tw.beanfun.com")];

        inject_webview_cookies(&client, &source_url, cookies);

        // Token must now be visible via the shared helper — i.e.
        // scoped to the portal origin exactly as `completed.rs` /
        // WPF `GetCookie("bfWebToken")` expects.
        assert_eq!(
            read_bfwebtoken_from_jar(&client).as_deref(),
            Some("WTOKEN_GP"),
            "bfWebToken must be visible on the portal origin after inject",
        );
    }

    #[test]
    fn inject_webview_cookies_is_additive_across_sources() {
        // GamePass harvests from three origins; a second inject from
        // a different source URL must NOT overwrite or drop the
        // first set.
        let client = tw_client();
        let tw_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        let login_url: Url = "https://login.beanfun.com/".parse().expect("valid url");

        inject_webview_cookies(
            &client,
            &tw_url,
            vec![raw_cookie("bfWebToken", "WT_TW", "tw.beanfun.com")],
        );
        inject_webview_cookies(
            &client,
            &login_url,
            // An auth-session cookie that would land on
            // `login.beanfun.com` during the GamePass flow.
            vec![raw_cookie("BF_SESSION", "SESS_LOGIN", "login.beanfun.com")],
        );

        // Portal-scoped lookup still sees the TW token.
        assert_eq!(
            read_bfwebtoken_from_jar(&client).as_deref(),
            Some("WT_TW"),
            "second inject must not clobber the first",
        );
    }

    #[test]
    fn inject_webview_cookies_rejects_cookies_with_mismatched_domain_attribute() {
        // RFC 6265 §5.3: a `Set-Cookie` whose Domain doesn't
        // domain-match the request URL must be silently rejected.
        // `cookie_store` enforces this; the helper must not silently
        // launder such cookies into the jar under a different
        // origin.
        let client = tw_client();
        let tw_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        let foreign = raw_cookie("bfWebToken", "SHOULD_BE_DROPPED", "evil.example.com");

        inject_webview_cookies(&client, &tw_url, vec![foreign]);

        // Jar stays empty — the cookie was refused, not misfiled
        // under tw.beanfun.com.
        assert!(
            read_bfwebtoken_from_jar(&client).is_none(),
            "mismatched-Domain cookie must be dropped, not laundered",
        );
    }

    #[test]
    fn inject_webview_cookies_empty_input_is_a_no_op() {
        let client = tw_client();
        let url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        inject_webview_cookies(&client, &url, std::iter::empty::<RawCookie<'static>>());
        assert!(read_bfwebtoken_from_jar(&client).is_none());
    }

    // ── seed_webview_cookies_from_client ──────────────────────────

    /// Happy path: a jar with a few WPF-shaped session cookies is
    /// fully yielded to the sink, owned, exactly once each.
    #[test]
    fn seed_yields_every_unexpired_cookie_to_the_sink() {
        // Build a jar with two cookies on distinct origins — matches
        // the shape `get_session_key` leaves behind (ASP.NET session
        // cookies on tw.beanfun.com / login.beanfun.com).
        let client = tw_client();
        let portal_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        let login_url: Url = "https://login.beanfun.com/".parse().expect("valid url");

        inject_webview_cookies(
            &client,
            &portal_url,
            vec![raw_cookie("BF_SESSION", "PORTAL_SID", "tw.beanfun.com")],
        );
        inject_webview_cookies(
            &client,
            &login_url,
            vec![raw_cookie(
                "ASP.NET_SessionId",
                "LOGIN_SID",
                "login.beanfun.com",
            )],
        );

        let mut seen: Vec<(String, String)> = Vec::new();
        let count = seed_webview_cookies_from_client(&client, |cookie| {
            seen.push((cookie.name().to_owned(), cookie.value().to_owned()));
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible sink");

        assert_eq!(count, 2, "both cookies reported in the count");
        assert_eq!(seen.len(), 2, "sink called exactly twice");
        // Order is unspecified by the underlying store, so assert
        // as a set.
        let names: std::collections::HashSet<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains("BF_SESSION"));
        assert!(names.contains("ASP.NET_SessionId"));
    }

    /// Empty jar → sink never invoked → count is zero. Pins the
    /// "nothing to seed on a fresh client" edge — important because
    /// `login_gamepass_start` mints a fresh client per attempt and
    /// the first tick before `get_session_key` runs the jar is
    /// literally empty.
    #[test]
    fn seed_on_empty_jar_is_a_no_op() {
        let client = tw_client();

        let mut invocations = 0usize;
        let count = seed_webview_cookies_from_client(&client, |_cookie| {
            invocations += 1;
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible sink");

        assert_eq!(count, 0);
        assert_eq!(invocations, 0, "sink never called on empty jar");
    }

    /// Sink error short-circuits the loop — subsequent cookies are
    /// NOT yielded. Pins the "first Err aborts" contract so callers
    /// relying on fail-fast behaviour (e.g. "if the WebView runtime
    /// is broken, stop early") don't get silently partial seeds.
    #[test]
    fn seed_propagates_sink_error_and_short_circuits() {
        let client = tw_client();
        let portal_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");

        // Inject TWO cookies so we can observe short-circuit:
        // if the loop kept going after the first Err, the second
        // sink call would land and our count would be 2.
        inject_webview_cookies(
            &client,
            &portal_url,
            vec![
                raw_cookie("COOKIE_A", "VAL_A", "tw.beanfun.com"),
                raw_cookie("COOKIE_B", "VAL_B", "tw.beanfun.com"),
            ],
        );

        let mut seen = 0usize;
        let result: Result<usize, &'static str> = seed_webview_cookies_from_client(&client, |_c| {
            seen += 1;
            // Bail on the very first cookie handed to us.
            Err("simulated set_cookie failure")
        });

        assert_eq!(result, Err("simulated set_cookie failure"));
        assert_eq!(
            seen, 1,
            "first Err must abort the loop (sink invoked exactly once)"
        );
    }

    /// Host-only cookie rehydration — a cookie inserted **without** a
    /// `Domain` attribute lands in the store as `CookieDomain::HostOnly`
    /// and its `RawCookie::domain()` is `None`. The seed helper MUST
    /// stamp the resolved host back onto the yielded RawCookie before
    /// handing it to the WebView, otherwise `window.set_cookie`
    /// silently drops the entry on WebView2 (Windows) and the portal's
    /// `pSKey` can't be correlated on `return.aspx`. Pins the regression
    /// observed in the 2026-04-18 live test.
    #[test]
    fn seed_rehydrates_domain_attribute_on_host_only_cookies() {
        let client = tw_client();
        let login_url: Url = "https://login.beanfun.com/".parse().expect("valid url");

        let host_only = RawCookie::parse("ASP.NET_SessionId=LIVE_SID; Path=/")
            .expect("cookie parses")
            .into_owned();
        // Sanity: the parsed RawCookie itself has no Domain — this is
        // the exact shape beanfun's portal emits on this origin.
        assert!(
            host_only.domain().is_none(),
            "fixture precondition: host-only cookie has no Domain attribute",
        );

        inject_webview_cookies(&client, &login_url, vec![host_only]);

        let mut seen: Vec<RawCookie<'static>> = Vec::new();
        let count = seed_webview_cookies_from_client(&client, |cookie| {
            seen.push(cookie);
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible sink");

        assert_eq!(count, 1, "host-only cookie must be seeded, not skipped");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].name(), "ASP.NET_SessionId");
        assert_eq!(
            seen[0].domain(),
            Some("login.beanfun.com"),
            "seed MUST stamp the resolved host back onto the RawCookie's Domain attribute",
        );
    }

    /// A cookie inserted with an explicit `Domain=` suffix lands in
    /// the store as `CookieDomain::Suffix` and its `RawCookie::domain()`
    /// already carries the suffix. The seed helper's rehydration must
    /// be a no-op in this case (set_domain with the same suffix is
    /// idempotent), NOT drop or rewrite the suffix.
    #[test]
    fn seed_preserves_explicit_domain_on_suffix_cookies() {
        let client = tw_client();
        let portal_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");

        inject_webview_cookies(
            &client,
            &portal_url,
            vec![raw_cookie("bfWebToken", "WT_SUFFIX", "tw.beanfun.com")],
        );

        let mut seen: Vec<RawCookie<'static>> = Vec::new();
        let _ = seed_webview_cookies_from_client(&client, |cookie| {
            seen.push(cookie);
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible sink");

        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].name(), "bfWebToken");
        assert_eq!(
            seen[0].domain(),
            Some("tw.beanfun.com"),
            "explicit Domain attribute must be preserved verbatim",
        );
    }

    /// Cookies yielded to the sink are **owned** `'static` clones —
    /// the sink can stash them in a `Vec<RawCookie<'static>>` without
    /// borrowing from the jar. Pins this because the production
    /// callsite (command layer) passes cookies to
    /// `tauri::WebviewWindow::set_cookie` across thread / dispatcher
    /// boundaries, which requires owned values.
    #[test]
    fn seed_yields_owned_static_cookies() {
        let client = tw_client();
        let portal_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        inject_webview_cookies(
            &client,
            &portal_url,
            vec![raw_cookie("OWNED_TEST", "VAL", "tw.beanfun.com")],
        );

        let mut stash: Vec<RawCookie<'static>> = Vec::new();
        let count = seed_webview_cookies_from_client(&client, |cookie| {
            // If `cookie` weren't `'static` this wouldn't compile.
            stash.push(cookie);
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("infallible sink");

        assert_eq!(count, 1);
        assert_eq!(stash.len(), 1);
        assert_eq!(stash[0].name(), "OWNED_TEST");
        assert_eq!(stash[0].value(), "VAL");
    }

    // ── try_complete_gamepass_login ───────────────────────────────

    #[test]
    fn try_complete_returns_none_when_web_token_missing() {
        let client = tw_client();

        let outcome = try_complete_gamepass_login(&client, "SKEY_GP", "610074", "T9");

        assert!(
            outcome.is_none(),
            "no bfWebToken → None (WPF TryCompleteLogin L143-144 early-return)",
        );
    }

    #[test]
    fn try_complete_returns_session_when_web_token_present_on_portal_origin() {
        let client = tw_client();
        let source_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        inject_webview_cookies(
            &client,
            &source_url,
            vec![raw_cookie(
                "bfWebToken",
                "WTOKEN_GP_HAPPY",
                "tw.beanfun.com",
            )],
        );

        let session = try_complete_gamepass_login(&client, "SKEY_GP", "610074", "T9")
            .expect("bfWebToken present → Some(session)");

        assert_eq!(session.region, LoginRegion::TW);
        assert_eq!(session.skey, "SKEY_GP");
        assert_eq!(session.web_token, "WTOKEN_GP_HAPPY");
        assert_eq!(
            session.account_id, "",
            "GamePass defers account resolution — same sentinel as QR flow",
        );
        assert_eq!(session.service_code, "610074");
        assert_eq!(session.service_region, "T9");
    }

    #[test]
    fn try_complete_returns_none_when_token_visible_only_on_foreign_origin() {
        // If the WebView injected bfWebToken into, say, `login.beanfun.com`
        // instead of the portal, the portal-scoped lookup must miss it
        // and we must keep waiting for the next page load. This pins
        // the "scope matters" invariant shared with `completed.rs`.
        let client = tw_client();
        let login_url: Url = "https://login.beanfun.com/".parse().expect("valid url");

        inject_webview_cookies(
            &client,
            &login_url,
            vec![raw_cookie(
                "bfWebToken",
                "WT_WRONG_SCOPE",
                "login.beanfun.com",
            )],
        );

        assert!(
            try_complete_gamepass_login(&client, "SKEY_GP", "610074", "T9").is_none(),
            "bfWebToken on non-portal origin must NOT resolve as success",
        );
    }

    #[test]
    fn try_complete_preserves_all_session_metadata_verbatim() {
        // Structural test: every caller-supplied argument lands on
        // the Session without silent rewriting. Guards against a
        // future edit accidentally hard-coding service codes (the
        // way WPF `GamePassLogin` L807 defaults them) when the
        // public-API contract is to echo the caller's values.
        let client = tw_client();
        let source_url: Url = "https://tw.beanfun.com/".parse().expect("valid url");
        inject_webview_cookies(
            &client,
            &source_url,
            vec![raw_cookie("bfWebToken", "WT", "tw.beanfun.com")],
        );

        let session =
            try_complete_gamepass_login(&client, "CUSTOM_SKEY", "999999", "CUSTOM_REGION")
                .expect("bfWebToken present");

        assert_eq!(session.service_code, "999999");
        assert_eq!(session.service_region, "CUSTOM_REGION");
        assert_eq!(session.skey, "CUSTOM_SKEY");
    }
}
