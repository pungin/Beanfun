//! Integration tests for the top-level login dispatcher
//! (`login/orchestrator.rs`).
//!
//! Per-flow branch coverage already lives in `tests/tw_login.rs`,
//! `tests/hk_login.rs`, `tests/totp.rs`, and the QR test files.
//! These tests prove only the **dispatch contract**: the right
//! `LoginMethod` variant invokes the right downstream flow, and
//! the credentials + service args plumb through correctly.
//!
//! | Dispatch path                                                     | Covered by                                               |
//! |-------------------------------------------------------------------|----------------------------------------------------------|
//! | `LoginMethod::TwRegular` → `login_tw_regular`                     | `tw_regular_dispatches_to_tw_login_flow`                 |
//! | `LoginMethod::HkRegular` (defaults) → `login_hk_regular`          | `hk_regular_dispatches_to_hk_login_flow`                 |
//! | `LoginMethod::HkRegular` (custom service args) flow through       | `hk_regular_passes_service_metadata_through_to_session`  |
//!
//! Each test crate is its own compile unit, so the mock helpers
//! below intentionally re-build minimal happy-path fixtures rather
//! than reaching into `tw_login.rs` / `hk_login.rs`.

use beanfun_lib::services::beanfun::{
    login::{login_with, LoginMethod},
    BeanfunClient, ClientConfig, Credentials, Endpoints, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ACCOUNT: &str = "alice";
const PASSWORD: &str = "hunter2";
const WEB_TOKEN: &str = "BFWT_orchestrator";

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

fn creds() -> Credentials {
    Credentials::new(ACCOUNT, PASSWORD)
}

/// `return.aspx` 302 with `bfWebToken=…` Set-Cookie — same shape
/// both flows expect at the LoginCompleted tail.
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
    mount_after_landing(server).await;
}

/// `GET /after` → `200 OK` landing page. `login_completed` follows
/// redirects (WPF L863 `UploadString` auto-follow parity), so the
/// 302 above needs a reachable target or the chain surfaces 404 as
/// `LoginError::Unknown`.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

// -- TW happy fixtures --------------------------------------------------------

const TW_SKEY: &str = "TW_ORCH_SKEY";
const TW_FORM_TOKEN: &str = "VTOKEN_orch";

async fn mount_tw_login_happy(server: &MockServer) {
    let location = format!("{}/login/id-pass.aspx?pSKey={}", server.uri(), TW_SKEY);
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

    let index_html = format!(
        r#"<html><body>
            <input name="__RequestVerificationToken" type="hidden" value="{TW_FORM_TOKEN}" />
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/Login/Index"))
        .respond_with(ResponseTemplate::new(200).set_body_string(index_html))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ResultCode": "1",
            "ResultData": { "Captcha": "" }
        })))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/Login/AccountLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ResultCode": "1",
            "Result": "0",
            "ResultMessage": ""
        })))
        .mount(server)
        .await;

    let send_login_html = r#"<html><body>
        <form action="/beanfun_block/bflogin/return.aspx" method="post">
            <input type="hidden" name="SessionKey" value="SKEY_INNER" />
            <input type="hidden" name="AuthKey" value="AUTH_INNER" />
            <input type="hidden" name="ServiceCode" value="610074" />
        </form>
    </body></html>"#;
    Mock::given(method("GET"))
        .and(path("/Login/SendLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_string(send_login_html))
        .mount(server)
        .await;

    mount_return_aspx_with_token(server, WEB_TOKEN).await;
}

// -- HK happy fixtures --------------------------------------------------------

const HK_SKEY: &str = "HK_ORCH_SKEY";
const HK_VIEWSTATE: &str = "VS_ORCH";
const HK_VIEWSTATE_GEN: &str = "GEN_ORCH";
const HK_EVENT_VALIDATION: &str = "EV_ORCH";
const HK_AKEY: &str = "AKEY_orch";

async fn mount_hk_login_happy(server: &MockServer) {
    let body = format!(
        r#"<html><body>
            <span id="ctl00_ContentPlaceHolder1_lblOtp1">{HK_SKEY}</span>
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;

    let login_page_html = format!(
        r#"<html><body><form>
            <input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="{HK_VIEWSTATE}" />
            <input type="hidden" name="__VIEWSTATEGENERATOR" id="__VIEWSTATEGENERATOR" value="{HK_VIEWSTATE_GEN}" />
            <input type="hidden" name="__EVENTVALIDATION" id="__EVENTVALIDATION" value="{HK_EVENT_VALIDATION}" />
        </form></body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(login_page_html))
        .mount(server)
        .await;

    let landing = format!("{}/hk-landing?akey={HK_AKEY}", server.uri());
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

    mount_return_aspx_with_token(server, WEB_TOKEN).await;
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_regular_dispatches_to_tw_login_flow() {
    let server = MockServer::start().await;
    mount_tw_login_happy(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    let session = login_with(&client, LoginMethod::TwRegular, &creds())
        .await
        .expect("TW dispatch must succeed");

    // Region + skey + web_token are the canonical "this came from
    // the TW path" markers; no other flow produces this combo with
    // these fixtures.
    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, TW_SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    // creds plumb-through assertion — orchestrator must pass the
    // same `&Credentials` it received without mangling them.
    assert_eq!(session.account_id, ACCOUNT);
}

#[tokio::test]
async fn hk_regular_dispatches_to_hk_login_flow() {
    let server = MockServer::start().await;
    mount_hk_login_happy(&server).await;
    let client = client_for(&server, LoginRegion::HK);

    let session = login_with(
        &client,
        LoginMethod::HkRegular {
            service_code: LoginRegion::HK.default_service_code(),
            service_region: LoginRegion::HK.default_service_region(),
        },
        &creds(),
    )
    .await
    .expect("HK dispatch must succeed");

    assert_eq!(session.region, LoginRegion::HK);
    assert_eq!(session.skey, HK_SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
    // With defaults passed, the session should reflect the region's
    // canonical service metadata.
    assert_eq!(session.service_code, LoginRegion::HK.default_service_code());
    assert_eq!(
        session.service_region,
        LoginRegion::HK.default_service_region()
    );
}

#[tokio::test]
async fn hk_regular_passes_service_metadata_through_to_session() {
    // Arbitrary non-default service args. If the orchestrator ever
    // hardcodes defaults instead of forwarding the variant's
    // payload, this test fails on the `service_code` /
    // `service_region` assertions below. This is the canonical
    // "creds + service args plumb-through" lock the user called
    // out for the dispatcher.
    const CUSTOM_SERVICE_CODE: &str = "999999";
    const CUSTOM_SERVICE_REGION: &str = "T0";

    let server = MockServer::start().await;
    mount_hk_login_happy(&server).await;
    let client = client_for(&server, LoginRegion::HK);

    let session = login_with(
        &client,
        LoginMethod::HkRegular {
            service_code: CUSTOM_SERVICE_CODE,
            service_region: CUSTOM_SERVICE_REGION,
        },
        &creds(),
    )
    .await
    .expect("HK dispatch with custom service args must succeed");

    assert_eq!(session.service_code, CUSTOM_SERVICE_CODE);
    assert_eq!(session.service_region, CUSTOM_SERVICE_REGION);
    assert_eq!(session.account_id, ACCOUNT);
}
