//! End-to-end integration tests for the QR-code poll step
//! (`login/qr_poll.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives [`poll_qr_login_status`]
//! against one canned response that exercises one branch of the
//! WPF `QRCodeCheckLoginStatus` `ResultMessage` switch
//! (`BeanfunClient.Login.cs` L640-653).
//!
//! | WPF branch (`ResultMessage`) | Covered by                                                 |
//! |------------------------------|------------------------------------------------------------|
//! | `"Failed"`                   | `failed_result_message_returns_failed_outcome`             |
//! | `"Wait Login"`               | `wait_login_result_message_returns_wait_login_outcome`     |
//! | `"Token Expired"`            | `token_expired_result_message_returns_token_expired`       |
//! | `"Success"`                  | `success_result_message_returns_approved_outcome`          |
//! | unknown value                | `unknown_result_message_returns_server_message_with_body`  |
//! | missing field                | `missing_result_message_returns_server_message_with_body`  |
//! | JSON parse failure           | `non_json_body_returns_qr_json_parse_failed`               |
//! | HK region guard              | `hk_region_returns_qr_unsupported_without_http_traffic`    |
//! | wire shape (headers + body)  | `request_carries_expected_headers_and_empty_form_body`     |
//!
//! Pure serde-shape unit tests live next to the source module; this
//! file covers the HTTP orchestration, header set, and the
//! `ResultMessage` dispatch table end-to-end.

use beanfun_lib::services::beanfun::{
    login::{poll_qr_login_status, QrLoginInit, QrPollOutcome},
    BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION_KEY: &str = "SKEY_QR_POLL";
const VERIFICATION_TOKEN: &str = "VTOKEN_qr_poll_xyz";

// -----------------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------------

/// Canned [`QrLoginInit`] for tests — bundles the skey + token the
/// poll function needs without standing up a full `init_qr_login`
/// flow first. Mirrors what `init_qr_login` would have produced.
fn fake_init() -> QrLoginInit {
    QrLoginInit {
        skey: SESSION_KEY.to_owned(),
        bitmap_base64: "data:image/png;base64,IGNORED_FOR_POLL".to_owned(),
        deeplink: None,
        verification_token: VERIFICATION_TOKEN.to_owned(),
    }
}

fn fake_init_without_verification_token() -> QrLoginInit {
    QrLoginInit {
        verification_token: String::new(),
        ..fake_init()
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
// Mock setup helpers — one per body shape
// -----------------------------------------------------------------------------

/// Mount `POST /QRLogin/CheckLoginStatus` returning the given JSON
/// `ResultMessage` value.
async fn mount_check_login_status_with_message(server: &MockServer, message: &str) {
    let body = format!(r#"{{"ResultMessage":"{message}"}}"#);
    Mock::given(method("POST"))
        .and(path("/QRLogin/CheckLoginStatus"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

/// Mount `POST /QRLogin/CheckLoginStatus` returning a raw body
/// (any string — JSON or not). Used by the missing-field / non-JSON
/// tests where the canned `{ResultMessage:"…"}` shape doesn't fit.
async fn mount_check_login_status_with_raw(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/QRLogin/CheckLoginStatus"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.to_owned())
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Happy-path dispatch table — one test per known ResultMessage
// -----------------------------------------------------------------------------

#[tokio::test]
async fn failed_result_message_returns_failed_outcome() {
    let server = MockServer::start().await;
    mount_check_login_status_with_message(&server, "Failed").await;
    let client = client_for(&server, LoginRegion::TW);

    let outcome = poll_qr_login_status(&client, &fake_init())
        .await
        .expect("Failed should map to QrPollOutcome::Failed (keep polling)");
    assert_eq!(outcome, QrPollOutcome::Failed);
}

#[tokio::test]
async fn wait_login_result_message_returns_wait_login_outcome() {
    let server = MockServer::start().await;
    mount_check_login_status_with_message(&server, "Wait Login").await;
    let client = client_for(&server, LoginRegion::TW);

    let outcome = poll_qr_login_status(&client, &fake_init())
        .await
        .expect("Wait Login should map to QrPollOutcome::WaitLogin (keep polling)");
    assert_eq!(outcome, QrPollOutcome::WaitLogin);
}

#[tokio::test]
async fn token_expired_result_message_returns_token_expired() {
    let server = MockServer::start().await;
    mount_check_login_status_with_message(&server, "Token Expired").await;
    let client = client_for(&server, LoginRegion::TW);

    let outcome = poll_qr_login_status(&client, &fake_init())
        .await
        .expect("Token Expired should map to QrPollOutcome::TokenExpired");
    assert_eq!(outcome, QrPollOutcome::TokenExpired);
}

#[tokio::test]
async fn success_result_message_returns_approved_outcome() {
    // Server commonly includes a `ResultData` payload alongside
    // Success — we ignore it (WPF L647-648 also ignores it; the
    // downstream `qr_finalize` step uses the cached QrLoginInit).
    let server = MockServer::start().await;
    mount_check_login_status_with_raw(
        &server,
        r#"{"ResultMessage":"Success","ResultData":{"SessionKey":"abc","Status":0}}"#,
    )
    .await;
    let client = client_for(&server, LoginRegion::TW);

    let outcome = poll_qr_login_status(&client, &fake_init())
        .await
        .expect("Success should map to QrPollOutcome::Approved");
    assert_eq!(outcome, QrPollOutcome::Approved);
}

// -----------------------------------------------------------------------------
// Catch-all dispatch — unknown / missing ResultMessage
// -----------------------------------------------------------------------------

#[tokio::test]
async fn unknown_result_message_returns_server_message_with_body() {
    // WPF L649-652: unknown ResultMessage falls into the `else`
    // branch, which sets `errmsg = response` (raw body). We
    // surface the raw body verbatim via `LoginError::ServerMessage`
    // so the UI can show the unexpected backend chatter.
    let raw = r#"{"ResultMessage":"Backend exploded","ErrorCode":42}"#;
    let server = MockServer::start().await;
    mount_check_login_status_with_raw(&server, raw).await;
    let client = client_for(&server, LoginRegion::TW);

    let err = poll_qr_login_status(&client, &fake_init())
        .await
        .expect_err("unknown ResultMessage must surface as ServerMessage");
    match err {
        LoginError::ServerMessage(body) => assert_eq!(body, raw),
        other => panic!("expected LoginError::ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_result_message_returns_server_message_with_body() {
    // WPF L640: `(string)jsonData["ResultMessage"]` casts a missing
    // field to null in C#, which then matches none of the literal
    // branches and falls into the same `else` arm as an unknown
    // value. We mirror that fall-through.
    let raw = r#"{"OtherField":42}"#;
    let server = MockServer::start().await;
    mount_check_login_status_with_raw(&server, raw).await;
    let client = client_for(&server, LoginRegion::TW);

    let err = poll_qr_login_status(&client, &fake_init())
        .await
        .expect_err("missing ResultMessage must surface as ServerMessage");
    match err {
        LoginError::ServerMessage(body) => assert_eq!(body, raw),
        other => panic!("expected LoginError::ServerMessage, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Parse failure
// -----------------------------------------------------------------------------

#[tokio::test]
async fn non_json_body_returns_qr_json_parse_failed() {
    // WPF L634-638: `JObject.Parse` throws → `errmsg =
    // "LoginJsonParseFailed"`. We collapse the underlying
    // `serde_json::Error` into `LoginError::QrJsonParseFailed` to
    // preserve the WPF errmsg mapping for callers that pattern-match
    // on it.
    let server = MockServer::start().await;
    mount_check_login_status_with_raw(&server, "<html>not json at all</html>").await;
    let client = client_for(&server, LoginRegion::TW);

    let err = poll_qr_login_status(&client, &fake_init())
        .await
        .expect_err("non-JSON body must surface as QrJsonParseFailed");
    assert!(
        matches!(err, LoginError::QrJsonParseFailed),
        "expected LoginError::QrJsonParseFailed, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Region guard — short-circuits BEFORE any HTTP traffic
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_region_returns_qr_unsupported_without_http_traffic() {
    // No mocks mounted. If the guard fails to fire, the request
    // would 404 against an empty wiremock and surface as
    // `LoginError::Unknown` instead of `QrUnsupportedRegion`, so
    // the assertion below implicitly proves the guard short-
    // circuits before the network. The explicit `received_requests`
    // check at the end belt-and-braces it.
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);

    let err = poll_qr_login_status(&client, &fake_init())
        .await
        .expect_err("HK region must refuse QR poll");
    assert!(matches!(err, LoginError::QrUnsupportedRegion));

    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "HK guard must short-circuit before sending any HTTP traffic"
    );
}

// -----------------------------------------------------------------------------
// Wire shape — headers + body. We assert against `received_requests`
// rather than chaining wiremock matchers so a mismatch reports which
// header diverged instead of a silent 404.
// -----------------------------------------------------------------------------

#[tokio::test]
async fn request_carries_expected_headers_and_empty_form_body() {
    let server = MockServer::start().await;
    mount_check_login_status_with_message(&server, "Failed").await;
    let client = client_for(&server, LoginRegion::TW);

    poll_qr_login_status(&client, &fake_init())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/QRLogin/CheckLoginStatus")
        .expect("CheckLoginStatus request was sent");

    fn header_value<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
        req.headers.get(name).and_then(|v| v.to_str().ok())
    }

    // WPF L615-621 — Accept + Referer (via SetBaseHeaders) + Origin
    // + RequestVerificationToken (via Headers.Set).
    assert_eq!(
        header_value(req, "Accept"),
        Some("application/json, text/plain, */*"),
    );
    let expected_origin = Url::parse(&format!("{}/", server.uri()))
        .unwrap()
        .origin()
        .ascii_serialization();
    assert_eq!(header_value(req, "Origin"), Some(expected_origin.as_str()));
    assert_eq!(
        header_value(req, "RequestVerificationToken"),
        Some(VERIFICATION_TOKEN),
    );
    let expected_referer = format!("{}/Login/Index?pSKey={}", server.uri(), SESSION_KEY);
    assert_eq!(
        header_value(req, "Referer"),
        Some(expected_referer.as_str()),
    );

    // WPF `WebClient.UploadString(url, NameValueCollection)` sets
    // Content-Type to `application/x-www-form-urlencoded` even with
    // an empty payload. reqwest does NOT do this for `.body("")`,
    // so qr_poll sets it explicitly — verify here so a future
    // refactor that drops the explicit header gets caught.
    assert_eq!(
        header_value(req, "Content-Type"),
        Some("application/x-www-form-urlencoded"),
    );

    // WPF's `SetBaseHeaders` clears all headers first (L917) and
    // then doesn't add `X-Requested-With`. Verify we mirror that —
    // adding the header here would observable to the server and
    // diverge from the WPF wire shape. (Mirrors the inverse
    // assertion in qr_init.)
    assert!(
        req.headers.get("X-Requested-With").is_none(),
        "qr_poll must NOT send X-Requested-With (WPF SetBaseHeaders clears it)"
    );

    // Empty body — payload was an empty NameValueCollection.
    assert!(
        req.body.is_empty(),
        "POST body must be empty (WPF empty NameValueCollection serializes to ''), got {:?}",
        String::from_utf8_lossy(&req.body),
    );
}

#[tokio::test]
async fn request_omits_verification_header_when_init_had_no_token() {
    let server = MockServer::start().await;
    mount_check_login_status_with_message(&server, "Failed").await;
    let client = client_for(&server, LoginRegion::TW);

    poll_qr_login_status(&client, &fake_init_without_verification_token())
        .await
        .expect("happy roundtrip so we can inspect the request");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/QRLogin/CheckLoginStatus")
        .expect("CheckLoginStatus request was sent");

    assert!(
        req.headers.get("RequestVerificationToken").is_none(),
        "QR poll should mirror WPF null-token behavior by omitting the header"
    );
}
