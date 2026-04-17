//! End-to-end integration tests for `services/beanfun/account.rs`
//! (P4 chunk 4.1).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], routes every
//! [`BeanfunClient`] endpoint base at the mock, and exercises one
//! public function against a canned server response that pins a
//! specific WPF behaviour.
//!
//! | Function                                  | Cases covered                                                                                                                                          |
//! |-------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------|
//! | [`get_accounts`]                          | happy multi-row (sort by ssn) / quota notice with `進階認證` / quota notice with other text / partial create_time failures degrade to `None` / no rows |
//! | [`get_service_contract`]                  | happy / `intResult != 1` returns empty                                                                                                                  |
//! | [`add_service_account`]                   | happy / empty name skips request / `intResult != 1` returns false                                                                                       |
//! | [`change_service_account_display_name`]   | happy / `new_name == account.sname` skips request / empty `new_name` skips request                                                                      |
//!
//! Pure helpers (`classify_amount_limit_notice`, `parse_int_result_eq_one`)
//! are covered by unit tests next to the source module; this file
//! locks the HTTP wire shapes and the orchestration on top of them.

use beanfun_next_lib::services::beanfun::{
    add_service_account, change_service_account_display_name, get_accounts, get_service_contract,
    AmountLimitNotice, BeanfunClient, ClientConfig, Endpoints, LoginRegion, ServiceAccount,
    Session,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SERVICE_CODE: &str = "610074";
const SERVICE_REGION: &str = "T9";
const ACCOUNT_ID: &str = "alice";
const SESSION_KEY: &str = "SKEY_TEST";
const WEB_TOKEN: &str = "BFWT_test_token";

// -----------------------------------------------------------------------------
// Fixture builders
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] whose three endpoint bases all point at
/// `server`. The region is TW by default — every account.rs endpoint
/// is region-routed via `portal_url`, which targets `portal_base`.
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

/// Build a fixed [`Session`] for tests.
fn test_session() -> Session {
    Session::new(
        LoginRegion::TW,
        SESSION_KEY,
        WEB_TOKEN,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
}

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step
// -----------------------------------------------------------------------------

/// Mount `auth.aspx` returning 200 with an empty body. The caller of
/// `get_accounts` discards the body anyway; we just need the request
/// to succeed.
async fn mount_auth_aspx(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/beanfun_block/auth.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

/// Mount `game_server_account_list.aspx` returning the supplied HTML
/// body (200).
async fn mount_account_list(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path(
            "/beanfun_block/game_zone/game_server_account_list.aspx",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

/// Mount `game_start_step2.aspx` for one specific `sotp` value with a
/// 200 body containing the supplied `create_time`. Use this when a
/// per-row `get_create_time` is expected to succeed.
async fn mount_create_time_ok(server: &MockServer, sotp: &str, create_time: &str) {
    let body = format!(r#"<script>ServiceAccountCreateTime: "{create_time}"</script>"#);
    Mock::given(method("GET"))
        .and(path("/beanfun_block/game_zone/game_start_step2.aspx"))
        .and(query_param("sotp", sotp))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Mount `game_start_step2.aspx` for one specific `sotp` returning 404.
/// Use this when a per-row `get_create_time` is expected to silently
/// degrade to `None`.
async fn mount_create_time_404(server: &MockServer, sotp: &str) {
    Mock::given(method("GET"))
        .and(path("/beanfun_block/game_zone/game_start_step2.aspx"))
        .and(query_param("sotp", sotp))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
}

/// Mount `gamezone.ashx` returning the supplied JSON body for any POST
/// hitting it. Tests that assert on the request body should use a more
/// targeted mock instead.
async fn mount_gamezone_json(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/generic_handlers/gamezone.ashx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.to_owned())
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// get_accounts
// -----------------------------------------------------------------------------

/// Multi-row HTML, every row gets a successful create_time, sorted by
/// ssn, no quota notice → returns the rows in sorted order with all
/// fields populated.
#[tokio::test]
async fn get_accounts_happy_multi_row_sorts_by_ssn_and_fills_create_time() {
    let server = MockServer::start().await;
    mount_auth_aspx(&server).await;
    // Rows in DOM order: ssn=222, ssn=111, ssn=333. After sort: 111, 222, 333.
    let html = r##"
<a onclick="x"><div id="bbb" sn="222" name="Bravo"></div></a>
<a onclick="x"><div id="aaa" sn="111" name="Alpha"></div></a>
<a onclick="x"><div id="ccc" sn="333" name="Charlie"></div></a>
"##;
    mount_account_list(&server, html).await;
    mount_create_time_ok(&server, "111", "2024-01-01 00:00:00").await;
    mount_create_time_ok(&server, "222", "2024-02-02 00:00:00").await;
    mount_create_time_ok(&server, "333", "2024-03-03 00:00:00").await;

    let client = client_for(&server);
    let session = test_session();
    let result = get_accounts(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("happy path returns Ok");

    assert_eq!(result.amount_limit_notice, AmountLimitNotice::None);
    assert_eq!(result.accounts.len(), 3);

    // Sorted by ssn ascending.
    assert_eq!(result.accounts[0].ssn, "111");
    assert_eq!(result.accounts[0].sname, "Alpha");
    assert_eq!(
        result.accounts[0].screatetime.as_deref(),
        Some("2024-01-01 00:00:00")
    );
    assert!(result.accounts[0].is_enable);
    assert!(result.accounts[0].visible);
    assert!(!result.accounts[0].is_inherited);
    assert_eq!(result.accounts[0].slastusedtime, None);
    assert_eq!(result.accounts[0].sauthtype, None);

    assert_eq!(result.accounts[1].ssn, "222");
    assert_eq!(result.accounts[1].sname, "Bravo");
    assert_eq!(result.accounts[2].ssn, "333");
    assert_eq!(result.accounts[2].sname, "Charlie");
}

/// Quota notice contains the `進階認證` substring → classified as
/// [`AmountLimitNotice::AuthReLoginRequired`].
#[tokio::test]
async fn get_accounts_quota_notice_with_advance_auth_keyword_classified() {
    let server = MockServer::start().await;
    mount_auth_aspx(&server).await;
    let html = r##"
<a onclick="x"><div id="aaa" sn="1" name="Solo"></div></a>
<div id="divServiceAccountAmountLimitNotice" class="InnerContent">需要進階認證才能再新增帳號</div>
"##;
    mount_account_list(&server, html).await;
    mount_create_time_ok(&server, "1", "2024-01-01 00:00:00").await;

    let client = client_for(&server);
    let session = test_session();
    let result = get_accounts(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert_eq!(
        result.amount_limit_notice,
        AmountLimitNotice::AuthReLoginRequired
    );
    assert_eq!(result.accounts.len(), 1);
}

/// Quota notice without the `進階認證` substring → classified as
/// [`AmountLimitNotice::Other`] carrying the raw text verbatim.
#[tokio::test]
async fn get_accounts_quota_notice_other_text_preserved_verbatim() {
    let server = MockServer::start().await;
    mount_auth_aspx(&server).await;
    let html = r##"
<a onclick="x"><div id="aaa" sn="1" name="Solo"></div></a>
<div id="divServiceAccountAmountLimitNotice" class="InnerContent">已達 5 個服務帳號上限。</div>
"##;
    mount_account_list(&server, html).await;
    mount_create_time_ok(&server, "1", "2024-01-01 00:00:00").await;

    let client = client_for(&server);
    let session = test_session();
    let result = get_accounts(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert_eq!(
        result.amount_limit_notice,
        AmountLimitNotice::Other("已達 5 個服務帳號上限。".to_owned())
    );
}

/// `get_create_time` failure for one row: the row stays in the list
/// with `screatetime = None`. Mirrors WPF
/// `GetCreateTime`'s `catch { return null; }`.
#[tokio::test]
async fn get_accounts_partial_create_time_failures_keep_screatetime_none() {
    let server = MockServer::start().await;
    mount_auth_aspx(&server).await;
    let html = r##"
<a onclick="x"><div id="aaa" sn="1" name="OK"></div></a>
<a onclick="x"><div id="bbb" sn="2" name="Broken"></div></a>
"##;
    mount_account_list(&server, html).await;
    mount_create_time_ok(&server, "1", "2024-01-01 00:00:00").await;
    mount_create_time_404(&server, "2").await;

    let client = client_for(&server);
    let session = test_session();
    let result = get_accounts(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert_eq!(result.accounts.len(), 2);
    let ok_row = result.accounts.iter().find(|a| a.ssn == "1").unwrap();
    let broken_row = result.accounts.iter().find(|a| a.ssn == "2").unwrap();
    assert_eq!(ok_row.screatetime.as_deref(), Some("2024-01-01 00:00:00"));
    assert_eq!(broken_row.screatetime, None);
}

/// Empty list page → empty `accounts`, no notice, no error. Locks the
/// "no rows + no notice = quiet success" contract.
#[tokio::test]
async fn get_accounts_no_rows_returns_empty_list_no_notice() {
    let server = MockServer::start().await;
    mount_auth_aspx(&server).await;
    mount_account_list(&server, "<html>no rows here</html>").await;

    let client = client_for(&server);
    let session = test_session();
    let result = get_accounts(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert!(result.accounts.is_empty());
    assert_eq!(result.amount_limit_notice, AmountLimitNotice::None);
}

// -----------------------------------------------------------------------------
// get_service_contract
// -----------------------------------------------------------------------------

#[tokio::test]
async fn get_service_contract_happy_returns_str_result() {
    let server = MockServer::start().await;
    mount_gamezone_json(&server, r#"{"intResult":1,"strResult":"<p>EULA</p>"}"#).await;

    let client = client_for(&server);
    let session = test_session();
    let contract = get_service_contract(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert_eq!(contract, "<p>EULA</p>");
}

/// `intResult != 1` → returns empty string (matches WPF return ""
/// short-circuit at `Account.cs` L682-683).
#[tokio::test]
async fn get_service_contract_int_result_not_one_returns_empty() {
    let server = MockServer::start().await;
    mount_gamezone_json(
        &server,
        r#"{"intResult":0,"strResult":"should not be returned"}"#,
    )
    .await;

    let client = client_for(&server);
    let session = test_session();
    let contract = get_service_contract(&client, &session, SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert_eq!(contract, "");
}

// -----------------------------------------------------------------------------
// add_service_account
// -----------------------------------------------------------------------------

#[tokio::test]
async fn add_service_account_happy_returns_true() {
    let server = MockServer::start().await;
    // Verify the request body carries the WPF-shaped form fields. We
    // only assert on the most diagnostic ones; reqwest's `.form()`
    // url-encodes them and the order may vary across reqwest versions.
    Mock::given(method("POST"))
        .and(path("/generic_handlers/gamezone.ashx"))
        .and(body_string_contains("strFunction=AddServiceAccount"))
        .and(body_string_contains("sadn=NewAccount"))
        .and(body_string_contains(format!("sc={SERVICE_CODE}")))
        .and(body_string_contains(format!("sr={SERVICE_REGION}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"intResult":1}"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server);
    let session = test_session();
    let ok = add_service_account(
        &client,
        &session,
        "NewAccount",
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .unwrap();

    assert!(ok);
}

/// Empty name → returns `false` *without firing the request*. We
/// register no mock so any HTTP attempt would surface as a connection
/// error (which the test would catch).
#[tokio::test]
async fn add_service_account_empty_name_returns_false_no_request() {
    let server = MockServer::start().await;
    // Intentionally no mocks mounted.

    let client = client_for(&server);
    let session = test_session();
    let ok = add_service_account(&client, &session, "", SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert!(!ok);
    // No request should have hit the mock; wiremock surfaces excess
    // requests on drop, so the absence of a panic here is the
    // assertion.
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn add_service_account_int_result_not_one_returns_false() {
    let server = MockServer::start().await;
    mount_gamezone_json(&server, r#"{"intResult":0}"#).await;

    let client = client_for(&server);
    let session = test_session();
    let ok = add_service_account(&client, &session, "AnyName", SERVICE_CODE, SERVICE_REGION)
        .await
        .unwrap();

    assert!(!ok);
}

// -----------------------------------------------------------------------------
// change_service_account_display_name
// -----------------------------------------------------------------------------

fn fixture_account() -> ServiceAccount {
    ServiceAccount {
        is_enable: true,
        visible: true,
        is_inherited: false,
        sid: "sid_test".to_owned(),
        ssn: "12345".to_owned(),
        sname: "OldName".to_owned(),
        screatetime: Some("2024-01-01 00:00:00".to_owned()),
        slastusedtime: None,
        sauthtype: None,
    }
}

#[tokio::test]
async fn change_display_name_happy_returns_true() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/generic_handlers/gamezone.ashx"))
        .and(body_string_contains(
            "strFunction=ChangeServiceAccountDisplayName",
        ))
        .and(body_string_contains("said=sid_test"))
        .and(body_string_contains("nsadn=BrandNewName"))
        .and(body_string_contains(format!(
            "sl={SERVICE_CODE}_{SERVICE_REGION}"
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"intResult":1}"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server);
    let session = test_session();
    let acc = fixture_account();
    let ok = change_service_account_display_name(
        &client,
        &session,
        "BrandNewName",
        &format!("{SERVICE_CODE}_{SERVICE_REGION}"),
        &acc,
    )
    .await
    .unwrap();

    assert!(ok);
}

/// `new_name == account.sname` → returns `false` *without firing the
/// request*. Matches WPF's early-return at `Account.cs` L646.
#[tokio::test]
async fn change_display_name_same_as_existing_returns_false_no_request() {
    let server = MockServer::start().await;
    let client = client_for(&server);
    let session = test_session();
    let acc = fixture_account();
    let same_name = acc.sname.clone();
    let ok = change_service_account_display_name(
        &client,
        &session,
        &same_name,
        &format!("{SERVICE_CODE}_{SERVICE_REGION}"),
        &acc,
    )
    .await
    .unwrap();

    assert!(!ok);
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// Empty `new_name` → returns `false` *without firing the request*.
/// Matches WPF's early-return on `newName == ""` at `Account.cs` L646.
#[tokio::test]
async fn change_display_name_empty_new_name_returns_false_no_request() {
    let server = MockServer::start().await;
    let client = client_for(&server);
    let session = test_session();
    let acc = fixture_account();
    let ok = change_service_account_display_name(
        &client,
        &session,
        "",
        &format!("{SERVICE_CODE}_{SERVICE_REGION}"),
        &acc,
    )
    .await
    .unwrap();

    assert!(!ok);
    assert!(server.received_requests().await.unwrap().is_empty());
}
