//! End-to-end integration tests for the HK Regular login orchestrator
//! (`login/hk_regular.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a HK
//! [`BeanfunClient`] at it, and drives
//! [`login_hk_regular`](beanfun_next_lib::services::beanfun::login::login_hk_regular)
//! against a set of canned responses that reproduce one branch of the
//! real server's behaviour:
//!
//! | Branch                      | Covered by                           |
//! |-----------------------------|--------------------------------------|
//! | Happy path (akey redirect)  | `hk_regular_happy_path_returns_session` |
//! | TOTP required               | `hk_regular_totp_triggered_returns_challenge` |
//! | Advance-check (captcha)     | `hk_regular_advance_check_returns_advance_check_required` |
//! | MsgBox error                | `hk_regular_msgbox_error_surfaces_server_message` |
//! | pollRequest error           | `hk_regular_poll_request_error_concats_url_and_param` |
//! | Missing `__VIEWSTATE`       | `hk_regular_missing_viewstate_returns_parser_error` |
//! | Missing generator/event val.| `hk_regular_missing_viewstate_generator_returns_error` |
//! | Unrecognised error body     | `hk_regular_unrecognised_body_no_akey_returns_missing_akey` |
//! | POST wire shape             | `hk_regular_post_body_contains_credentials_and_viewstate` |
//!
//! Pure decode / classification unit tests live next to the source
//! modules (`hk_error.rs`, `hk_regular.rs`); this file covers the
//! **orchestration** — step ordering, branching, and the downstream
//! `login_completed` hand-off.

use beanfun_next_lib::services::beanfun::{
    login::login_hk_regular, BeanfunClient, ClientConfig, Credentials, Endpoints, LoginError,
    LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ACCOUNT: &str = "alice";
const PASSWORD: &str = "hunter2";
const SKEY: &str = "HK_TEST_SKEY";
const VIEWSTATE: &str = "VS_HK";
const VIEWSTATE_GEN: &str = "GEN_HK";
const EVENT_VALIDATION: &str = "EV_HK";
const AKEY: &str = "AKEY_HK_HAPPY";
const WEB_TOKEN: &str = "BFWT_hk_happy";

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step
// -----------------------------------------------------------------------------

/// Portal entry — HK delivers the session key inline in the body
/// inside the `ctl00_ContentPlaceHolder1_lblOtp1` span (mirrors the
/// real WPF `GetSessionkey` HK branch at L734-742).
async fn mount_hk_session_key(server: &MockServer) {
    let body = format!(
        r#"<html><body>
            <span id="ctl00_ContentPlaceHolder1_lblOtp1">{SKEY}</span>
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// HK login page — responds with the `__VIEWSTATE*` triad. Any field
/// name passed as `""` is simply omitted, which lets us craft
/// "missing ___" scenarios with the same helper.
async fn mount_hk_login_page(
    server: &MockServer,
    viewstate: &str,
    generator: &str,
    event_validation: &str,
) {
    let mut html = String::from("<html><body><form>");
    if !viewstate.is_empty() {
        html.push_str(&format!(
            r#"<input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="{viewstate}" />"#
        ));
    }
    if !generator.is_empty() {
        html.push_str(&format!(
            r#"<input type="hidden" name="__VIEWSTATEGENERATOR" id="__VIEWSTATEGENERATOR" value="{generator}" />"#
        ));
    }
    if !event_validation.is_empty() {
        html.push_str(&format!(
            r#"<input type="hidden" name="__EVENTVALIDATION" id="__EVENTVALIDATION" value="{event_validation}" />"#
        ));
    }
    html.push_str("</form></body></html>");

    Mock::given(method("GET"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(server)
        .await;
}

/// Happy-path variant — all three viewstate fields present.
async fn mount_hk_login_page_happy(server: &MockServer) {
    mount_hk_login_page(server, VIEWSTATE, VIEWSTATE_GEN, EVENT_VALIDATION).await;
}

/// POST credentials → 302 redirect carrying `akey=…` on the final
/// URL. Two mocks: the 302 itself plus the landing page it redirects
/// to (so reqwest's follow-redirects doesn't 404).
async fn mount_hk_credentials_post_redirects_with_akey(server: &MockServer, akey: &str) {
    let landing = format!("{}/hk-landing?akey={akey}", server.uri());
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .respond_with(ResponseTemplate::new(302).append_header("Location", landing.as_str()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/hk-landing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hk landing"))
        .mount(server)
        .await;
}

/// POST credentials → 200 response with a custom body. Reused by the
/// TOTP / advance-check / MsgBox / pollRequest branches since they
/// differ only in the body the server returns.
async fn mount_hk_credentials_post_with_body(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

/// `return.aspx` — the shared `login_completed` tail. Mirrors
/// `tests/login_completed.rs::mount_return_aspx_with_token`; duplicated
/// here so this test file is self-contained (each integration test
/// crate is its own compilation unit).
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

// -----------------------------------------------------------------------------
// Client + creds builders
// -----------------------------------------------------------------------------

fn client_for(server: &MockServer) -> BeanfunClient {
    let base = Url::parse(&format!("{}/", server.uri())).expect("mock URL parses");
    let endpoints = Endpoints {
        login_base: base.clone(),
        portal_base: base.clone(),
        newlogin_base: base,
    };
    let mut cfg = ClientConfig::for_region(LoginRegion::HK);
    cfg.endpoints = endpoints;
    BeanfunClient::new(cfg).expect("client builds")
}

fn creds() -> Credentials {
    Credentials::new(ACCOUNT, PASSWORD)
}

/// Drive the HK Regular flow with the WPF-default game slot
/// (new MapleStory — `610074` / `T9`). Tests that care about
/// service-metadata propagation (see
/// `hk_regular_custom_service_metadata_flows_to_session`) skip this
/// wrapper and call `login_hk_regular` directly with the custom
/// values they want to verify.
async fn run_hk_regular(
    client: &BeanfunClient,
) -> Result<beanfun_next_lib::services::beanfun::Session, LoginError> {
    login_hk_regular(
        client,
        &creds(),
        LoginRegion::HK.default_service_code(),
        LoginRegion::HK.default_service_region(),
    )
    .await
}

// -----------------------------------------------------------------------------
// Tests — happy path and continuations
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_regular_happy_path_returns_session() {
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_redirects_with_akey(&server, AKEY).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = run_hk_regular(&client)
        .await
        .expect("HK happy path must succeed");

    assert_eq!(session.region, LoginRegion::HK);
    assert_eq!(session.skey, SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
    // HK defaults come from `LoginRegion::HK.default_service_code/region`.
    // We assert on them so any drift in the region defaults surfaces
    // here — they are part of the observable session contract.
    assert_eq!(session.service_code, LoginRegion::HK.default_service_code());
    assert_eq!(
        session.service_region,
        LoginRegion::HK.default_service_region()
    );
}

#[tokio::test]
async fn hk_regular_custom_service_metadata_flows_to_session() {
    // Audit fix for chunks 3.3.2 + 3.3.3: WPF `HkRegularLogin`
    // (L191-195) and `TotpLogin` (L303-311) both accept
    // service_code / service_region parameters, and the sole call
    // site (`MainWindow.xaml.cs` L1542-1551) passes
    // `this.service_code` / `this.service_region` which may have
    // been overridden from saved config at startup
    // (`MainWindow.xaml.cs` L357-358).
    //
    // We must thread non-default values through `login_hk_regular`
    // all the way to the observable `Session`. This test locks in
    // that contract with a synthetic slot (`999999` / `TZ`) that
    // would never be the WPF default — a regression to the old
    // hardcoded `region.default_service_code()` call inside
    // `login_completed`'s dispatch would fail this assertion.
    const CUSTOM_SERVICE_CODE: &str = "999999";
    const CUSTOM_SERVICE_REGION: &str = "TZ";

    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_redirects_with_akey(&server, AKEY).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = login_hk_regular(
        &client,
        &creds(),
        CUSTOM_SERVICE_CODE,
        CUSTOM_SERVICE_REGION,
    )
    .await
    .expect("HK happy path with custom service metadata must succeed");

    assert_eq!(session.service_code, CUSTOM_SERVICE_CODE);
    assert_eq!(session.service_region, CUSTOM_SERVICE_REGION);
    // Sanity: other session fields are unchanged by the metadata
    // swap — the values travel through strictly as pass-through.
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
}

#[tokio::test]
async fn hk_regular_totp_triggered_returns_challenge() {
    // A TOTP-enabled HK account gets `totpLoginBtn` in the POST
    // response body instead of a redirect.
    let totp_body = format!(
        r#"<html><body><form>
            <input type="hidden" id="__VIEWSTATE" name="__VIEWSTATE" value="{VIEWSTATE}" />
            <input type="submit" id="totpLoginBtn" value="登入" />
        </form></body></html>"#
    );
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_with_body(&server, &totp_body).await;
    // Intentionally no return.aspx mount — TOTP branch must short-
    // circuit before `login_completed`.

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("TOTP-enabled account must error with a challenge");

    match err {
        LoginError::TotpRequired(challenge) => {
            // The challenge should carry the HK URL we scraped from
            // (i.e. include `otp1={SKEY}` in its query).
            assert!(
                challenge
                    .totp_url()
                    .as_str()
                    .contains(&format!("otp1={SKEY}")),
                "challenge URL must preserve the otp1 query: {}",
                challenge.totp_url()
            );
            assert_eq!(challenge.account_id(), ACCOUNT);
            // session_key / viewstate are crate-private, so all we
            // can observe from this crate is account_id + totp_url.
            // That's sufficient — the unit test in `totp_challenge.rs`
            // already covers Debug redaction.
        }
        other => panic!("expected TotpRequired, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Tests — error branches
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_regular_advance_check_returns_advance_check_required() {
    let body = "<script>if(window.RELOAD_CAPTCHA_CODE){alert('need captcha');}</script>";
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_with_body(&server, body).await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("RELOAD_CAPTCHA_CODE + alert must trigger advance check");

    // HK never sets the URL — WPF L247-251 only sets the errmsg, the
    // url stays whatever was there before. Our typed variant carries
    // `None` to make that absence explicit.
    match err {
        LoginError::AdvanceCheckRequired { url: None } => {}
        other => panic!("expected AdvanceCheckRequired{{url:None}}, got {other:?}"),
    }
}

#[tokio::test]
async fn hk_regular_msgbox_error_surfaces_server_message() {
    let body =
        r#"<script type="text/javascript">$(function(){MsgBox.Show('帳號或密碼錯誤');});</script>"#;
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_with_body(&server, body).await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("MsgBox body must surface as ServerMessage");

    match err {
        LoginError::ServerMessage(msg) => assert_eq!(msg, "帳號或密碼錯誤"),
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn hk_regular_poll_request_error_concats_url_and_param() {
    let body = r#"<div>pollRequest("/poll/url","TOKEN_HK","extra_param");</div>"#;
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_with_body(&server, body).await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("pollRequest body must surface as ServerMessage");

    match err {
        LoginError::ServerMessage(msg) => {
            // WPF L277-280 exact concatenation: g1 + `","` + g3.
            assert_eq!(msg, "/poll/url\",\"extra_param");
        }
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn hk_regular_missing_viewstate_returns_parser_error() {
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    // Generator + event validation present, but NO __VIEWSTATE.
    mount_hk_login_page(&server, "", VIEWSTATE_GEN, EVENT_VALIDATION).await;
    // POST mount intentionally omitted — the flow must abort before
    // the credentials POST.

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("missing __VIEWSTATE must error");

    // `extract_viewstate` surfaces `ParserError::MissingViewState`,
    // which `LoginError` maps to its own `MissingViewState` variant
    // via `From<ParserError>`. We assert on the final public shape.
    assert!(
        matches!(err, LoginError::MissingViewState),
        "expected MissingViewState, got {err:?}"
    );
}

#[tokio::test]
async fn hk_regular_missing_viewstate_generator_returns_error() {
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    // __VIEWSTATE + __EVENTVALIDATION present, __VIEWSTATEGENERATOR absent.
    // HK requires all three — our orchestrator enforces that even
    // though `extract_viewstate` itself returns `None` for the
    // generator.
    mount_hk_login_page(&server, VIEWSTATE, "", EVENT_VALIDATION).await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("missing __VIEWSTATEGENERATOR must error");

    assert!(
        matches!(err, LoginError::MissingViewStateGenerator),
        "expected MissingViewStateGenerator, got {err:?}"
    );
}

#[tokio::test]
async fn hk_regular_missing_event_validation_returns_error() {
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page(&server, VIEWSTATE, VIEWSTATE_GEN, "").await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("missing __EVENTVALIDATION must error");

    assert!(
        matches!(err, LoginError::MissingEventValidation),
        "expected MissingEventValidation, got {err:?}"
    );
}

#[tokio::test]
async fn hk_regular_missing_both_optional_fields_prefers_event_validation() {
    // Regression lock for WPF's check ordering at
    // `BeanfunClient.Login.cs` L218-232 — `__EVENTVALIDATION` is
    // evaluated *before* `__VIEWSTATEGENERATOR`, so when both are
    // simultaneously absent the observable error must be
    // `MissingEventValidation`, never `MissingViewStateGenerator`.
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page(&server, VIEWSTATE, "", "").await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("missing both __EVENTVALIDATION and __VIEWSTATEGENERATOR must error");

    assert!(
        matches!(err, LoginError::MissingEventValidation),
        "WPF checks EVENTVALIDATION before VIEWSTATEGENERATOR; expected \
         MissingEventValidation, got {err:?}"
    );
}

#[tokio::test]
async fn hk_regular_unrecognised_body_no_akey_returns_missing_akey() {
    // The POST response carries no redirect, no MsgBox, no
    // pollRequest, no TOTP marker, no advance-check marker. WPF L264
    // defaults `errmsg = "LoginNoAkey"` — we surface `MissingAkey`.
    let body = "<html><body>completely unrelated content</body></html>";
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;
    mount_hk_credentials_post_with_body(&server, body).await;

    let client = client_for(&server);
    let err = run_hk_regular(&client)
        .await
        .expect_err("unrecognised body must error with MissingAkey");

    assert!(
        matches!(err, LoginError::MissingAkey),
        "expected MissingAkey, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Tests — wire shape (request body contents)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn hk_regular_post_body_contains_credentials_and_viewstate() {
    // Verify the credentials POST carries the account, password, all
    // three viewstate fields, and the `btn_login` literal. Field
    // ORDER is pinned by the unit test in `hk_regular.rs`; here we
    // only assert key=value presence (order-independent).
    let server = MockServer::start().await;
    mount_hk_session_key(&server).await;
    mount_hk_login_page_happy(&server).await;

    let landing = format!("{}/hk-landing?akey={AKEY}", server.uri());
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .and(body_string_contains(format!("t_AccountID={ACCOUNT}")))
        .and(body_string_contains(format!("t_Password={PASSWORD}")))
        .and(body_string_contains(format!("__VIEWSTATE={VIEWSTATE}")))
        .and(body_string_contains(format!(
            "__VIEWSTATEGENERATOR={VIEWSTATE_GEN}"
        )))
        .and(body_string_contains(format!(
            "__EVENTVALIDATION={EVENT_VALIDATION}"
        )))
        // `登入` — literal Traditional Chinese label, URL-encoded by
        // reqwest's `.form()` to the percent-encoded UTF-8 bytes.
        // We assert the decoded key=encoded-value shape to pin the
        // WPF-matching wire behaviour.
        .and(body_string_contains("btn_login=%E7%99%BB%E5%85%A5"))
        .respond_with(ResponseTemplate::new(302).append_header("Location", landing.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/hk-landing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hk landing"))
        .mount(&server)
        .await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = run_hk_regular(&client)
        .await
        .expect("POST with matching body must succeed");
    assert_eq!(session.web_token, WEB_TOKEN);
}
