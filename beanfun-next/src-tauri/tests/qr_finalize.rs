//! End-to-end integration tests for the QR-code finalize step
//! (`login/qr_finalize.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives [`finalize_qr_login`] against
//! one canned three-step response chain that exercises one branch of
//! the WPF `QRCodeLogin` flow (`BeanfunClient.Login.cs` L530-607).
//!
//! | WPF branch / wire-shape detail                           | Covered by                                                |
//! |----------------------------------------------------------|-----------------------------------------------------------|
//! | happy path → Session populated                           | `happy_path_returns_session`                              |
//! | HK region guard, no HTTP traffic                         | `hk_region_returns_qr_unsupported_without_http_traffic`   |
//! | step 1 (`QRLogin/QRLogin`) HTTP 5xx                      | `qrlogin_handshake_failure_propagates_as_unknown`         |
//! | step 2 (`Login/SendLogin`) empty form                    | `send_login_empty_form_yields_send_login_no_form_data`    |
//! | step 3 (`return.aspx`) missing bfWebToken cookie         | `return_aspx_missing_set_cookie_yields_missing_web_token` |
//! | step 1 wire shape — Accept=JSON, Referer=Index URL       | `step1_qrlogin_handshake_sends_expected_headers`          |
//! | step 2 wire shape — Accept=QR-specific HTML, Referer     | `step2_send_login_sends_qr_specific_html_accept`          |
//! | step 3 wire shape — Referer=login_base, form body, …    | `step3_return_aspx_sends_login_base_referer_and_form_body`|
//! | Session.account_id is empty (deferred to GetAccounts)    | `session_account_id_is_empty_pending_get_accounts`        |
//!
//! Pure-helper unit tests (Accept-string locks) live next to the
//! source module; this file covers the HTTP orchestration end-to-end.

use beanfun_next_lib::services::beanfun::{
    login::{finalize_qr_login, QrLoginInit},
    BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION_KEY: &str = "SKEY_QR_FIN";
const VERIFICATION_TOKEN: &str = "VTOKEN_qr_fin_xyz";
const WEB_TOKEN: &str = "BFWT_qr_fin_happy";

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

/// `POST /beanfun_block/bflogin/return.aspx` — 302 redirect carrying
/// a `bfWebToken=…` Set-Cookie.
async fn mount_return_aspx_with_token(server: &MockServer, token: &str) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
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

/// `POST /beanfun_block/bflogin/return.aspx` — 302 redirect *without*
/// the `bfWebToken` cookie. Drives the [`LoginError::MissingWebToken`]
/// branch.
async fn mount_return_aspx_without_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str()),
        )
        .mount(server)
        .await;
}

/// One-stop shop for tests that just need a fully-mounted happy path.
async fn mount_happy_path(server: &MockServer) {
    mount_qrlogin_handshake_ok(server).await;
    mount_send_login_happy(server).await;
    mount_return_aspx_with_token(server, WEB_TOKEN).await;
}

// -----------------------------------------------------------------------------
// Happy path
// -----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_returns_session() {
    let server = MockServer::start().await;
    mount_happy_path(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let session = finalize_qr_login(&client, &fake_init())
        .await
        .expect("happy path must succeed");

    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, SESSION_KEY);
    assert_eq!(session.web_token, WEB_TOKEN);
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
async fn return_aspx_missing_set_cookie_yields_missing_web_token() {
    // Steps 1 & 2 succeed; step 3 returns 302 without a
    // `bfWebToken` cookie → `post_return_aspx` returns
    // MissingWebToken. This is the WPF "logged in but cookie not
    // captured" failure surface (WPF would silently leave
    // this.webtoken null and the next call would fail).
    let server = MockServer::start().await;
    mount_qrlogin_handshake_ok(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_without_token(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let err = finalize_qr_login(&client, &fake_init())
        .await
        .expect_err("missing Set-Cookie must error");
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

#[tokio::test]
async fn step3_return_aspx_sends_login_base_referer_and_form_body() {
    // WPF L588-591:
    //   SetBaseHeaders(true, null, "https://login.beanfun.com/");
    //   UploadString("https://tw.beanfun.com/beanfun_block/bflogin/return.aspx",
    //                payload);
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
    let req = received
        .iter()
        .find(|r| r.url.path() == "/beanfun_block/bflogin/return.aspx")
        .expect("step 3 request was sent");

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
            "form body missing `{fragment}`; got: {body_str}"
        );
    }
    // `.form()` sets Content-Type for us — verify so a future
    // refactor that drops it gets caught.
    assert_eq!(
        header_value(req, "Content-Type"),
        Some("application/x-www-form-urlencoded"),
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
