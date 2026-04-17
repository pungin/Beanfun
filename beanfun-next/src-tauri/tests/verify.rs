//! End-to-end integration tests for `services/beanfun/verify.rs`
//! (P4 chunk 4.3).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], routes the
//! `newlogin_base` (the only host verify uses) at the mock, and
//! exercises one of the three public functions
//! ([`get_verify_page_info`], [`get_verify_captcha`],
//! [`submit_verify`]) against canned responses that pin a specific
//! WPF behaviour.
//!
//! Pure helpers (`parse_verify_page`, `classify_verify_response`,
//! `build_verify_form`, `build_captcha_url`,
//! `build_default_advance_check_url`, `ensure_tw`) are covered by
//! unit tests next to the source module; this file locks the wire
//! shapes and the orchestration on top of them.
//!
//! | Scenario                                                      | Outcome                                                                  |
//! |---------------------------------------------------------------|--------------------------------------------------------------------------|
//! | TW happy path (3 calls in order)                              | success [`VerifyOutcome::Success`]                                       |
//! | HK region rejection × 3 fns                                   | [`LoginError::VerifyUnsupportedRegion`] before any HTTP traffic          |
//! | get_verify_page_info uses passed advance_check_url            | wiremock receives GET on the explicit URL                                |
//! | get_verify_page_info falls back to default URL when None      | wiremock receives GET on `LoginCheck/AdvanceCheck.aspx`                  |
//! | get_verify_page_info HTML alert short-circuits                | [`LoginError::ServerMessage`]                                            |
//! | get_verify_captcha returns bytes verbatim on ≥ 500-byte body  | bytes match what server sent                                             |
//! | get_verify_captcha rejects < 500-byte body                    | [`LoginError::VerifyCaptchaImageTooSmall`]                               |
//! | submit_verify POST body has 8 fields in WPF order             | wiremock body matcher confirms                                           |
//! | submit_verify alert success → Success                         | [`VerifyOutcome::Success`]                                               |
//! | submit_verify alert other → ServerMessage                     | [`VerifyOutcome::ServerMessage`] with raw text                           |
//! | submit_verify wrong captcha text → WrongCaptcha               | [`VerifyOutcome::WrongCaptcha`]                                          |
//! | submit_verify no alert + no captcha text → WrongAuthInfo      | [`VerifyOutcome::WrongAuthInfo`]                                         |

use beanfun_next_lib::services::beanfun::{
    get_verify_captcha, get_verify_page_info, submit_verify, BeanfunClient, ClientConfig,
    Endpoints, LoginError, LoginRegion, VerifyOutcome, VerifyPageInfo,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

// -----------------------------------------------------------------------------
// Fixture builders
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] whose `newlogin_base` (and the other
/// two bases, harmlessly) point at `server`. Region defaults to TW
/// because verify is TW-only by design; HK tests construct their
/// client separately to exercise the `VerifyUnsupportedRegion`
/// guard.
fn tw_client_for(server: &MockServer) -> BeanfunClient {
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

/// HK client backed by no real server — verifies that the
/// region guard short-circuits **before** any HTTP traffic.
fn hk_client_no_server() -> BeanfunClient {
    BeanfunClient::new(ClientConfig::for_region(LoginRegion::HK)).expect("client builds")
}

/// AdvanceCheck.aspx HTML with every required field present plus a
/// resolvable form action. Mirrors the production page shape closely
/// enough that all four extraction regexes match.
fn full_verify_page_html() -> String {
    r#"
<html><body>
<form method="post" action="AdvanceCheck.aspx?ReturnUrl=foo&amp;sid=BAR" id="form1">
<input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="VS_ITG" />
<input type="hidden" name="__VIEWSTATEGENERATOR" id="__VIEWSTATEGENERATOR" value="GEN_ITG" />
<input type="hidden" name="__EVENTVALIDATION" id="__EVENTVALIDATION" value="EV_ITG" />
<input type="hidden" name="LBD_VCID_c_logincheck_advancecheck_samplecaptcha" id="LBD_VCID_c_logincheck_advancecheck_samplecaptcha" value="VCID_ITG" />
<span id="lblAuthType">Email</span>
</form>
</body></html>
"#.to_string()
}

/// 600-byte fake PNG payload — large enough to clear the
/// `< 500` rejection threshold without actually being a real image
/// (we don't decode it).
fn fake_captcha_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(600);
    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.resize(600, 0xAB);
    bytes
}

// -----------------------------------------------------------------------------
// Mock setup helpers
// -----------------------------------------------------------------------------

async fn mount_advance_check_get(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/LoginCheck/AdvanceCheck.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

async fn mount_captcha(server: &MockServer, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path("/LoginCheck/BotDetectCaptcha.ashx"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

async fn mount_advance_check_post(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/LoginCheck/AdvanceCheck.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Group A — Happy path (full 3-call flow)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_happy_path_get_page_then_captcha_then_submit_success() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);

    mount_advance_check_get(&server, &full_verify_page_html()).await;
    mount_captcha(&server, fake_captcha_bytes()).await;
    mount_advance_check_post(&server, "<script>alert('資料已驗證成功');</script>").await;

    let info = get_verify_page_info(&client, None)
        .await
        .expect("page info fetched");
    assert_eq!(info.viewstate, "VS_ITG");
    assert_eq!(info.lbl_auth_type, "Email");

    let bytes = get_verify_captcha(&client, &info.samplecaptcha)
        .await
        .expect("captcha fetched");
    assert!(bytes.len() >= 500);

    let outcome = submit_verify(&client, &info, "AUTH_CODE_123", "CAPTCHA_XYZ")
        .await
        .expect("submit succeeds");
    assert_eq!(outcome, VerifyOutcome::Success);
}

// -----------------------------------------------------------------------------
// Group B — HK region guard (no HTTP traffic at all)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_get_verify_page_info_returns_unsupported_region() {
    let client = hk_client_no_server();
    let err = get_verify_page_info(&client, None).await.unwrap_err();
    assert!(matches!(err, LoginError::VerifyUnsupportedRegion));
}

#[tokio::test]
async fn hk_get_verify_captcha_returns_unsupported_region() {
    let client = hk_client_no_server();
    let err = get_verify_captcha(&client, "VCID_x").await.unwrap_err();
    assert!(matches!(err, LoginError::VerifyUnsupportedRegion));
}

#[tokio::test]
async fn hk_submit_verify_returns_unsupported_region() {
    let client = hk_client_no_server();
    let info = VerifyPageInfo {
        viewstate: "x".into(),
        viewstate_generator: None,
        event_validation: "x".into(),
        samplecaptcha: "x".into(),
        lbl_auth_type: "x".into(),
        form_action: "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx".into(),
    };
    let err = submit_verify(&client, &info, "v", "c").await.unwrap_err();
    assert!(matches!(err, LoginError::VerifyUnsupportedRegion));
}

// -----------------------------------------------------------------------------
// Group C — get_verify_page_info URL routing
// -----------------------------------------------------------------------------

#[tokio::test]
async fn get_verify_page_info_uses_passed_url_when_some() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);

    // Mount on a non-default path; if the function ignores the
    // passed URL, the default-path mock would be unmounted and the
    // request would 404.
    Mock::given(method("GET"))
        .and(path("/LoginCheck/AdvanceCheck.aspx"))
        .and(query_param("ReturnUrl", "explicit"))
        .respond_with(ResponseTemplate::new(200).set_body_string(full_verify_page_html()))
        .mount(&server)
        .await;

    let explicit_url = format!(
        "{}/LoginCheck/AdvanceCheck.aspx?ReturnUrl=explicit",
        server.uri()
    );
    let info = get_verify_page_info(&client, Some(&explicit_url))
        .await
        .expect("explicit URL request lands");
    assert_eq!(info.viewstate, "VS_ITG");
}

#[tokio::test]
async fn get_verify_page_info_falls_back_to_default_url_when_none() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    mount_advance_check_get(&server, &full_verify_page_html()).await;

    let info = get_verify_page_info(&client, None)
        .await
        .expect("default URL request lands");
    assert_eq!(info.viewstate, "VS_ITG");
}

#[tokio::test]
async fn get_verify_page_info_alert_short_circuits_with_server_message() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let html = full_verify_page_html()
        .replace("</form>", "</form><script>alert('帳號暫時鎖定');</script>");
    mount_advance_check_get(&server, &html).await;

    match get_verify_page_info(&client, None).await.unwrap_err() {
        LoginError::ServerMessage(msg) => assert_eq!(msg, "帳號暫時鎖定"),
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Group D — get_verify_captcha
// -----------------------------------------------------------------------------

#[tokio::test]
async fn get_verify_captcha_returns_bytes_verbatim_on_large_enough_body() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let bytes = fake_captcha_bytes();
    mount_captcha(&server, bytes.clone()).await;

    let got = get_verify_captcha(&client, "VCID_xyz")
        .await
        .expect("captcha bytes fetched");
    assert_eq!(got, bytes);
}

#[tokio::test]
async fn get_verify_captcha_too_small_returns_typed_error() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    // 100 bytes — well below the 500-byte threshold.
    mount_captcha(&server, vec![0xAB; 100]).await;

    match get_verify_captcha(&client, "VCID_xyz").await.unwrap_err() {
        LoginError::VerifyCaptchaImageTooSmall { actual } => assert_eq!(actual, 100),
        other => panic!("expected VerifyCaptchaImageTooSmall, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Group E — submit_verify wire shape & outcome classification
// -----------------------------------------------------------------------------

fn page_info_for_submit() -> VerifyPageInfo {
    VerifyPageInfo {
        viewstate: "VS_SUB".into(),
        viewstate_generator: Some("GEN_SUB".into()),
        event_validation: "EV_SUB".into(),
        samplecaptcha: "VCID_SUB".into(),
        lbl_auth_type: "Email".into(),
        // Will be overridden per-test to point at the wiremock URL.
        form_action: String::new(),
    }
}

/// Build a `VerifyPageInfo` whose `form_action` resolves to the
/// mock server. We can't simply call `tw_client_for(server)` →
/// build a URL because [`VerifyPageInfo`] is constructed by the
/// caller in production; tests fabricate one directly.
fn page_info_pointing_at(server: &MockServer) -> VerifyPageInfo {
    VerifyPageInfo {
        form_action: format!("{}/LoginCheck/AdvanceCheck.aspx", server.uri()),
        ..page_info_for_submit()
    }
}

#[tokio::test]
async fn submit_verify_post_body_has_eight_fields_in_wpf_order() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);

    // Wiremock's `body_string_contains` is sufficient to assert
    // each field is present; for ordering we capture the body via
    // `respond_with(|req| ...)` and assert the exact form.
    Mock::given(method("POST"))
        .and(path("/LoginCheck/AdvanceCheck.aspx"))
        .and(body_string_contains("__VIEWSTATE=VS_SUB"))
        .and(body_string_contains("__VIEWSTATEGENERATOR=GEN_SUB"))
        .and(body_string_contains("__EVENTVALIDATION=EV_SUB"))
        .and(body_string_contains("txtVerify=VCODE"))
        .and(body_string_contains("CodeTextBox=CCODE"))
        .and(body_string_contains("imgbtnSubmit.x=19"))
        .and(body_string_contains("imgbtnSubmit.y=23"))
        .and(body_string_contains(
            "LBD_VCID_c_logincheck_advancecheck_samplecaptcha=VCID_SUB",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string("<script>alert('OK');</script>"))
        .mount(&server)
        .await;

    let outcome = submit_verify(&client, &info, "VCODE", "CCODE")
        .await
        .expect("submit succeeds");
    // Smoke check on outcome — the field-shape mock served an alert
    // with non-success keyword so we expect ServerMessage.
    assert!(matches!(outcome, VerifyOutcome::ServerMessage(_)));
}

#[tokio::test]
async fn submit_verify_post_body_field_order_matches_wpf() {
    // Locks the **order** of the form fields, not just presence.
    // WPF `Verify.cs::verify` L79-88 emits exactly this sequence and
    // we want byte-identical wire format.
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);

    Mock::given(method("POST"))
        .and(path("/LoginCheck/AdvanceCheck.aspx"))
        .respond_with(|req: &Request| {
            let body = std::str::from_utf8(&req.body).unwrap_or("");
            // The body is x-www-form-urlencoded; check substring
            // ordering rather than full equality so the encoder is
            // free to evolve.
            let positions: Vec<Option<usize>> = [
                "__VIEWSTATE=",
                "__VIEWSTATEGENERATOR=",
                "__EVENTVALIDATION=",
                "txtVerify=",
                "CodeTextBox=",
                "imgbtnSubmit.x=",
                "imgbtnSubmit.y=",
                "LBD_VCID_c_logincheck_advancecheck_samplecaptcha=",
            ]
            .iter()
            .map(|needle| body.find(needle))
            .collect();
            let all_found = positions.iter().all(|p| p.is_some());
            let monotonic = positions.windows(2).all(|w| match (w[0], w[1]) {
                (Some(a), Some(b)) => a < b,
                _ => false,
            });
            if all_found && monotonic {
                ResponseTemplate::new(200).set_body_string("<script>alert('OK');</script>")
            } else {
                ResponseTemplate::new(400).set_body_string(format!("bad order: {body}"))
            }
        })
        .mount(&server)
        .await;

    submit_verify(&client, &info, "v", "c")
        .await
        .expect("ordered POST body accepted");
}

#[tokio::test]
async fn submit_verify_alert_success_returns_success_outcome() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);
    mount_advance_check_post(&server, "<script>alert('資料已驗證成功');</script>").await;

    let outcome = submit_verify(&client, &info, "v", "c").await.unwrap();
    assert_eq!(outcome, VerifyOutcome::Success);
}

#[tokio::test]
async fn submit_verify_alert_other_returns_server_message_verbatim() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);
    mount_advance_check_post(&server, "<script>alert('連線過於頻繁');</script>").await;

    match submit_verify(&client, &info, "v", "c").await.unwrap() {
        VerifyOutcome::ServerMessage(msg) => assert_eq!(msg, "連線過於頻繁"),
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn submit_verify_wrong_captcha_text_returns_wrong_captcha_outcome() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);
    mount_advance_check_post(&server, "<html>圖形驗證碼輸入錯誤，請重新輸入</html>").await;

    let outcome = submit_verify(&client, &info, "v", "c").await.unwrap();
    assert_eq!(outcome, VerifyOutcome::WrongCaptcha);
}

#[tokio::test]
async fn submit_verify_no_alert_no_captcha_text_returns_wrong_auth_info_outcome() {
    let server = MockServer::start().await;
    let client = tw_client_for(&server);
    let info = page_info_pointing_at(&server);
    // A plain re-rendering of the verify page with no alert and no
    // captcha-error text — WPF interprets this as "wrong
    // authentication info".
    mount_advance_check_post(&server, "<html><body>some neutral content</body></html>").await;

    let outcome = submit_verify(&client, &info, "v", "c").await.unwrap();
    assert_eq!(outcome, VerifyOutcome::WrongAuthInfo);
}
