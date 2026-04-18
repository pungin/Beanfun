//! End-to-end integration tests for the QR-code finalize step
//! (`login/qr_finalize.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives [`finalize_qr_login`] against
//! one canned **four-step** response chain that exercises one branch
//! of the WPF QR flow (`BeanfunClient.Login.cs::QRCodeLogin` L530-607
//! plus `LoginCompleted` L838-882).
//!
//! | WPF branch / wire-shape detail                                  | Covered by                                                       |
//! |-----------------------------------------------------------------|------------------------------------------------------------------|
//! | happy path → Session populated, web_token comes from step 4     | `happy_path_returns_session_with_step4_web_token`                |
//! | HK region guard, no HTTP traffic                                | `hk_region_returns_qr_unsupported_without_http_traffic`          |
//! | step 1 (`QRLogin/QRLogin`) HTTP 5xx                             | `qrlogin_handshake_failure_propagates_as_unknown`                |
//! | step 2 (`Login/SendLogin`) empty form                           | `send_login_empty_form_yields_send_login_no_form_data`           |
//! | step 3 missing bfWebToken → tolerated, step 4 still runs        | `step3_missing_set_cookie_is_tolerated_and_continues_to_step4`   |
//! | step 4 (`return.aspx` AuthKey=OK form) missing bfWebToken cookie| `step4_login_completed_missing_token_yields_missing_web_token`   |
//! | step 1 wire shape — Accept=JSON, Referer=Index URL              | `step1_qrlogin_handshake_sends_expected_headers`                 |
//! | step 2 wire shape — Accept=QR-specific HTML, Referer            | `step2_send_login_sends_qr_specific_html_accept`                 |
//! | step 3 wire shape — SendLogin form body + Referer=login_base    | `step3_return_aspx_posts_send_login_form_with_login_base_referer`|
//! | step 4 wire shape — 5-field AuthKey=OK form                     | `step4_login_completed_posts_five_field_form_with_authkey_ok`    |
//! | step 3 → step 4 sequencing                                      | `steps_3_and_4_post_return_aspx_in_that_order`                   |
//! | Session.account_id is empty (deferred to GetAccounts)           | `session_account_id_is_empty_pending_get_accounts`               |
//!
//! Pure-helper unit tests (Accept-string locks) live next to the
//! source module; this file covers the HTTP orchestration end-to-end.

use beanfun_next_lib::services::beanfun::{
    login::{finalize_qr_login, QrLoginInit},
    BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION_KEY: &str = "SKEY_QR_FIN";
const VERIFICATION_TOKEN: &str = "VTOKEN_qr_fin_xyz";

/// Token returned by the **canonical** step (4). This is what should
/// end up on `Session.web_token` after a happy roundtrip — step 3's
/// token is intentionally discarded by `finalize_qr_login`. See the
/// `qr_finalize` module docs for the WPF L868 alignment rationale.
const STEP4_WEB_TOKEN: &str = "BFWT_qr_fin_step4_canonical";

/// Token returned by step 3. We mount it with a *distinct* value from
/// [`STEP4_WEB_TOKEN`] so tests can prove `finalize_qr_login` returns
/// the step 4 value (and not accidentally surface step 3's).
const STEP3_DISCARDED_TOKEN: &str = "BFWT_qr_fin_step3_discarded";

/// Accept string WPF's `QRCodeLogin` sends on the SendLogin GET
/// (L545). Reproduced here verbatim so the wire-shape test asserts
/// what we actually expect to see on the wire instead of trusting
/// the source-code constant.
const EXPECTED_QR_SEND_LOGIN_ACCEPT: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,\
     image/avif,image/webp,image/apng,*/*;q=0.8";

// -----------------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------------

/// Canned [`QrLoginInit`] for tests — bundles the skey + token the
/// finalize function needs without standing up a full
/// init+poll flow first. Mirrors what `init_qr_login` would have
/// produced after the user scanned and the poll returned `Approved`.
fn fake_init() -> QrLoginInit {
    QrLoginInit {
        skey: SESSION_KEY.to_owned(),
        bitmap_base64: "data:image/png;base64,IGNORED_FOR_FINALIZE".to_owned(),
        deeplink: None,
        verification_token: VERIFICATION_TOKEN.to_owned(),
    }
}

/// Build a [`BeanfunClient`] whose login_base / portal_base /
/// newlogin_base all point at `server`. Region is parameterised so
/// the HK guard test can use the same builder.
fn client_for(server: &MockServer, region: LoginRegion) -> BeanfunClient {
    let base = Url::parse(&format!("{}/", server.uri())).expect("mock URL parses");
    let endpoints = Endpoints {
        login_base: base.clone(),
        portal_base: base.clone(),
        newlogin_base: base,
    };
    let mut cfg = ClientConfig::for_region(region);
    cfg.endpoints = endpoints;
    BeanfunClient::new(cfg).expect("client builds")
}

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step
// -----------------------------------------------------------------------------

/// `GET /QRLogin/QRLogin` — handshake step. Body is discarded by the
/// production code, so any payload works; we send a token sentinel
/// to confirm we're not accidentally parsing it.
async fn mount_qrlogin_handshake_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/QRLogin/QRLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_string("HANDSHAKE_BODY_DISCARDED"))
        .mount(server)
        .await;
}

/// `GET /QRLogin/QRLogin` — handshake step returning a 5xx so we can
/// drive the [`LoginError::Unknown`] branch out of `ensure_success`.
async fn mount_qrlogin_handshake_500(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/QRLogin/QRLogin"))
        .respond_with(ResponseTemplate::new(500).set_body_string("oops"))
        .mount(server)
        .await;
}

/// `GET /Login/SendLogin` — responds with the given HTML body.
async fn mount_send_login_with_html(server: &MockServer, html: &str) {
    Mock::given(method("GET"))
        .and(path("/Login/SendLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html.to_owned()))
        .mount(server)
        .await;
}

/// `GET /Login/SendLogin` — happy-path form with three hidden inputs
/// (mirrors the TW Regular fixture for parity).
async fn mount_send_login_happy(server: &MockServer) {
    let html = r#"<html><body>
        <form action="/beanfun_block/bflogin/return.aspx" method="post">
            <input type="hidden" name="SessionKey" value="SKEY_INNER" />
            <input type="hidden" name="AuthKey" value="AUTH_INNER" />
            <input type="hidden" name="ServiceCode" value="610074" />
            <input type="submit" name="btn" value="go" />
        </form>
    </body></html>"#;
    mount_send_login_with_html(server, html).await;
}

/// `POST /beanfun_block/bflogin/return.aspx` for **step 3** (the
/// SendLogin-form POST inside `QRCodeLogin`, WPF L588-591). The form
/// body always carries `AuthKey=AUTH_INNER` — the value we hardcode
/// inside [`mount_send_login_happy`]'s HTML — so we discriminate on
/// that fragment and let step 4's mock handle the `AuthKey=OK` POST.
///
/// Returns a 302 with `bfWebToken={token}; Path=/; HttpOnly`. Pass
/// [`STEP3_DISCARDED_TOKEN`] in happy tests so a regression that
/// surfaces step 3's token on `Session.web_token` is observable.
async fn mount_return_aspx_step3_with_token(server: &MockServer, token: &str) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .and(body_string_contains("AuthKey=AUTH_INNER"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str())
                .append_header(
                    "Set-Cookie",
                    format!("bfWebToken={token}; Path=/; HttpOnly").as_str(),
                ),
        )
        .mount(server)
        .await;
}

/// Step 3 mock that responds **without** a `bfWebToken` cookie.
/// `finalize_qr_login` tolerates this (WPF L591-598 parity —
/// the canonical cookie comes from step 4); see
/// `step3_missing_set_cookie_is_tolerated_and_continues_to_step4`.
async fn mount_return_aspx_step3_without_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .and(body_string_contains("AuthKey=AUTH_INNER"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str()),
        )
        .mount(server)
        .await;
}

/// `POST /beanfun_block/bflogin/return.aspx` for **step 4** (the
/// `LoginCompleted` POST with the 5-field `AuthKey=OK` payload, WPF
/// L853-864). Discriminator: `AuthKey=OK` in the URL-encoded body.
///
/// Pass [`STEP4_WEB_TOKEN`] in happy tests so the assertion that
/// `Session.web_token == STEP4_WEB_TOKEN` proves we propagated step
/// 4's value (not step 3's).
async fn mount_return_aspx_step4_with_token(server: &MockServer, token: &str) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .and(body_string_contains("AuthKey=OK"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str())
                .append_header(
                    "Set-Cookie",
                    format!("bfWebToken={token}; Path=/; HttpOnly").as_str(),
                ),
        )
        .mount(server)
        .await;
    mount_after_landing(server).await;
}

/// Step 4 mock that responds **without** a `bfWebToken` cookie.
/// Drives the canonical `MissingWebToken` failure surface (this is
/// the only path that actually surfaces to the caller because step
/// 3's token is discarded; if step 3 succeeds and step 4 fails, the
/// returned error is the one users would see).
async fn mount_return_aspx_step4_without_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .and(body_string_contains("AuthKey=OK"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str()),
        )
        .mount(server)
        .await;
    mount_after_landing(server).await;
}

/// `GET /after` → `200 OK` landing page. Step 4 uses
/// [`login_completed`] which auto-follows redirects (WPF L863
/// parity), so the 302 above needs a reachable target or reqwest
/// surfaces 404 as `LoginError::Unknown`. Step 3 stays on
/// `post_return_aspx` (no-redirect) and never hits this endpoint,
/// but mounting it unconditionally keeps the per-step fixtures
/// symmetric and harmless.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

/// One-stop shop for tests that just need a fully-mounted happy path.
/// Mounts both step 3 (with [`STEP3_DISCARDED_TOKEN`]) and step 4
/// (with [`STEP4_WEB_TOKEN`]) so the happy path can prove which one
/// ends up on `Session.web_token`.
async fn mount_happy_path(server: &MockServer) {
    mount_qrlogin_handshake_ok(server).await;
    mount_send_login_happy(server).await;
    mount_return_aspx_step3_with_token(server, STEP3_DISCARDED_TOKEN).await;
    mount_return_aspx_step4_with_token(server, STEP4_WEB_TOKEN).await;
}

// -----------------------------------------------------------------------------
// Happy path
// -----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_returns_session_with_step4_web_token() {
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let session = finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy path must succeed");

    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, SESSION_KEY);
    // Critical assertion: web_token must be the value from step 4
    // (LoginCompleted), NOT step 3. Mirrors WPF L868
    // `this.webtoken = this.GetCookie("bfWebToken")` — the cookie
    // jar value AFTER the second POST. If a refactor ever flips
    // back to "use step 3's token and skip step 4", this assertion
    // will fail loudly.
    assert_eq!(
        session.web_token, STEP4_WEB_TOKEN,
        "web_token must come from step 4 (LoginCompleted), not step 3"
    );
    assert_ne!(
        session.web_token, STEP3_DISCARDED_TOKEN,
        "step 3's transient token must not surface on the Session"
    );
    // QR has no user-typed account id; surfaced as empty until the
    // P3.5 `GetAccounts` step fills it. See `qr_finalize` module docs.
    assert_eq!(session.account_id, "");
    // TW defaults — same as TW Regular.
    assert_eq!(session.service_code, "610074");
    assert_eq!(session.service_region, "T9");
}

// -----------------------------------------------------------------------------
// Region guard — short-circuits BEFORE any HTTP traffic
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_region_returns_qr_unsupported_without_http_traffic() {
    // No mocks mounted. If the guard fails to fire, step 1 would
    // 404 against an empty wiremock and surface as
    // `LoginError::Unknown(... HTTP 404)` instead of
    // `QrUnsupportedRegion`. The explicit `received_requests` check
    // belt-and-braces it.
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);

    let err = finalize_qr_login(&client, &fake_init())
        .await
        .expect_err("HK region must refuse QR finalize");
    assert!(matches!(err, LoginError::QrUnsupportedRegion));

    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "HK guard must short-circuit before sending any HTTP traffic"
    );
}

// -----------------------------------------------------------------------------
// Per-step error paths
// -----------------------------------------------------------------------------

#[tokio::test]
async fn qrlogin_handshake_failure_propagates_as_unknown() {
    // Step 1 returns 500 → ensure_success collapses to LoginError::Unknown.
    // Step 2 / 3 mocks are deliberately omitted — the failure must
    // short-circuit before they're hit.
    let server = MockServer::start().await;
    mount_qrlogin_handshake_500(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let err = finalize_qr_login(&client, &fake_init())
        .await
        .expect_err("step 1 5xx must surface as a typed error");
    match err {
        LoginError::Unknown(msg) => assert!(
            msg.contains("QRLogin/QRLogin") && msg.contains("500"),
            "Unknown message should mention the step and HTTP status, got: {msg}"
        ),
        other => panic!("expected LoginError::Unknown, got {other:?}"),
    }

    // Verify subsequent steps were not attempted.
    let received = server.received_requests().await.unwrap();
    assert!(
        received.iter().all(|r| r.url.path() == "/QRLogin/QRLogin"),
        "step 1 failure must short-circuit; saw: {:?}",
        received
            .iter()
            .map(|r| r.url.path().to_owned())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn send_login_empty_form_yields_send_login_no_form_data() {
    // Step 1 succeeds, step 2 returns HTML with no <input> tags →
    // `send_login` returns SendLoginNoFormData (WPF L582-586
    // `errmsg = "SendLoginNoFormData"`).
    let server = MockServer::start().await;
    mount_qrlogin_handshake_ok(&server).await;
    mount_send_login_with_html(&server, "<html><body>oops</body></html>").await;
    let client = client_for(&server, LoginRegion::TW);

    let err = finalize_qr_login(&client, &fake_init())
        .await
        .expect_err("empty SendLogin must error");
    assert!(
        matches!(err, LoginError::SendLoginNoFormData),
        "expected SendLoginNoFormData, got {err:?}"
    );
}

#[tokio::test]
async fn step3_missing_set_cookie_is_tolerated_and_continues_to_step4() {
    // Steps 1 & 2 succeed; step 3 returns 302 **without** a
    // `bfWebToken` cookie; step 4 returns 302 WITH the cookie.
    //
    // Locks WPF parity: `QRCodeLogin` L591-598 wraps the step 3
    // cookie read inside `if (!string.IsNullOrEmpty(setCookieHeader))`
    // and silently falls through to `return "OK"` (L600) when the
    // server omits the cookie. The canonical `bfWebToken` comes from
    // step 4's `LoginCompleted` (L868 `GetCookie("bfWebToken")`), so
    // step 3's token is only a side effect — missing it is NOT a
    // fatal condition.
    //
    // Before this test was flipped (2026-04-16 hotfix), a strict
    // `MissingWebToken` at step 3 surfaced to the UI as
    // `auth.missing_web_token` even on fully-valid QR sessions,
    // because beanfun's live TW server does sometimes omit the
    // `Set-Cookie` header on this hop. See `qr_finalize.rs` module
    // "Leniency on missing bfWebToken" section.
    let server = MockServer::start().await;
    mount_qrlogin_handshake_ok(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_step3_without_token(&server).await;
    mount_return_aspx_step4_with_token(&server, STEP4_WEB_TOKEN).await;
    let client = client_for(&server, LoginRegion::TW);

    let session = finalize_qr_login(&client, &fake_init())
        .await
        .expect("step 3 MissingWebToken must be tolerated — WPF L591-598 parity");

    // Step 4's token is the one that should surface on Session.
    assert_eq!(
        session.web_token, STEP4_WEB_TOKEN,
        "web_token must come from step 4 even when step 3 omitted the cookie"
    );

    // Belt-and-braces: step 3 AND step 4 both reached the server.
    // If step 3 had erred (pre-hotfix behaviour), we'd see exactly
    // ONE return.aspx hit instead of two.
    let received = server.received_requests().await.unwrap();
    let return_aspx_hits = received
        .iter()
        .filter(|r| r.url.path() == "/beanfun_block/bflogin/return.aspx")
        .count();
    assert_eq!(
        return_aspx_hits, 2,
        "step 3 tolerance must let step 4 run (saw {return_aspx_hits} return.aspx calls)"
    );
}

#[tokio::test]
async fn step4_login_completed_missing_token_yields_missing_web_token() {
    // Steps 1 & 2 succeed; neither step 3 NOR step 4 sets
    // `bfWebToken`. The cookie jar therefore stays empty through the
    // whole flow, and `login_completed` surfaces
    // `LoginError::MissingWebToken` after the redirect chain settles.
    //
    // Why both steps must omit the cookie (vs. "just step 4"): under
    // WPF parity, `login_completed` reads `bfWebToken` from the
    // shared cookie jar (WPF L868 `GetCookie("bfWebToken")`), not
    // from step 4's immediate `Set-Cookie` header. reqwest records
    // every `Set-Cookie` observed on any hop — step 3's included —
    // into that shared jar, so if step 3 supplies a token the jar
    // is NOT empty when step 4 reads it, regardless of whether step
    // 4 itself sets the cookie. Mirroring WPF behaviour on that
    // point: `LoginCompleted` would also return step 3's token in
    // the same scenario. The canonical MissingWebToken surface is
    // therefore "no hop in the chain set bfWebToken", not "step 4
    // specifically omitted it".
    //
    // WPF parallel: `LoginCompleted` L868-873 sets `errmsg =
    // "LoginNoWebtoken"` when `GetCookie("bfWebToken") == ""` after
    // the POST. We surface the same condition as
    // `LoginError::MissingWebToken`.
    let server = MockServer::start().await;
    mount_qrlogin_handshake_ok(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_step3_without_token(&server).await;
    mount_return_aspx_step4_without_token(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let err = finalize_qr_login(&client, &fake_init())
        .await
        .expect_err("empty cookie jar after step 4 must surface as MissingWebToken");
    assert!(
        matches!(err, LoginError::MissingWebToken),
        "expected MissingWebToken, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Wire-shape assertions — assert against `received_requests` so a
// mismatch reports which header diverged instead of a silent 404.
// -----------------------------------------------------------------------------

fn header_value<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
    req.headers.get(name).and_then(|v| v.to_str().ok())
}

#[tokio::test]
async fn step1_qrlogin_handshake_sends_expected_headers() {
    // WPF L535-540:
    //   SetBaseHeaders(true,
    //                  "application/json, text/plain, */*",
    //                  $"https://login.beanfun.com/Login/Index?pSKey={skey}");
    //   DownloadString("https://login.beanfun.com/QRLogin/QRLogin");
    //
    // Expected on the wire:
    //   Accept:  application/json, text/plain, */*
    //   Referer: {login_base}Login/Index?pSKey={skey}
    //   (no Origin, no X-Requested-With, no RequestVerificationToken
    //    — `SetBaseHeaders` clears the slate first.)
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/QRLogin/QRLogin")
        .expect("step 1 request was sent");

    assert_eq!(
        header_value(req, "Accept"),
        Some("application/json, text/plain, */*"),
    );
    let expected_referer = format!("{}/Login/Index?pSKey={}", server.uri(), SESSION_KEY);
    assert_eq!(
        header_value(req, "Referer"),
        Some(expected_referer.as_str()),
    );

    // Sanity: the headers WPF clears and never re-adds in QRCodeLogin
    // step 1 must NOT appear on the wire.
    for omitted in ["Origin", "X-Requested-With", "RequestVerificationToken"] {
        assert!(
            req.headers.get(omitted).is_none(),
            "step 1 must NOT send `{omitted}` (WPF SetBaseHeaders cleared it)"
        );
    }
}

#[tokio::test]
async fn step2_send_login_sends_qr_specific_html_accept() {
    // WPF L543-550:
    //   SetBaseHeaders(true,
    //                  "text/html,application/xhtml+xml,application/xml;q=0.9,
    //                   image/avif,image/webp,image/apng,*/*;q=0.8",
    //                  $"https://login.beanfun.com/Login/Index?pSKey={skey}");
    //   DownloadString("https://login.beanfun.com/Login/SendLogin");
    //
    // The Accept value differs from the TW Regular flow's L124 string
    // (which omits the three image/* tokens) — the whole point of
    // parameterising `send_login`'s `accept` argument is to keep both
    // wire shapes byte-identical to WPF.
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/Login/SendLogin")
        .expect("step 2 request was sent");

    assert_eq!(
        header_value(req, "Accept"),
        Some(EXPECTED_QR_SEND_LOGIN_ACCEPT),
        "step 2 Accept must match WPF L545 byte-for-byte (with image/* tokens)"
    );
    let expected_referer = format!("{}/Login/Index?pSKey={}", server.uri(), SESSION_KEY);
    assert_eq!(
        header_value(req, "Referer"),
        Some(expected_referer.as_str()),
    );
}

/// Find the first `return.aspx` POST whose body contains
/// `discriminator`. Both step 3 and step 4 hit the same path; the
/// only practical way to tell them apart from `received_requests` is
/// the body content.
fn find_return_aspx_request<'a>(
    requests: &'a [wiremock::Request],
    discriminator: &str,
) -> Option<&'a wiremock::Request> {
    requests.iter().find(|r| {
        r.url.path() == "/beanfun_block/bflogin/return.aspx"
            && std::str::from_utf8(&r.body)
                .map(|body| body.contains(discriminator))
                .unwrap_or(false)
    })
}

#[tokio::test]
async fn step3_return_aspx_posts_send_login_form_with_login_base_referer() {
    // WPF L588-591:
    //   SetBaseHeaders(true, null, "https://login.beanfun.com/");
    //   UploadString("https://tw.beanfun.com/beanfun_block/bflogin/return.aspx",
    //                payload);  // payload = SendLogin form scrape
    //
    // The `accept = null` argument means `SetBaseHeaders` skips the
    // `Accept` header entirely. We don't *explicitly* set Accept in
    // `post_return_aspx` either, but reqwest 0.12 (via hyper) auto-
    // injects `Accept: */*` on every request and there's no public
    // API to suppress it short of swapping HTTP clients.
    //
    // **Intentional divergence**: WPF sends no Accept; we send
    // `Accept: */*`. Semantically inert — `*/*` is exactly the
    // implicit default an HTTP server uses when Accept is absent
    // (RFC 9110 §12.5.1) — but explicitly documented so a future
    // reader doesn't think this gap is a bug.
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = find_return_aspx_request(&received, "AuthKey=AUTH_INNER")
        .expect("step 3 (SendLogin form) POST was sent");

    // Referer = login_base with trailing slash. We point all bases at
    // the same mock origin; `Url::as_str()` canonicalises the trailing
    // slash so this matches what `post_return_aspx` actually sends.
    let expected_referer = format!("{}/", server.uri());
    assert_eq!(
        header_value(req, "Referer"),
        Some(expected_referer.as_str()),
    );

    // Lock the documented divergence — anything other than absent or
    // `*/*` would be a real wire-shape change worth investigating.
    let accept_header = header_value(req, "Accept");
    assert!(
        accept_header.is_none() || accept_header == Some("*/*"),
        "step 3 Accept must be absent or `*/*` (reqwest default), got: {accept_header:?}"
    );

    // Body shape — `.form(form)` URL-encodes; the inner hidden inputs
    // from `mount_send_login_happy` should all appear.
    let body_str = std::str::from_utf8(&req.body).expect("form body is utf-8");
    for fragment in [
        "SessionKey=SKEY_INNER",
        "AuthKey=AUTH_INNER",
        "ServiceCode=610074",
    ] {
        assert!(
            body_str.contains(fragment),
            "step 3 form body missing `{fragment}`; got: {body_str}"
        );
    }
    // `.form()` sets Content-Type for us — verify so a future
    // refactor that drops it gets caught.
    assert_eq!(
        header_value(req, "Content-Type"),
        Some("application/x-www-form-urlencoded"),
    );
}

#[tokio::test]
async fn step4_login_completed_posts_five_field_form_with_authkey_ok() {
    // WPF L853-864 (LoginCompleted):
    //   payload.Add("SessionKey", this.SessionKey);
    //   payload.Add("AuthKey", akey);              // akey = "OK" for QR
    //   payload.Add("ServiceCode", "");
    //   payload.Add("ServiceRegion", "");
    //   payload.Add("ServiceAccountSN", "0");
    //   UploadString("https://tw.beanfun.com/beanfun_block/bflogin/return.aspx",
    //                payload);
    //
    // SessionKey here is the *outer* skey (init.skey == SESSION_KEY),
    // NOT the SendLogin-form's `SessionKey=SKEY_INNER` from step 3.
    // ServiceCode/Region are blank on the wire by design — see
    // `login/completed.rs` module docs L19-23.
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = find_return_aspx_request(&received, "AuthKey=OK")
        .expect("step 4 (LoginCompleted AuthKey=OK form) POST was sent");

    // Same Referer + Accept divergence story as step 3 — they share
    // the `post_return_aspx` helper.
    let expected_referer = format!("{}/", server.uri());
    assert_eq!(
        header_value(req, "Referer"),
        Some(expected_referer.as_str()),
    );

    let body_str = std::str::from_utf8(&req.body).expect("form body is utf-8");
    // Five-field LoginCompleted form. Use `SessionKey={SESSION_KEY}`
    // to disambiguate from step 3's `SessionKey=SKEY_INNER`. Bind
    // the formatted fragment to a `String` first so the array below
    // is uniformly `&str` (otherwise type inference on `contains`
    // gets ambiguous with a mixed `&String`/`&str` array).
    let session_key_fragment = format!("SessionKey={SESSION_KEY}");
    for fragment in [
        session_key_fragment.as_str(),
        "AuthKey=OK",
        "ServiceCode=&",
        "ServiceRegion=&",
        "ServiceAccountSN=0",
    ] {
        assert!(
            body_str.contains(fragment),
            "step 4 form body missing `{fragment}`; got: {body_str}"
        );
    }
    // The SendLogin-form fields from step 3 must NOT leak into
    // step 4's body. Catches an accidental form-reuse refactor.
    assert!(
        !body_str.contains("SKEY_INNER"),
        "step 4 form body must not contain step 3's SKEY_INNER; got: {body_str}"
    );
    assert!(
        !body_str.contains("AUTH_INNER"),
        "step 4 form body must not contain step 3's AUTH_INNER; got: {body_str}"
    );
}

#[tokio::test]
async fn steps_3_and_4_post_return_aspx_in_that_order() {
    // Both steps hit the same path; the only way to verify ordering
    // is to walk `received_requests` (which preserves arrival order)
    // and assert the AuthKey fragments appear in the right sequence.
    // A regression that swapped the two posts (or accidentally
    // skipped step 4 by reverting to the old "redundant" reading)
    // would fail this test loudly.
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip");

    let received = server.received_requests().await.expect("requests recorded");
    let return_aspx_posts: Vec<_> = received
        .iter()
        .filter(|r| r.url.path() == "/beanfun_block/bflogin/return.aspx")
        .collect();

    assert_eq!(
        return_aspx_posts.len(),
        2,
        "expected exactly two return.aspx POSTs (step 3 + step 4), got {}",
        return_aspx_posts.len()
    );

    let body0 = std::str::from_utf8(&return_aspx_posts[0].body).unwrap();
    let body1 = std::str::from_utf8(&return_aspx_posts[1].body).unwrap();
    assert!(
        body0.contains("AuthKey=AUTH_INNER"),
        "first return.aspx POST should be step 3 (SendLogin form), got body: {body0}"
    );
    assert!(
        body1.contains("AuthKey=OK"),
        "second return.aspx POST should be step 4 (LoginCompleted), got body: {body1}"
    );
}

// -----------------------------------------------------------------------------
// Session shape — explicit lock on the QR-specific account_id design
// -----------------------------------------------------------------------------

#[tokio::test]
async fn session_account_id_is_empty_pending_get_accounts() {
    // QR mode never asks the user for an account id (the mobile app
    // resolves it server-side). We surface that as `account_id = ""`
    // to be filled in by P3.5's `GetAccounts`. Locking this here so
    // a future refactor that defaults it to something else (e.g.
    // skey) gets caught.
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let session = finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy roundtrip");

    assert_eq!(
        session.account_id, "",
        "QR Session.account_id must be empty until GetAccounts populates it"
    );
}
