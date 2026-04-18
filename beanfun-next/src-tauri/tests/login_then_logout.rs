//! Cross-flow integration tests: drive a full Regular login, then call
//! [`logout`] against the same client, and assert both phases hit the
//! right endpoint sequence end-to-end.
//!
//! The per-step tests for login (`tests/tw_login.rs`,
//! `tests/hk_login.rs`) and logout (`tests/logout.rs`) cover the
//! happy / failure branches in isolation. The point of THIS file is
//! to prove the two phases compose: the cookie jar primed by login
//! flows through to logout, the orchestrator orderings interleave
//! correctly, and the design decisions specific to chunk 3.5
//! (best-effort logout, no cookie-jar clear) survive a realistic
//! end-to-end exercise.
//!
//! | Scenario                                                 | Covered by                                                       |
//! |----------------------------------------------------------|------------------------------------------------------------------|
//! | TW Regular login → logout → all 3 logout endpoints hit   | `tw_regular_then_logout_hits_all_login_and_3_logout_steps`       |
//! | HK Regular login → logout → 2 logout endpoints + skip 3  | `hk_regular_then_logout_hits_all_login_and_2_logout_steps`       |
//! | Cookie jar NOT cleared by logout (WPF-aligned design)    | both tests assert the jar is non-empty after logout              |
//!
//! Each test crate in `tests/` is a separate compilation unit, so
//! the mount helpers below intentionally duplicate the ones in
//! `tw_login.rs` / `hk_login.rs` / `logout.rs` rather than reaching
//! across crates. Same trade-off `hk_login.rs::mount_return_aspx_with_token`
//! (L132-134) already calls out.

use beanfun_next_lib::services::beanfun::{
    login::{login_hk_regular, login_tw_regular, logout},
    BeanfunClient, ClientConfig, Credentials, Endpoints, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ACCOUNT: &str = "alice";
const PASSWORD: &str = "hunter2";
const WEB_TOKEN: &str = "BFWT_cross_flow";

// -----------------------------------------------------------------------------
// Shared mock helpers (duplicated across crates; see module docs)
// -----------------------------------------------------------------------------

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

/// `return.aspx` — 302 redirect carrying a `bfWebToken=…` Set-Cookie.
/// Reused by both the login finish step and (incidentally) defines
/// the cookie that should still be in the jar after logout.
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

/// `GET /after` → `200 OK` landing page. HK Regular's
/// `login_completed` follows redirects (WPF L863 parity), so the
/// 302 mounted above needs a reachable target.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

// -- TW login fixtures --------------------------------------------------------

const TW_SKEY: &str = "TW_TEST_SKEY";
const TW_FORM_TOKEN: &str = "VTOKEN_tw_xflow";

async fn mount_tw_login_happy(server: &MockServer) {
    // session_key — 302 to id-pass.aspx?pSKey=…
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

    // Login/Index with __RequestVerificationToken
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

    // Login/CheckAccountType — empty captcha (no advance check)
    let check_body = serde_json::json!({
        "ResultCode": "1",
        "ResultData": { "Captcha": "" }
    });
    Mock::given(method("POST"))
        .and(path("/Login/CheckAccountType"))
        .respond_with(ResponseTemplate::new(200).set_body_json(check_body))
        .mount(server)
        .await;

    // Login/AccountLogin — happy {ResultCode:"1", Result:"0"}
    let login_body = serde_json::json!({
        "ResultCode": "1",
        "Result": "0",
        "ResultMessage": ""
    });
    Mock::given(method("POST"))
        .and(path("/Login/AccountLogin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(login_body))
        .mount(server)
        .await;

    // Login/SendLogin — form with three hidden inputs
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

// -- HK login fixtures --------------------------------------------------------

const HK_SKEY: &str = "HK_TEST_SKEY";
const HK_VIEWSTATE: &str = "VS_HK";
const HK_VIEWSTATE_GEN: &str = "GEN_HK";
const HK_EVENT_VALIDATION: &str = "EV_HK";
const HK_AKEY: &str = "AKEY_HK_xflow";

async fn mount_hk_login_happy(server: &MockServer) {
    // session_key — HK delivers it inline in the body
    let session_body = format!(
        r#"<html><body>
            <span id="ctl00_ContentPlaceHolder1_lblOtp1">{HK_SKEY}</span>
        </body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/beanfun_block/bflogin/default.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(session_body))
        .mount(server)
        .await;

    // HK login page — VIEWSTATE triad
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

    // POST credentials — 302 to akey landing
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

// -- Logout fixtures ----------------------------------------------------------

async fn mount_logout_step1(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/generic_handlers/remove_bflogin_session.ashx"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

async fn mount_logout_step2(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/logout.aspx"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

async fn mount_logout_step3(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/generic_handlers/erase_token.ashx"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_regular_then_logout_hits_all_login_and_3_logout_steps() {
    let server = MockServer::start().await;
    mount_tw_login_happy(&server).await;
    mount_logout_step1(&server).await;
    mount_logout_step2(&server).await;
    mount_logout_step3(&server).await;
    let client = client_for(&server, LoginRegion::TW);

    // Phase 1: login
    let session = login_tw_regular(&client, &creds())
        .await
        .expect("TW login must succeed");
    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, TW_SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);

    // Snapshot how many requests we'd seen at the boundary so the
    // logout assertion below can focus on just the post-login ones.
    let pre_logout_request_count = server.received_requests().await.unwrap().len();

    // Phase 2: logout
    logout(&client).await.expect("TW logout must succeed");

    let received = server.received_requests().await.unwrap();
    let logout_paths: Vec<_> = received
        .iter()
        .skip(pre_logout_request_count)
        .map(|r| r.url.path().to_owned())
        .collect();
    assert_eq!(
        logout_paths,
        vec![
            "/generic_handlers/remove_bflogin_session.ashx".to_owned(),
            "/logout.aspx".to_owned(),
            "/generic_handlers/erase_token.ashx".to_owned(),
        ],
        "TW cross-flow: logout must fire all 3 endpoints in WPF order"
    );

    // Cookie-jar policy lock (chunk 3.5 design: never_clear). The
    // jar should STILL carry `bfWebToken` after logout — wiremock
    // didn't expire it and our logout deliberately doesn't clear.
    let cookie_jar = client.cookie_store();
    let jar = cookie_jar.lock().expect("cookie jar lock");
    let still_present = jar.iter_unexpired().any(|c| c.name() == "bfWebToken");
    assert!(
        still_present,
        "WPF-aligned design: logout must NOT clear the cookie jar"
    );
}

#[tokio::test]
async fn hk_regular_then_logout_hits_all_login_and_2_logout_steps() {
    let server = MockServer::start().await;
    mount_hk_login_happy(&server).await;
    mount_logout_step1(&server).await;
    mount_logout_step2(&server).await;
    // step 3 deliberately NOT mounted — HK must skip it. If HK
    // accidentally calls `erase_token.ashx`, wiremock 404s and
    // logout returns LoginError::Unknown, failing this test.
    let client = client_for(&server, LoginRegion::HK);

    // Phase 1: login
    let session = login_hk_regular(
        &client,
        &creds(),
        LoginRegion::HK.default_service_code(),
        LoginRegion::HK.default_service_region(),
    )
    .await
    .expect("HK login must succeed");
    assert_eq!(session.region, LoginRegion::HK);
    assert_eq!(session.skey, HK_SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);

    let pre_logout_request_count = server.received_requests().await.unwrap().len();

    // Phase 2: logout
    logout(&client).await.expect("HK logout must succeed");

    let received = server.received_requests().await.unwrap();
    let logout_paths: Vec<_> = received
        .iter()
        .skip(pre_logout_request_count)
        .map(|r| r.url.path().to_owned())
        .collect();
    assert_eq!(
        logout_paths,
        vec![
            "/generic_handlers/remove_bflogin_session.ashx".to_owned(),
            "/logout.aspx".to_owned(),
        ],
        "HK cross-flow: logout must fire only 2 endpoints (WPF skips erase_token for HK)"
    );

    // Same cookie-jar lock as the TW test — the policy is region-
    // independent.
    let cookie_jar = client.cookie_store();
    let jar = cookie_jar.lock().expect("cookie jar lock");
    let still_present = jar.iter_unexpired().any(|c| c.name() == "bfWebToken");
    assert!(
        still_present,
        "WPF-aligned design: HK logout must NOT clear the cookie jar"
    );
}
