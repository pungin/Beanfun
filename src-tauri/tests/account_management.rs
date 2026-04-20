//! End-to-end integration tests for the WebForms account-management
//! surface in `services/beanfun/account.rs` (P4 chunk 4.4).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], routes every
//! [`BeanfunClient`] endpoint base at the mock, and exercises one of the
//! five public functions against canned responses that pin a specific
//! WPF behaviour.
//!
//! | Function                                       | Cases covered                                                                                                                           |
//! |------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
//! | [`unconnected_game_init_add_account_payload`]  | TW happy path (CheckNickName supported) / HK happy path (CheckNickName disabled) / missing GameName / missing AccountLen / GET-step missing __VIEWSTATE |
//! | [`unconnected_game_add_account_check`]         | TW happy path returns refreshed session + lblErrorMessage / no-DN skips `t1` field / HK uses `txtServiceAccountDN` instead of `t1`      |
//! | [`unconnected_game_add_account_check_nickname`]| sends `__EVENTTARGET=lbtnCheckNickName` and empty `txtServiceAccountID`                                                                  |
//! | [`unconnected_game_add_account`]               | success (empty lblErrorMessage) / rejection (populated lblErrorMessage) / empty name short-circuits to `LoginError::Unknown`             |
//! | [`unconnected_game_change_password`]           | 5-step success returns `ChangePasswordOutcome::VerifyCodeSent` / lblErrorMessage rejection / neither signal yields `LoginError::Unknown` |
//!
//! Pure helpers (`mgmt_url`, `change_password_url`, `parse_viewstate_triplet`,
//! `build_viewstate_payload_prefix`, `push_account_dn`, `build_add_account_form`,
//! `extract_lbl_error_message`, `extract_verify_code_from_url`) are
//! covered by unit tests next to the source module; this file locks
//! the HTTP wire shapes and the orchestration on top of them.

use beanfun_lib::services::beanfun::{
    unconnected_game_add_account, unconnected_game_add_account_check,
    unconnected_game_add_account_check_nickname, unconnected_game_change_password,
    unconnected_game_init_add_account_payload, AddAccountOutcome, AddAccountSession, BeanfunClient,
    ChangePasswordOutcome, ClientConfig, Endpoints, LoginError, LoginRegion, Session,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const SERVICE_CODE: &str = "610074";
const SERVICE_REGION: &str = "T9";
const ACCOUNT_ID: &str = "alice";
const SESSION_KEY: &str = "SKEY_TEST";
const WEB_TOKEN: &str = "BFWT_test_token";

// -----------------------------------------------------------------------------
// Fixture builders
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] in `region` with all three endpoint bases
/// pointed at `server`. The wire-level `https://` vs `http://` split that
/// `change_password_url` introduces for HK is exercised via the unit
/// tests; integration tests run against `http://localhost` either way
/// (`set_scheme("http")` is a no-op when the base is already http).
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

fn test_session(region: LoginRegion) -> Session {
    Session::new(
        region,
        SESSION_KEY,
        WEB_TOKEN,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
}

fn fake_mgmt_session(region: LoginRegion) -> AddAccountSession {
    AddAccountSession {
        viewstate: "VS_FIXED".to_owned(),
        viewstate_generator: "GEN_FIXED".to_owned(),
        event_validation: "EV_FIXED".to_owned(),
        region,
    }
}

// -----------------------------------------------------------------------------
// HTML fixtures
// -----------------------------------------------------------------------------

/// `auth.aspx` (initial GET) page with __VIEWSTATE + __VIEWSTATEGENERATOR.
fn auth_aspx_page() -> String {
    r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_AUTH" />
<input id="__VIEWSTATEGENERATOR" value="GEN_AUTH" />
</form></body></html>"#
        .to_owned()
}

/// `02.aspx` POST response (init_add_account success path) — full
/// triplet plus lblGameName + lblAccountLen + the `lbtnCheckNickName`
/// anchor that gates the dialog's nickname row.
fn init_add_account_response_full() -> String {
    r##"<html><body><form>
<input id="__VIEWSTATE" value="VS_INIT" />
<input id="__VIEWSTATEGENERATOR" value="GEN_INIT" />
<input id="__EVENTVALIDATION" value="EV_INIT" />
<span id="lblGameName">新楓之谷</span>
<span id="lblAccountLen">6 - 12</span>
<a id="lbtnCheckNickName" href="#">Check nickname</a>
</form></body></html>"##
        .to_owned()
}

/// Same as [`init_add_account_response_full`] but without the
/// `lbtnCheckNickName` anchor — exercises the
/// `check_nickname_supported = false` branch.
fn init_add_account_response_no_check_nickname() -> String {
    r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_INIT_HK" />
<input id="__VIEWSTATEGENERATOR" value="GEN_INIT_HK" />
<input id="__EVENTVALIDATION" value="EV_INIT_HK" />
<span id="lblGameName">楓之谷HK</span>
<span id="lblAccountLen">8 - 16</span>
</form></body></html>"#
        .to_owned()
}

/// `02.aspx` POST response with the triplet but **no** `lblGameName`.
fn init_add_account_response_missing_game_name() -> String {
    r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_X" />
<input id="__VIEWSTATEGENERATOR" value="GEN_X" />
<input id="__EVENTVALIDATION" value="EV_X" />
<span id="lblAccountLen">6 - 12</span>
</form></body></html>"#
        .to_owned()
}

/// `02.aspx` POST response with the triplet + lblGameName but no
/// `lblAccountLen`.
fn init_add_account_response_missing_account_len() -> String {
    r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_X" />
<input id="__VIEWSTATEGENERATOR" value="GEN_X" />
<input id="__EVENTVALIDATION" value="EV_X" />
<span id="lblGameName">XYZ</span>
</form></body></html>"#
        .to_owned()
}

/// `auth.aspx` page that's missing __VIEWSTATE entirely (forces the
/// GET-step typed error in `init_account_payload`).
fn auth_aspx_page_missing_viewstate() -> String {
    r#"<html><body>
<input id="__VIEWSTATEGENERATOR" value="GEN_X" />
</body></html>"#
        .to_owned()
}

/// Generic `02.aspx` POST response carrying just the triplet — used by
/// the check / add variants where `lblGameName` / `lblAccountLen` are
/// not parsed.
fn check_response(error_message: &str) -> String {
    let lbl = if error_message.is_empty() {
        String::new()
    } else {
        format!(r#"<span id="lblErrorMessage" style="color:Red;">{error_message}</span>"#)
    };
    format!(
        r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_NEXT" />
<input id="__VIEWSTATEGENERATOR" value="GEN_NEXT" />
<input id="__EVENTVALIDATION" value="EV_NEXT" />
{lbl}
</form></body></html>"#
    )
}

/// `01Accounts.aspx` / `03.aspx` GET response with the triplet that
/// `change_password` parses on steps 2 & 4.
fn change_password_triplet_page(suffix: &str) -> String {
    format!(
        r#"<html><body><form>
<input id="__VIEWSTATE" value="VS_{suffix}" />
<input id="__VIEWSTATEGENERATOR" value="GEN_{suffix}" />
<input id="__EVENTVALIDATION" value="EV_{suffix}" />
</form></body></html>"#
    )
}

// -----------------------------------------------------------------------------
// Group A — init_add_account_payload
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_init_add_account_happy_path_parses_full_metadata() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(init_add_account_response_full()))
        .mount(&server)
        .await;

    let init =
        unconnected_game_init_add_account_payload(&client, &session, SERVICE_CODE, SERVICE_REGION)
            .await
            .expect("TW init_add_account succeeds");

    assert_eq!(init.session.viewstate, "VS_INIT");
    assert_eq!(init.session.viewstate_generator, "GEN_INIT");
    assert_eq!(init.session.event_validation, "EV_INIT");
    assert_eq!(init.session.region, LoginRegion::TW);
    assert_eq!(init.game_name, "新楓之谷");
    assert_eq!(init.account_len, "6 - 12");
    assert!(
        init.check_nickname_supported,
        "lbtnCheckNickName anchor present ⇒ flag must be true"
    );
}

#[tokio::test]
async fn hk_init_add_account_happy_path_with_check_nickname_disabled() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);
    let session = test_session(LoginRegion::HK);

    Mock::given(method("GET"))
        .and(path("/HK/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/HK/accounts_management/02.aspx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(init_add_account_response_no_check_nickname()),
        )
        .mount(&server)
        .await;

    let init =
        unconnected_game_init_add_account_payload(&client, &session, SERVICE_CODE, SERVICE_REGION)
            .await
            .expect("HK init_add_account succeeds");

    assert_eq!(init.session.region, LoginRegion::HK);
    assert_eq!(init.account_len, "8 - 16");
    assert!(
        !init.check_nickname_supported,
        "no lbtnCheckNickName anchor ⇒ flag must be false (UI hides nickname row)"
    );
}

#[tokio::test]
async fn init_add_account_missing_game_name_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(init_add_account_response_missing_game_name()),
        )
        .mount(&server)
        .await;

    let err =
        unconnected_game_init_add_account_payload(&client, &session, SERVICE_CODE, SERVICE_REGION)
            .await
            .expect_err("missing lblGameName ⇒ typed error");
    assert!(matches!(err, LoginError::AccountMgmtMissingGameName));
}

#[tokio::test]
async fn init_add_account_missing_account_len_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(init_add_account_response_missing_account_len()),
        )
        .mount(&server)
        .await;

    let err =
        unconnected_game_init_add_account_payload(&client, &session, SERVICE_CODE, SERVICE_REGION)
            .await
            .expect_err("missing lblAccountLen ⇒ typed error");
    assert!(matches!(err, LoginError::AccountMgmtMissingAccountLen));
}

#[tokio::test]
async fn init_add_account_get_step_missing_viewstate_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(auth_aspx_page_missing_viewstate()),
        )
        .mount(&server)
        .await;

    let err =
        unconnected_game_init_add_account_payload(&client, &session, SERVICE_CODE, SERVICE_REGION)
            .await
            .expect_err("GET-step missing viewstate ⇒ typed error");
    assert!(matches!(err, LoginError::AccountMgmtMissingViewState));
}

// -----------------------------------------------------------------------------
// Group B — add_account_check[_nickname]
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_add_account_check_returns_refreshed_session_and_lbl_error_message() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .and(body_string_contains("__EVENTTARGET=lbtnCheckAccount"))
        .and(body_string_contains("txtServiceAccountID=newAcc"))
        .and(body_string_contains("t1=MyDN"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("帳號已存在")))
        .mount(&server)
        .await;

    let outcome = unconnected_game_add_account_check(
        &client,
        &session,
        &mgmt_session,
        "newAcc",
        Some("MyDN"),
    )
    .await
    .expect("TW add_account_check succeeds at the HTTP layer");

    assert_eq!(outcome.session.viewstate, "VS_NEXT");
    assert_eq!(outcome.session.viewstate_generator, "GEN_NEXT");
    assert_eq!(outcome.session.event_validation, "EV_NEXT");
    assert_eq!(outcome.session.region, LoginRegion::TW);
    assert_eq!(outcome.error_message, "帳號已存在");
}

#[tokio::test]
async fn add_account_check_no_dn_skips_t1_field() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    // Capture the request body so we can assert what was *not* sent.
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .respond_with(move |req: &Request| {
            let body = std::str::from_utf8(&req.body).unwrap_or("");
            assert!(
                !body.contains("t1="),
                "no DN passed ⇒ `t1=` must be absent, got body: {body}"
            );
            assert!(
                !body.contains("txtServiceAccountDN="),
                "TW client must not emit txtServiceAccountDN"
            );
            ResponseTemplate::new(200).set_body_string(check_response(""))
        })
        .mount(&server)
        .await;

    let outcome =
        unconnected_game_add_account_check(&client, &session, &mgmt_session, "newAcc", None)
            .await
            .expect("call succeeds");
    assert_eq!(outcome.error_message, "");
}

#[tokio::test]
async fn hk_add_account_check_uses_long_dn_field_name() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);
    let session = test_session(LoginRegion::HK);
    let mgmt_session = fake_mgmt_session(LoginRegion::HK);

    Mock::given(method("POST"))
        .and(path("/HK/accounts_management/02.aspx"))
        .and(body_string_contains("txtServiceAccountDN=HKDN"))
        .and(body_string_contains("__VIEWSTATEENCRYPTED=")) // HK marker present
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("")))
        .mount(&server)
        .await;

    let outcome = unconnected_game_add_account_check(
        &client,
        &session,
        &mgmt_session,
        "newAcc",
        Some("HKDN"),
    )
    .await
    .expect("HK add_account_check succeeds");
    assert_eq!(outcome.session.region, LoginRegion::HK);
}

#[tokio::test]
async fn add_account_check_nickname_uses_lbtn_check_nickname_event_target() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .and(body_string_contains("__EVENTTARGET=lbtnCheckNickName"))
        // WPF L372 sends `txtServiceAccountID=` (empty value) for the
        // nickname-check variant.
        .and(body_string_contains("txtServiceAccountID=&"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("")))
        .mount(&server)
        .await;

    let outcome =
        unconnected_game_add_account_check_nickname(&client, &session, &mgmt_session, Some("MyDN"))
            .await
            .expect("nickname check succeeds");
    assert_eq!(outcome.error_message, "");
}

// -----------------------------------------------------------------------------
// Group C — add_account
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_add_account_success_when_lbl_error_message_is_empty() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .and(body_string_contains("chkBox1=on"))
        .and(body_string_contains("imgbtn_Submit.x=0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("")))
        .mount(&server)
        .await;

    let outcome = unconnected_game_add_account(
        &client,
        &session,
        &mgmt_session,
        "newAcc",
        "P@ssw0rd!",
        "P@ssw0rd!",
        Some("MyDN"),
    )
    .await
    .expect("add_account call succeeds");
    assert_eq!(outcome, AddAccountOutcome::Success);
}

#[tokio::test]
async fn add_account_with_lbl_error_returns_error_message_outcome() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/02.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("密碼強度不足")))
        .mount(&server)
        .await;

    let outcome = unconnected_game_add_account(
        &client,
        &session,
        &mgmt_session,
        "newAcc",
        "weak",
        "weak",
        None,
    )
    .await
    .expect("add_account call succeeds at HTTP layer");
    assert_eq!(
        outcome,
        AddAccountOutcome::ErrorMessage("密碼強度不足".to_owned())
    );
}

#[tokio::test]
async fn add_account_empty_name_short_circuits_to_typed_error_no_request_fired() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let mgmt_session = fake_mgmt_session(LoginRegion::TW);

    // Deliberately mount no expectation: if the early-return short-circuit
    // is broken and the function fires a POST, wiremock will 404 and the
    // test surfaces the regression as a transport error here.
    let err = unconnected_game_add_account(
        &client,
        &session,
        &mgmt_session,
        "",
        "P@ssw0rd!",
        "P@ssw0rd!",
        None,
    )
    .await
    .expect_err("empty name ⇒ typed error before any HTTP call");
    assert!(
        matches!(err, LoginError::Unknown(ref msg) if msg.contains("name")),
        "expected LoginError::Unknown about empty name, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Group D — change_password (5-step orchestration)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_change_password_5_step_happy_path_returns_verify_code_token() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    // Step 1 — auth.aspx (cookie seed + viewstate parse).
    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    // Step 2 — GET 01Accounts.aspx returns triplet.
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S2")),
        )
        .mount(&server)
        .await;
    // Step 3 — POST 01Accounts.aspx (response discarded).
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .and(body_string_contains("__EVENTTARGET=gvServiceAccountList"))
        .and(body_string_contains("__EVENTARGUMENT=ChangePassword%243"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ignored"))
        .mount(&server)
        .await;
    // Step 4 — GET 03.aspx returns triplet.
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/03.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S4")),
        )
        .mount(&server)
        .await;
    // Step 5 — POST 03.aspx returns 302 → /done?verify_code=ABC123XYZ
    // (we mount the redirect target too so reqwest can follow).
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/03.aspx"))
        .and(body_string_contains("txtEmail=user%40example.com"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", "/done?verify_code=ABC123XYZ"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/done"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>landed</html>"))
        .mount(&server)
        .await;

    let outcome = unconnected_game_change_password(
        &client,
        &session,
        SERVICE_CODE,
        SERVICE_REGION,
        3,
        "user@example.com",
    )
    .await
    .expect("change_password 5-step succeeds");

    assert_eq!(
        outcome,
        ChangePasswordOutcome::VerifyCodeSent("ABC123XYZ".to_owned())
    );
}

#[tokio::test]
async fn change_password_with_lbl_error_returns_error_message_outcome() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S2")),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ignored"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/03.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S4")),
        )
        .mount(&server)
        .await;
    // Step 5 — server returns 200 with lblErrorMessage populated.
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/03.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("Email 格式錯誤")))
        .mount(&server)
        .await;

    let outcome = unconnected_game_change_password(
        &client,
        &session,
        SERVICE_CODE,
        SERVICE_REGION,
        0,
        "not-an-email",
    )
    .await
    .expect("call succeeds at HTTP layer");
    assert_eq!(
        outcome,
        ChangePasswordOutcome::ErrorMessage("Email 格式錯誤".to_owned())
    );
}

#[tokio::test]
async fn change_password_neither_token_nor_lbl_returns_unknown_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);

    Mock::given(method("GET"))
        .and(path("/TW/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(auth_aspx_page()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S2")),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/01Accounts.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ignored"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/TW/accounts_management/03.aspx"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(change_password_triplet_page("S4")),
        )
        .mount(&server)
        .await;
    // Step 5 — 200 with no lblErrorMessage and no verify_code redirect.
    Mock::given(method("POST"))
        .and(path("/TW/accounts_management/03.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(check_response("")))
        .mount(&server)
        .await;

    let err = unconnected_game_change_password(
        &client,
        &session,
        SERVICE_CODE,
        SERVICE_REGION,
        0,
        "user@example.com",
    )
    .await
    .expect_err("neither outcome signal ⇒ Unknown error");
    assert!(
        matches!(err, LoginError::Unknown(ref msg) if msg.contains("verify_code")),
        "expected LoginError::Unknown mentioning verify_code, got {err:?}"
    );
}
