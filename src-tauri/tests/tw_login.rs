//! End-to-end integration tests for the TW Regular login orchestrator.
//!
//! Each test spins up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it, and drives
//! [`login_tw_regular`](beanfun_lib::services::beanfun::login::login_tw_regular)
//! against a set of canned HTTP responses that reproduce one branch of
//! the real server's behaviour.
//!
//! Pure decode / classification unit tests live next to the source
//! modules; this file covers the **orchestration** — cookies, headers,
//! step ordering, error-variant mapping.

use beanfun_lib::services::beanfun::{
    login::login_tw_regular, BeanfunClient, ClientConfig, Credentials, Endpoints, LoginError,
    LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_partial_json, header_regex, method, path};
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
async fn recaptcha_required_diverts_to_webview_login() {
    // Token-replay (#313/#315/#318): reCAPTCHA is detected empty-first
    // from the CheckAccountType response (IsRecaptcha=true), not from a
    // separate InitLogin probe. It only counts when the check FAILED
    // (ResultCode != 1), so the demand here carries ResultCode 0.
    // The single-shot `login_tw_regular` has no interactive surface, so it
    // surfaces `RecaptchaRequired`. AccountLogin is intentionally left
    // unmounted: escalating at the check step must NOT touch it.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    let body = serde_json::json!({
        "ResultCode": "0",
        "ResultData": { "IsRecaptcha": true }
    });
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("reCAPTCHA-required must divert to the WebView flow");

    match err {
        LoginError::RecaptchaRequired { skey } => assert_eq!(skey, SKEY),
        other => panic!("expected RecaptchaRequired, got {other:?}"),
    }
}

#[tokio::test]
async fn recaptcha_false_continues_headless_flow() {
    // The complementary guard: a normal CheckAccountType response (no
    // IsRecaptcha / 機器人) must NOT divert — the headless flow runs
    // end-to-end and yields a session.
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
        .expect("no reCAPTCHA must continue the headless flow");
    assert_eq!(session.web_token, WEB_TOKEN);
}

#[tokio::test]
async fn is_recaptcha_flag_on_check_success_does_not_divert() {
    // The server flags IsRecaptcha as a session-level advisory even on a
    // ResultCode 1 CheckAccountType success. That must NOT pop the useless
    // pre-password widget — the headless flow proceeds to AccountLogin and
    // completes. (This is the exact "always asks for a useless reCAPTCHA
    // first" symptom, now guarded end-to-end.)
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ResultCode": "1",
            "ResultData": { "IsRecaptcha": true, "Captcha": "" }
        })))
        .mount(&server)
        .await;
    mount_account_login_success(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("IsRecaptcha on a check success must not divert");
    assert_eq!(session.web_token, WEB_TOKEN);
}

#[tokio::test]
async fn recaptcha_required_at_account_login_step_diverts() {
    // reCAPTCHA can also gate the *second* POST (AccountLogin) even when
    // CheckAccountType passed clean — empty-first must escalate there too.
    // As with the check step, the demand is a FAILURE response (ResultCode 0
    // + IsRecaptcha), not a ResultCode 1 success.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "").await;
    mount_account_login_with_body(
        &server,
        serde_json::json!({ "IsRecaptcha": true, "ResultCode": "0" }),
    )
    .await;

    let client = client_for(&server);
    let err = login_tw_regular(&client, &creds())
        .await
        .expect_err("AccountLogin reCAPTCHA must divert");
    assert!(matches!(err, LoginError::RecaptchaRequired { .. }));
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
async fn check_account_type_non_json_response_falls_through_empty_captcha() {
    // WPF L70-78: when CheckAccountType returns anything that does not
    // start with `{`, the captcha token defaults to empty and the flow
    // continues. We model that here by returning an HTML error page and
    // asserting the full flow still succeeds.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<html><body>transient error</body></html>"),
        )
        .mount(&server)
        .await;
    mount_account_login_success(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("non-JSON CheckAccountType must be tolerated");
    assert_eq!(session.web_token, WEB_TOKEN);
}

#[tokio::test]
async fn account_login_payload_propagates_captcha_from_check_account_type() {
    // Guards the Chunk 3.2 wiring: the captcha value returned by
    // CheckAccountType MUST be forwarded verbatim into AccountLogin's
    // JSON body. We enforce this by making AccountLogin's mock only
    // match when the request body contains
    // `"Captcha": "CAPTCHA_FROM_STEP_2"`. If the propagation breaks in
    // a future refactor, the mock falls through to wiremock's 404 and
    // the test fails with a `LoginError::Unknown("AccountLogin returned
    // HTTP 404 …")` which surfaces the regression loudly.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_index_with_token(&server, FORM_TOKEN).await;
    mount_check_account_type(&server, "CAPTCHA_FROM_STEP_2").await;

    Mock::given(method("POST"))
        .and(path("/Login/AccountLogin"))
        .and(body_partial_json(serde_json::json!({
            "Account": ACCOUNT,
            "Pasw": PASSWORD,
            "Captcha": "CAPTCHA_FROM_STEP_2",
            "IsMobile": false,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ResultCode": "1",
            "Result": "0",
            "ResultMessage": ""
        })))
        .mount(&server)
        .await;

    mount_send_login_happy(&server).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("captcha must be forwarded into AccountLogin body");
    assert_eq!(session.web_token, WEB_TOKEN);
}

#[tokio::test]
async fn session_cookies_persist_across_login_steps() {
    // Verifies that the shared cookie jar on `BeanfunClient` captures a
    // `Set-Cookie` from Login/Index and forwards it on subsequent same-
    // host requests. If the two reqwest clients (redirect / no-redirect)
    // ever stop sharing the jar, the CheckAccountType mock below will
    // 404 and the test will surface the regression.
    let session_cookie = "ASP.NET_SessionId=COOKIE_FIXTURE";
    let server = MockServer::start().await;
    mount_session_key(&server).await;

    // Override the plain Login/Index mock with one that plants a cookie
    // in its Set-Cookie header.
    let index_html = format!(
        r#"<html><body>
            <input name="__RequestVerificationToken" type="hidden" value="{FORM_TOKEN}" />
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/Login/Index"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(index_html)
                .append_header("Set-Cookie", format!("{session_cookie}; Path=/").as_str()),
        )
        .mount(&server)
        .await;

    // Gate CheckAccountType on the cookie being present on the inbound
    // request. `header_regex` escapes the dot in `ASP.NET_SessionId`.
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .and(header_regex("cookie", r"ASP\.NET_SessionId=COOKIE_FIXTURE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ResultCode": "1",
            "ResultData": { "Captcha": "" }
        })))
        .mount(&server)
        .await;

    mount_account_login_success(&server).await;
    mount_send_login_happy(&server).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("cookies from Login/Index must be forwarded to CheckAccountType");
    assert_eq!(session.web_token, WEB_TOKEN);
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
