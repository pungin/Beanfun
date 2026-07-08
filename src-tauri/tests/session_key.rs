//! End-to-end tests for `services::beanfun::login::get_session_key`.
//!
//! These tests spin up a [`wiremock::MockServer`], point a
//! [`BeanfunClient`] at it via [`Endpoints::custom`], and exercise the TW
//! and HK paths against realistic HTTP interactions. Pure regex unit
//! tests live inside the source module itself.

use beanfun_lib::services::beanfun::{
    login::get_session_key, BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a [`BeanfunClient`] whose three endpoint bases all point at
/// `server`. Every integration test uses this so the client config stays
/// consistent and the setup noise stays out of the test bodies.
fn client_for_mock(server: &MockServer, region: LoginRegion) -> BeanfunClient {
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

#[tokio::test]
async fn tw_session_key_extracted_from_redirect_url() {
    let server = MockServer::start().await;
    let location = format!(
        "{}/login/id-pass.aspx?service=999999_T0&pSKey=TW_SKEY_TEST_42",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .and(query_param("service", "999999_T0"))
        .respond_with(ResponseTemplate::new(302).append_header("Location", location.as_str()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/login/id-pass.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string("landing page"))
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::TW);
    let key = get_session_key(&client)
        .await
        .expect("session key extracted");

    assert_eq!(key, "TW_SKEY_TEST_42");
}

#[tokio::test]
async fn tw_missing_key_in_final_url_returns_missing_session_key() {
    let server = MockServer::start().await;
    // No redirect, no pSKey anywhere — we simulate a broken portal page.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("no key here"))
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::TW);
    let result = get_session_key(&client).await;

    assert!(
        matches!(result, Err(LoginError::MissingSessionKey)),
        "expected MissingSessionKey, got: {result:?}"
    );
}

#[tokio::test]
async fn hk_session_key_extracted_from_otp1_span() {
    let server = MockServer::start().await;
    let body = r#"<html><body><span id="ctl00_ContentPlaceHolder1_lblOtp1">HK_OTP1_FIXTURE</span></body></html>"#;

    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .and(query_param("service", "999999_T0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::HK);
    let key = get_session_key(&client)
        .await
        .expect("HK session key extracted");

    assert_eq!(key, "HK_OTP1_FIXTURE");
}

#[tokio::test]
async fn hk_missing_span_returns_missing_session_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>no otp span here</html>"))
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::HK);
    let result = get_session_key(&client).await;

    assert!(
        matches!(result, Err(LoginError::MissingSessionKey)),
        "expected MissingSessionKey, got: {result:?}"
    );
}

#[tokio::test]
async fn hk_empty_body_returns_empty_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::HK);
    let result = get_session_key(&client).await;

    assert!(
        matches!(result, Err(LoginError::EmptyResponse)),
        "expected EmptyResponse, got: {result:?}"
    );
}

#[tokio::test]
async fn body_cap_triggers_body_too_large() {
    let server = MockServer::start().await;
    // 2 KB payload; client cap will be 1 KB below.
    let big_body = "x".repeat(2048);
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(big_body))
        .mount(&server)
        .await;

    let base = Url::parse(&format!("{}/", server.uri())).unwrap();
    let endpoints = Endpoints {
        login_base: base.clone(),
        portal_base: base.clone(),
        newlogin_base: base,
    };
    let mut cfg = ClientConfig::for_region(LoginRegion::HK);
    cfg.endpoints = endpoints;
    cfg.max_body_size = 1024;

    let client = BeanfunClient::new(cfg).expect("client builds");
    let result = get_session_key(&client).await;

    assert!(
        matches!(result, Err(LoginError::BodyTooLarge { limit: 1024, .. })),
        "expected BodyTooLarge with limit=1024, got: {result:?}"
    );
}

#[tokio::test]
async fn user_agent_matches_wpf_reference() {
    let server = MockServer::start().await;
    // Mount a mock that matches any GET to the portal default URL.
    // We verify the UA by checking the mock was actually hit (if the
    // UA were wrong the server would reject it in production; here we
    // just confirm the request succeeds with the body we expect).
    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .and(query_param("service", "999999_T0"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"<span id="ctl00_ContentPlaceHolder1_lblOtp1">UA_OK</span>"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for_mock(&server, LoginRegion::HK);

    // Verify the client is configured with the full Chrome UA.
    assert_eq!(
        client.config().user_agent,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
        "DEFAULT_USER_AGENT must be the full Chrome string for HK portal compatibility"
    );

    let key = get_session_key(&client)
        .await
        .expect("request with matching UA succeeds");

    assert_eq!(key, "UA_OK");
}
