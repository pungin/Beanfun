//! End-to-end integration tests for the TW Regular login orchestrator.
//!
//! Each test spins up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives
//! [`login_tw_regular`](beanfun_next_lib::services::beanfun::login::login_tw_regular)
//! against a set of canned HTTP responses that reproduce one branch of
//! the real server's behaviour.
//!
//! Pure decode / classification unit tests live next to the source
//! modules; this file covers the **orchestration** — cookies, headers,
//! step ordering, error-variant mapping.

use beanfun_next_lib::services::beanfun::{
    login::login_tw_regular, BeanfunClient, ClientConfig, Credentials, Endpoints, LoginError,
    LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ACCOUNT: &str = "alice";
const PASSWORD: &str = "hunter2";
const SKEY: &str = "TW_TEST_SKEY";
const FORM_TOKEN: &str = "VTOKEN_abc";
const WEB_TOKEN: &str = "BFWT_happy_path";

// -----------------------------------------------------------------------------
// Mock setup helpers
// -----------------------------------------------------------------------------
//
// One function per protocol step. Each returns `()` and mounts the
// route on `server`; tests string them together in whatever combination
// the scenario needs. Keeping the helpers fine-grained (rather than a
// single "mount everything happy" god-function) means a test can swap
// one step for an error variant without rebuilding the whole chain.

/// Session-key step (two mocks: the 302 on `default.aspx` and the 200
/// landing at the redirect target).
async fn mount_session_key(server: &MockServer) {
    let location = format!("{}/login/id-pass.aspx?pSKey={}", server.uri(), SKEY);
    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .respond_with(ResponseTemplate::new(302).append_header("Location", location.as_str()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/login/id-pass.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string("landing"))
        .mount(server)
        .await;
}

/// Login/Index — responds with an HTML page carrying a
/// `__RequestVerificationToken` hidden input.
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

/// Login/CheckAccountType — responds with the given captcha token string
/// (pass `""` to simulate "no captcha required").
async fn mount_check_account_type(server: &MockServer, captcha: &str) {
    let body = serde_json::json!({
        "ResultCode": "1",
        "ResultData": { "Captcha": captcha }
    });
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Login/AccountLogin — responds with an arbitrary JSON body. Tests
/// pass different bodies to reach each response-classification branch.
async fn mount_account_login_with_body(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/Login/AccountLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Login/AccountLogin — the happy-path `{ResultCode:"1", Result:"0"}` body.
async fn mount_account_login_success(server: &MockServer) {
    mount_account_login_with_body(
        server,
        serde_json::json!({
            "ResultCode": "1",
            "Result": "0",
            "ResultMessage": ""
        }),
    )
    .await;
}

/// Login/SendLogin — responds with the given HTML body.
async fn mount_send_login_with_html(server: &MockServer, html: &str) {
    Mock::given(method("GET"))
        .and(path("/Login/SendLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(server)
        .await;
}

/// Login/SendLogin — a realistic happy-path form with three hidden inputs.
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

/// return.aspx — 302 redirect carrying a `bfWebToken=…` Set-Cookie.
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

/// return.aspx — 302 redirect **without** the `bfWebToken` cookie.
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

/// Build a client whose three endpoint bases all point at `server`.
fn client_for(server: &MockServer) -> BeanfunClient {
    let base = Url::parse(&format!("{}/", server.uri())).expect("mock URL parses");
    let endpoints = Endpoints {
        login_base: base.clone(),
        portal_base: base.clone(),
        newlogin_base: base,
    };
    let mut cfg = ClientConfig::for_region(LoginRegion::TW);
    cfg.endpoints = endpoints;
    BeanfunClient::new(cfg).expect("client builds")
}

fn creds() -> Credentials {
    Credentials::new(ACCOUNT, PASSWORD)
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_regular_happy_path_returns_session() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_success(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("happy path must succeed");

    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
    assert_eq!(session.service_code, "610074");
    assert_eq!(session.service_region, "T9");
}

#[tokio::test]
async fn wrong_password_surfaces_server_message() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_with_body(
        &server,
        serde_json::json!({
            "ResultCode": "-1",
            "Result": "",
            "ResultMessage": "帳號或密碼錯誤"
        }),
    )
    .await;
    // SendLogin / return.aspx unused; no need to mount.

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("wrong password must error");

    match err {
        LoginError::ServerMessage(msg) => assert_eq!(msg, "帳號或密碼錯誤"),
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn advance_check_result_1_1_yields_none_url() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_with_body(
        &server,
        serde_json::json!({
            "ResultCode": "1",
            "Result": "1",
            "ResultMessage": "ignored"
        }),
    )
    .await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("advance check must error");

    match err {
        LoginError::AdvanceCheckRequired { url: None } => {}
        other => panic!("expected AdvanceCheckRequired {{ url: None }}, got {other:?}"),
    }
}

#[tokio::test]
async fn advance_check_result_code_2_preserves_http_url() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_with_body(
        &server,
        serde_json::json!({
            "ResultCode": "2",
            "Result": "",
            "ResultMessage": "https://verify.example/check?t=123"
        }),
    )
    .await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("advance check must error");

    match err {
        LoginError::AdvanceCheckRequired { url: Some(u) } => {
            assert_eq!(u, "https://verify.example/check?t=123");
        }
        other => panic!("expected AdvanceCheckRequired {{ url: Some }}, got {other:?}"),
    }
}

#[tokio::test]
async fn send_login_empty_form_yields_send_login_no_form_data() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_success(&server).await;
    // SendLogin returns HTML with no `<input>` tags at all.
    mount_send_login_with_html(&server, "<html><body>oops</body></html>").await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("empty SendLogin must error");

    assert!(
        matches!(err, LoginError::SendLoginNoFormData),
        "expected SendLoginNoFormData, got {err:?}"
    );
}

#[tokio::test]
async fn return_aspx_without_cookie_yields_missing_web_token() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_success(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_without_token(&server).await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("missing bfWebToken must error");

    assert!(
        matches!(err, LoginError::MissingWebToken),
        "expected MissingWebToken, got {err:?}"
    );
}

#[tokio::test]
async fn index_missing_token_yields_missing_verification_token() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    // Index HTML does NOT carry a `__RequestVerificationToken`.
    Mock::given(method("GET"))
        .and(path("/Login/Index"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html><body>nope</body></html>"))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("missing verification token must error");

    assert!(
        matches!(err, LoginError::MissingVerificationToken),
        "expected MissingVerificationToken, got {err:?}"
    );
}
