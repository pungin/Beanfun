//! End-to-end integration tests for `services/beanfun/otp.rs`
//! (P4 chunk 4.2).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], routes every
//! [`BeanfunClient`] endpoint base at the mock, and exercises the
//! orchestrated 5-step OTP retrieval flow against canned server
//! responses that pin a specific WPF behaviour.
//!
//! Pure parsing helpers (`parse_long_polling_key`, `parse_unk_data`,
//! `parse_secret_code`, `parse_screatetime_fallback`,
//! `step_6_decrypt`, `build_get_webstart_otp_url`) are covered by
//! unit tests next to the source module; this file locks the wire
//! shapes and the orchestration on top of them.
//!
//! | Scenario                                                   | Outcome                                                                  |
//! |------------------------------------------------------------|--------------------------------------------------------------------------|
//! | TW happy path (account.screatetime=Some)                   | returns decrypted OTP, trimmed of NULs                                   |
//! | HK happy path                                              | skips unk_data parsing, otherwise identical                              |
//! | step 1 missing `GetResultByLongPolling&key=...`            | [`LoginError::OtpMissingLongPollingKey`] with bounded snippet            |
//! | TW step 1 missing `MyAccountData` literal                  | [`LoginError::OtpMissingUnkData`]                                        |
//! | account.screatetime=None + fallback regex hits             | uses fallback value verbatim in step 3 form & step 5 URL                 |
//! | account.screatetime=None + fallback regex absent           | [`LoginError::OtpMissingCreateTime`]                                     |
//! | step 2 missing `m_strSecretCode`                           | [`LoginError::OtpMissingSecretCode`]                                     |
//! | step 5 empty body                                          | [`LoginError::OtpEmptyResponse`]                                         |
//! | step 5 `parts[0] != "1"`                                   | [`LoginError::OtpServerRejected`] with raw server text                   |
//! | step 5 `parts[0] == "1"` with non-hex ciphertext           | [`LoginError::OtpDecryptionFailed`]                                      |
//! | wire shape: step 3 form payload includes `unk_data` (TW)   | wiremock body matcher confirms the extra k=v field                       |
//! | wire shape: step 5 URL contains `%20`, `ppppp=`, `sid=...` | wiremock query/path matchers confirm verbatim WPF byte format            |

use beanfun_next_lib::core::wcdes::encrypt_hex;
use beanfun_next_lib::services::beanfun::{
    get_otp, BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion, ServiceAccount,
    Session,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SERVICE_CODE: &str = "610074";
const SERVICE_REGION: &str = "T9";
const SESSION_KEY: &str = "SKEY_TEST";
const WEB_TOKEN: &str = "BFWT_test_token";
const ACCOUNT_ID: &str = "alice";

const SID: &str = "SID_test";
const SSN: &str = "1234";
const SNAME: &str = "PlayerOne";

// -----------------------------------------------------------------------------
// Fixture builders
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] for `region` with all three endpoint
/// bases routed at `server`. Step 2 of OTP is the only step that
/// depends on the region branch, but we want to test both regions
/// against the same mock server, so this helper accepts the region
/// explicitly.
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

fn account_with(screatetime: Option<&str>) -> ServiceAccount {
    ServiceAccount {
        is_enable: true,
        visible: true,
        is_inherited: false,
        sid: SID.to_string(),
        ssn: SSN.to_string(),
        sname: SNAME.to_string(),
        screatetime: screatetime.map(str::to_string),
        slastusedtime: None,
        sauthtype: None,
    }
}

/// Construct a valid step-5 envelope from a plaintext OTP and the
/// 8-byte WCDES key the server will prefix it with.
fn make_envelope(key: &str, plaintext: &str) -> String {
    let cipher_hex = encrypt_hex(plaintext, key).expect("encrypt_hex must accept key+plaintext");
    format!("1;{key}{cipher_hex}")
}

// -----------------------------------------------------------------------------
// Mock setup helpers
// -----------------------------------------------------------------------------

/// Step 1 body for TW: contains both `GetResultByLongPolling&key=...`
/// and the `MyAccountData.ServiceAccountCreateTime + "k=v";` literal.
/// `screatetime_literal` is optionally appended so the fallback regex
/// can hit (or miss) deterministically.
///
/// Each JS literal is placed on its own line — WPF's regex
/// `GetResultByLongPolling&key=(.*)"` is greedy, so `(.*)` would
/// span across multiple `"`s on the **same line**. Real production
/// responses put each statement on its own line; this fixture
/// mirrors that to avoid pathological greediness in the matcher.
fn step1_body_tw(
    long_polling_key: &str,
    unk_kv: &str,
    screatetime_literal: Option<&str>,
) -> String {
    let create_time_line = match screatetime_literal {
        Some(s) => format!("ServiceAccountCreateTime: \"{s}\";\n"),
        None => String::new(),
    };
    format!(
        "<script>\n{create_time_line}url = \"GetResultByLongPolling&key={long_polling_key}\";\nfoo = MyAccountData.ServiceAccountCreateTime + \"{unk_kv}\";\n</script>"
    )
}

/// Step 1 body for HK: only the `GetResultByLongPolling&key=...`
/// literal — no `MyAccountData` (HK skips that parse).
fn step1_body_hk(long_polling_key: &str, screatetime_literal: Option<&str>) -> String {
    let create_time_line = match screatetime_literal {
        Some(s) => format!("ServiceAccountCreateTime: \"{s}\";\n"),
        None => String::new(),
    };
    format!(
        "<script>\n{create_time_line}url = \"GetResultByLongPolling&key={long_polling_key}\";\n</script>"
    )
}

async fn mount_step1(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/beanfun_block/game_zone/game_start_step2.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

async fn mount_step2(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/generic_handlers/get_cookies.ashx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

async fn mount_step3_ok(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(
            "/beanfun_block/generic_handlers/record_service_start.ashx",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

async fn mount_step4_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/generic_handlers/get_result.ashx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

async fn mount_step5(server: &MockServer, envelope: &str) {
    Mock::given(method("GET"))
        .and(path(
            "/beanfun_block/generic_handlers/get_webstart_otp.ashx",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope.to_owned()))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Group A — Happy paths
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_happy_path_returns_decrypted_otp() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-15 12:34:56"));
    let envelope = make_envelope("ABCDEFGH", "OTP12345");

    mount_step1(&server, &step1_body_tw("LPK_OK", "extraKey=extraVal", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SECRET_OK';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    mount_step5(&server, &envelope).await;

    let otp = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("OTP retrieval succeeds on happy path");
    assert_eq!(otp, "OTP12345");
}

#[tokio::test]
async fn hk_happy_path_skips_unk_data_parsing() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::HK);
    let session = test_session(LoginRegion::HK);
    let account = account_with(Some("2024-02-29 00:00:01"));
    let envelope = make_envelope("HKKEY123", "HKOTP567");

    // HK step 1 body has no `MyAccountData` literal — the HK branch
    // never tries to parse it.
    mount_step1(&server, &step1_body_hk("LPK_HK", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SECRET_HK';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    mount_step5(&server, &envelope).await;

    let otp = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("OTP retrieval succeeds on HK happy path");
    assert_eq!(otp, "HKOTP567");
}

// -----------------------------------------------------------------------------
// Group B — Step 1 errors
// -----------------------------------------------------------------------------

#[tokio::test]
async fn step1_missing_long_polling_key_returns_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    mount_step1(&server, "<html>no key here at all</html>").await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 1 should fail");
    match err {
        LoginError::OtpMissingLongPollingKey { snippet } => {
            assert!(snippet.contains("no key here"));
        }
        other => panic!("expected OtpMissingLongPollingKey, got {other:?}"),
    }
}

#[tokio::test]
async fn tw_step1_missing_unk_data_returns_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    // Has the long-polling key but **no** MyAccountData literal.
    mount_step1(
        &server,
        r#"<script>url = "GetResultByLongPolling&key=LPK"; </script>"#,
    )
    .await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("TW step 1 should fail without unk_data");
    assert!(matches!(err, LoginError::OtpMissingUnkData));
}

#[tokio::test]
async fn step1_screatetime_none_with_fallback_regex_uses_fallback_value() {
    // When `account.screatetime == None`, the orchestrator falls back
    // to scraping `ServiceAccountCreateTime: "..."` from step 1's
    // body, and that value must propagate verbatim into step 3's form
    // payload AND step 5's `CreateTime=...` URL parameter.
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(None);
    let envelope = make_envelope("KEY12345", "OTP54321");

    let fallback_create_time = "2099-12-31 23:59:59";
    mount_step1(
        &server,
        &step1_body_tw("LPK_OK", "k=v", Some(fallback_create_time)),
    )
    .await;
    mount_step2(&server, "var m_strSecretCode = 'SC';").await;

    // Assert step 3 carries the fallback create_time in its form body.
    Mock::given(method("POST"))
        .and(path(
            "/beanfun_block/generic_handlers/record_service_start.ashx",
        ))
        .and(body_string_contains(
            // form-urlencoded: space → `+`, `:` → `%3A`
            "service_account_create_time=2099-12-31+23%3A59%3A59",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;
    mount_step4_ok(&server).await;

    // Assert step 5 URL carries the fallback create_time with `%20`
    // encoding for the space (NOT `+`).
    Mock::given(method("GET"))
        .and(path(
            "/beanfun_block/generic_handlers/get_webstart_otp.ashx",
        ))
        .and(query_param("CreateTime", fallback_create_time))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope.clone()))
        .mount(&server)
        .await;

    let otp = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("fallback create_time should drive a successful flow");
    assert_eq!(otp, "OTP54321");
}

#[tokio::test]
async fn step1_screatetime_none_without_fallback_returns_missing_create_time() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(None);

    // Step 1 has the long-polling key + unk_data but **no**
    // `ServiceAccountCreateTime: "..."` literal for the fallback.
    mount_step1(&server, &step1_body_tw("LPK", "k=v", None)).await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 1 should fail without create_time fallback");
    assert!(matches!(err, LoginError::OtpMissingCreateTime));
}

// -----------------------------------------------------------------------------
// Group C — Step 2 errors
// -----------------------------------------------------------------------------

#[tokio::test]
async fn step2_missing_secret_code_returns_typed_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    mount_step1(&server, &step1_body_tw("LPK", "k=v", None)).await;
    mount_step2(&server, "<html>no secret code in this body</html>").await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 2 should fail");
    assert!(matches!(err, LoginError::OtpMissingSecretCode));
}

// -----------------------------------------------------------------------------
// Group D — Step 5 errors
// -----------------------------------------------------------------------------

#[tokio::test]
async fn step5_empty_body_returns_empty_response_error() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    mount_step1(&server, &step1_body_tw("LPK", "k=v", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SC';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    mount_step5(&server, "").await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 5 empty body should fail");
    assert!(matches!(err, LoginError::OtpEmptyResponse));
}

#[tokio::test]
async fn step5_server_rejection_surfaces_message_verbatim() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    mount_step1(&server, &step1_body_tw("LPK", "k=v", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SC';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    mount_step5(&server, "0;maintenance until 03:00").await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 5 should surface server rejection");
    match err {
        LoginError::OtpServerRejected { message } => {
            assert_eq!(message, "maintenance until 03:00");
        }
        other => panic!("expected OtpServerRejected, got {other:?}"),
    }
}

#[tokio::test]
async fn step5_invalid_hex_ciphertext_is_decryption_failed() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-01 00:00:00"));

    mount_step1(&server, &step1_body_tw("LPK", "k=v", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SC';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    // status=1, key=ABCDEFGH, ciphertext=ZZZZZZZZZZZZZZZZ (not hex).
    mount_step5(&server, "1;ABCDEFGHZZZZZZZZZZZZZZZZ").await;

    let err = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect_err("step 5 invalid hex should fail decryption");
    assert!(matches!(err, LoginError::OtpDecryptionFailed { .. }));
}

// -----------------------------------------------------------------------------
// Group E — Wire shape locks
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_step3_form_payload_includes_unk_data_kv() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-15 12:34:56"));
    let envelope = make_envelope("ABCDEFGH", "OTP_____");

    mount_step1(&server, &step1_body_tw("LPK", "extraK=extraV", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SC';").await;
    // Step 3 mock asserts the form body contains the verbatim
    // unk_data key=value pair and the standard 6 fields.
    Mock::given(method("POST"))
        .and(path(
            "/beanfun_block/generic_handlers/record_service_start.ashx",
        ))
        .and(body_string_contains("service_account_id=SID_test"))
        .and(body_string_contains("sotp=1234"))
        .and(body_string_contains(
            "service_account_display_name=PlayerOne",
        ))
        .and(body_string_contains("extraK=extraV"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;
    mount_step4_ok(&server).await;
    mount_step5(&server, &envelope).await;

    let otp = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("happy flow");
    assert_eq!(otp, "OTP_____");
}

#[tokio::test]
async fn step5_url_carries_ppppp_literal_and_percent20_create_time() {
    let server = MockServer::start().await;
    let client = client_for(&server, LoginRegion::TW);
    let session = test_session(LoginRegion::TW);
    let account = account_with(Some("2024-01-15 12:34:56"));
    let envelope = make_envelope("ABCDEFGH", "OK______");

    mount_step1(&server, &step1_body_tw("LPK_X", "k=v", None)).await;
    mount_step2(&server, "var m_strSecretCode = 'SECRET';").await;
    mount_step3_ok(&server).await;
    mount_step4_ok(&server).await;
    // wiremock's `query_param` decodes percent-encoding before
    // matching, so a literal `2024-01-15 12:34:56` here proves the
    // server received `CreateTime=2024-01-15%2012:34:56` on the
    // wire (NOT `+`-encoded form). The `ppppp=` literal is matched
    // verbatim too.
    Mock::given(method("GET"))
        .and(path(
            "/beanfun_block/generic_handlers/get_webstart_otp.ashx",
        ))
        .and(query_param("CreateTime", "2024-01-15 12:34:56"))
        .and(query_param(
            "ppppp",
            "1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA",
        ))
        .and(query_param("WebToken", WEB_TOKEN))
        .and(query_param("SN", "LPK_X"))
        .and(query_param("SecretCode", "SECRET"))
        .and(query_param("ServiceCode", SERVICE_CODE))
        .and(query_param("ServiceRegion", SERVICE_REGION))
        .and(query_param("ServiceAccount", SID))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope.clone()))
        .mount(&server)
        .await;

    let otp = get_otp(&client, &session, &account, SERVICE_CODE, SERVICE_REGION)
        .await
        .expect("step 5 URL shape happy path");
    assert_eq!(otp, "OK______");
}
