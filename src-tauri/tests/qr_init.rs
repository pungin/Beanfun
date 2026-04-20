//! End-to-end integration tests for the QR-code init step
//! (`login/qr_init.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives [`init_qr_login`] against one
//! canned response combination that exercises one branch of the WPF
//! `GetQRCodeValue` / `getQRCodeStrEncryptData` flow
//! (`BeanfunClient.Login.cs` L409-476).
//!
//! | WPF branch                                  | Covered by                                              |
//! |---------------------------------------------|---------------------------------------------------------|
//! | happy path → `QRCodeClass` populated        | `happy_path_returns_qr_login_init`                      |
//! | bitmap shape `"data:image/png;base64,…"`    | `bitmap_base64_carries_full_data_url_prefix`            |
//! | deeplink wrapper unwraps inner `?url=`      | `deeplink_unwraps_play_games_gamania_wrapper`           |
//! | deeplink passes through when no wrapper     | `deeplink_passes_through_plain_url`                     |
//! | missing `DeepLink` field                    | `deeplink_is_none_when_server_omits_field`              |
//! | empty `DeepLink` value                      | `deeplink_is_none_when_server_sends_empty_string`       |
//! | HK region guard                             | `hk_region_returns_qr_unsupported_without_http_traffic` |
//! | step 1 missing antiforgery token            | `missing_verification_token_propagates_from_index`      |
//! | `Result != 0`                               | `init_login_result_non_zero_returns_qr_init_error`      |
//! | `Result` field missing                      | `init_login_missing_result_field_returns_qr_init_error` |
//! | `ResultData` missing                        | `init_login_missing_result_data_returns_qr_init_error`  |
//! | `QRImage` field missing                     | `init_login_missing_qr_image_returns_qr_init_error`     |
//! | `QRImage` empty string                      | `init_login_empty_qr_image_returns_qr_init_error`       |
//! | non-JSON body                               | `init_login_invalid_json_returns_json_error`            |
//! | request headers (Accept / Origin / etc.)    | `init_login_request_sends_expected_headers`             |
//!
//! Pure helpers ([`normalize_beanfun_app_deeplink`] + serde envelope)
//! are unit-tested next to the source module; this file covers the
//! HTTP orchestration, header set, and the layered `Result` / field
//! error mapping end-to-end.

use beanfun_lib::services::beanfun::{
    login::{init_qr_login, QrLoginInit},
    BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION_KEY: &str = "SKEY_QR";
const VERIFICATION_TOKEN: &str = "VTOKEN_qr_xyz";
const QR_IMAGE_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step
// -----------------------------------------------------------------------------

/// Login/Index — responds with an HTML page carrying a
/// `__RequestVerificationToken` hidden input (matches WPF L416-418
/// regex shape).
async fn mount_index_with_token(server: &MockServer, token: &str) {
    let body = format!(
        r#"<html><body>
            <input name="__RequestVerificationToken" type="hidden" value="{token}" />
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/Login/Index"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Login/Index — HTML page WITHOUT the antiforgery token, used to
/// drive the [`LoginError::MissingVerificationToken`] branch.
async fn mount_index_without_token(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/Login/Index"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<html><body>nothing here</body></html>"),
        )
        .mount(server)
        .await;
}

/// Login/InitLogin — GET responder with arbitrary JSON body. Tests
/// pass different bodies to reach each branch of the layered
/// `Result` / `ResultData` / `QRImage` checks.
async fn mount_init_login_get_json(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/Login/InitLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Login/InitLogin — GET responder with a raw (non-JSON) body, to
/// drive the [`LoginError::Json`] branch.
async fn mount_init_login_get_raw(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/Login/InitLogin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.to_owned())
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

/// Happy-path InitLogin body — the canonical
/// `{Result: 0, ResultData: { QRImage, DeepLink }}` shape.
fn happy_init_body(deeplink: &str) -> serde_json::Value {
    serde_json::json!({
        "Result": 0,
        "ResultData": {
            "QRImage": QR_IMAGE_BASE64,
            "DeepLink": deeplink,
        }
    })
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
// Happy path
// -----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_returns_qr_login_init() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        happy_init_body("https://target.example/auth?code=xyz"),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let QrLoginInit {
        skey,
        bitmap_base64,
        deeplink,
        verification_token,
    } = init_qr_login(&client, SESSION_KEY)
        .await
        .expect("happy path returns Ok");

    // skey roundtrips verbatim from `init_qr_login`'s argument so the
    // poll/finalize steps can rebuild the `Referer` URL without the
    // caller threading the value separately.
    assert_eq!(skey, SESSION_KEY);
    assert_eq!(verification_token, VERIFICATION_TOKEN);
    assert_eq!(
        bitmap_base64,
        format!("data:image/png;base64,{QR_IMAGE_BASE64}")
    );
    assert_eq!(
        deeplink.as_deref(),
        Some("https://target.example/auth?code=xyz")
    );
}

#[tokio::test]
async fn bitmap_base64_carries_full_data_url_prefix() {
    // Lock the WPF storage shape (`bitmapBase64 = "data:image/png;base64," + raw`)
    // — frontend consumers depend on dropping the field straight into
    // `<img src=…>`.
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(&server, happy_init_body("")).await;

    let client = client_for(&server, LoginRegion::TW);
    let init = init_qr_login(&client, SESSION_KEY).await.unwrap();

    assert!(init.bitmap_base64.starts_with("data:image/png;base64,"));
    assert!(init.bitmap_base64.ends_with(QR_IMAGE_BASE64));
}

// -----------------------------------------------------------------------------
// Deeplink normalization (full pipeline — not just the pure helper)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn deeplink_unwraps_play_games_gamania_wrapper() {
    // Real-world server occasionally wraps the deeplink in
    // `play.games.gamania.com/.../deeplink/?url=…` — `init_qr_login`
    // should deliver the unwrapped inner URL to the caller.
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        happy_init_body(
            "https://play.games.gamania.com/app/deeplink/?url=https://target.example/auth?token=abc",
        ),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let init = init_qr_login(&client, SESSION_KEY).await.unwrap();

    assert_eq!(
        init.deeplink.as_deref(),
        Some("https://target.example/auth?token=abc")
    );
}

#[tokio::test]
async fn deeplink_passes_through_plain_url() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(&server, happy_init_body("beanfunapp://login?token=plain")).await;

    let client = client_for(&server, LoginRegion::TW);
    let init = init_qr_login(&client, SESSION_KEY).await.unwrap();

    assert_eq!(
        init.deeplink.as_deref(),
        Some("beanfunapp://login?token=plain")
    );
}

#[tokio::test]
async fn deeplink_is_none_when_server_omits_field() {
    // ResultData carries QRImage but no DeepLink — WPF stores null,
    // we surface as `Option::None`.
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        serde_json::json!({
            "Result": 0,
            "ResultData": {
                "QRImage": QR_IMAGE_BASE64
            }
        }),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let init = init_qr_login(&client, SESSION_KEY).await.unwrap();

    assert!(init.deeplink.is_none());
}

#[tokio::test]
async fn deeplink_is_none_when_server_sends_empty_string() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(&server, happy_init_body("")).await;

    let client = client_for(&server, LoginRegion::TW);
    let init = init_qr_login(&client, SESSION_KEY).await.unwrap();

    assert!(init.deeplink.is_none());
}

// -----------------------------------------------------------------------------
// Region guard — short-circuits BEFORE any HTTP traffic
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_region_returns_qr_unsupported_without_http_traffic() {
    // No mocks mounted. If the guard fails to fire, the GET request
    // would 404 against an empty wiremock and surface as
    // `LoginError::Unknown(... HTTP 404)` instead of
    // `LoginError::QrUnsupportedRegion` — so the assertion below
    // implicitly proves the guard short-circuits before the network.
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);

    let err = init_qr_login(&client, SESSION_KEY)
        .await
        .expect_err("HK region should refuse QR init");
    assert!(matches!(err, LoginError::QrUnsupportedRegion));

    // Belt-and-braces: explicit "no requests reached the mock".
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "HK guard must short-circuit before sending any HTTP traffic"
    );
}

// -----------------------------------------------------------------------------
// Step 1 (Login/Index) error propagation
// -----------------------------------------------------------------------------

#[tokio::test]
async fn missing_verification_token_propagates_from_index() {
    let server = MockServer::start().await;
    mount_index_without_token(&server).await;
    // No InitLogin mock — should never be called.

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY)
        .await
        .expect_err("missing token should error before InitLogin");
    assert!(matches!(err, LoginError::MissingVerificationToken));
}

// -----------------------------------------------------------------------------
// Step 2 (Login/InitLogin) layered Result / ResultData / QRImage checks
// -----------------------------------------------------------------------------

#[tokio::test]
async fn init_login_result_non_zero_returns_qr_init_error() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        serde_json::json!({
            "Result": -1,
            "ResultData": null
        }),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(matches!(err, LoginError::QrInitResultError));
}

#[tokio::test]
async fn init_login_missing_result_field_returns_qr_init_error() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        serde_json::json!({
            "ResultData": { "QRImage": QR_IMAGE_BASE64 }
        }),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(matches!(err, LoginError::QrInitResultError));
}

#[tokio::test]
async fn init_login_missing_result_data_returns_qr_init_error() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(&server, serde_json::json!({ "Result": 0 })).await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(matches!(err, LoginError::QrInitResultError));
}

#[tokio::test]
async fn init_login_missing_qr_image_returns_qr_init_error() {
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        serde_json::json!({
            "Result": 0,
            "ResultData": { "DeepLink": "x" }
        }),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(matches!(err, LoginError::QrInitResultError));
}

#[tokio::test]
async fn init_login_empty_qr_image_returns_qr_init_error() {
    // WPF L436-441 collapses null QRImage and "" QRImage to the same
    // `LoginIntResultError` branch via `string.IsNullOrEmpty`.
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(
        &server,
        serde_json::json!({
            "Result": 0,
            "ResultData": { "QRImage": "", "DeepLink": "x" }
        }),
    )
    .await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(matches!(err, LoginError::QrInitResultError));
}

#[tokio::test]
async fn init_login_invalid_json_returns_json_error() {
    // WPF's `JObject.Parse` would throw — we surface a typed
    // `LoginError::Json(...)` instead. Strictly safer than crashing
    // the dispatcher (same rationale as P3 chunk 3.3.4).
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_raw(&server, "<html>not json</html>").await;

    let client = client_for(&server, LoginRegion::TW);
    let err = init_qr_login(&client, SESSION_KEY).await.unwrap_err();
    assert!(
        matches!(err, LoginError::Json(_)),
        "expected LoginError::Json, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Wire shape — headers
// -----------------------------------------------------------------------------

#[tokio::test]
async fn init_login_request_sends_expected_headers() {
    // Verify the four headers WPF's `getQRCodeStrEncryptData` sets
    // (L455-466): Accept, Referer (= Login/Index?pSKey=…),
    // X-Requested-With, Origin (= scheme://host of login_base).
    //
    // We assert on the recorded request rather than chaining
    // wiremock matchers so a mismatch reports which header diverged
    // (instead of a single 404 with no further detail).
    let server = MockServer::start().await;
    mount_index_with_token(&server, VERIFICATION_TOKEN).await;
    mount_init_login_get_json(&server, happy_init_body("")).await;

    let client = client_for(&server, LoginRegion::TW);
    init_qr_login(&client, SESSION_KEY)
        .await
        .expect("happy path returns Ok");

    let received = server.received_requests().await.expect("requests recorded");
    let init_req = received
        .iter()
        .find(|r| r.url.path() == "/Login/InitLogin")
        .expect("Login/InitLogin request was sent");

    fn header_value<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
        req.headers.get(name).and_then(|v| v.to_str().ok())
    }

    assert_eq!(
        header_value(init_req, "Accept"),
        Some("application/json, text/plain, */*"),
    );
    assert_eq!(
        header_value(init_req, "X-Requested-With"),
        Some("XMLHttpRequest"),
    );

    let expected_origin = Url::parse(&format!("{}/", server.uri()))
        .unwrap()
        .origin()
        .ascii_serialization();
    assert_eq!(
        header_value(init_req, "Origin"),
        Some(expected_origin.as_str())
    );

    let expected_referer = format!("{}/Login/Index?pSKey={}", server.uri(), SESSION_KEY);
    assert_eq!(
        header_value(init_req, "Referer"),
        Some(expected_referer.as_str()),
    );
}
