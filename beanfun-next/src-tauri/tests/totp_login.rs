//! End-to-end integration tests for the TOTP continuation
//! orchestrator (`login/totp.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], drives
//! `login_hk_regular` to obtain a real [`TotpChallenge`] via the
//! `LoginError::TotpRequired` branch, then feeds that challenge into
//! [`login_totp`] to exercise a single TOTP-response branch.
//!
//! | Branch                        | Covered by                                 |
//! |-------------------------------|--------------------------------------------|
//! | Happy path (akey redirect)    | `totp_happy_path_returns_session`          |
//! | Advance-check (captcha)       | `totp_advance_check_returns_advance_check_required` |
//! | MsgBox error                  | `totp_msgbox_error_surfaces_server_message`|
//! | pollRequest error             | `totp_poll_request_surfaces_device_registration_required` |
//! | Unrecognised error body       | `totp_unrecognised_body_no_akey_returns_missing_akey` |
//! | HK wire shape (w/ encrypted)  | `totp_hk_post_body_has_six_otps_and_viewstate_encrypted` |
//! | TW wire shape (no encrypted)  | `totp_tw_post_body_drops_viewstate_encrypted` |
//!
//! The two wire-shape tests verify the `__VIEWSTATEENCRYPTED`
//! region branch (WPF `TotpLogin` L347-348, `if App.LoginRegion ==
//! "HK"`). For the TW case the `TotpChallenge` is produced by the HK
//! orchestrator (no TW TOTP producer exists yet) and handed to a
//! TW-configured [`BeanfunClient`] ? this is a controlled setup that
//! isolates the region branch from producer-side variables.

use beanfun_next_lib::services::beanfun::{
    login::{login_hk_regular, login_totp, TotpChallenge},
    BeanfunClient, ClientConfig, Credentials, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ACCOUNT: &str = "alice";
const PASSWORD: &str = "hunter2";
const SKEY: &str = "HK_TOTP_SKEY";
const VIEWSTATE: &str = "VS_TOTP";
const VIEWSTATE_GEN: &str = "GEN_TOTP";
const EVENT_VALIDATION: &str = "EV_TOTP";
const AKEY: &str = "AKEY_TOTP_HAPPY";
const WEB_TOKEN: &str = "BFWT_totp_happy";
const OTPS: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

// -----------------------------------------------------------------------------
// Mock setup ? session key + HK regular POST (always the same shape)
// -----------------------------------------------------------------------------

/// HK portal entry ? session key via `ctl00_ContentPlaceHolder1_lblOtp1`.
async fn mount_session_key(server: &MockServer) {
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

/// HK login page ? the viewstate triad embedded in the regular
/// credentials form.
async fn mount_hk_login_page(server: &MockServer) {
    let html = format!(
        r#"<html><body><form>
            <input type="hidden" id="__VIEWSTATE" name="__VIEWSTATE" value="{VIEWSTATE}" />
            <input type="hidden" id="__VIEWSTATEGENERATOR" name="__VIEWSTATEGENERATOR" value="{VIEWSTATE_GEN}" />
            <input type="hidden" id="__EVENTVALIDATION" name="__EVENTVALIDATION" value="{EVENT_VALIDATION}" />
        </form></body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(server)
        .await;
}

/// HK Regular credentials POST ? the server echoes back a new page
/// containing the `totpLoginBtn` marker, which makes `login_hk_regular`
/// return `LoginError::TotpRequired(challenge)`. Differentiated from
/// the TOTP POST by the presence of `t_AccountID` in the form body.
async fn mount_hk_credentials_post_returns_totp_form(server: &MockServer) {
    let body = format!(
        r#"<html><body><form>
            <input type="hidden" id="__VIEWSTATE" name="__VIEWSTATE" value="{VIEWSTATE}" />
            <input type="hidden" id="__VIEWSTATEGENERATOR" name="__VIEWSTATEGENERATOR" value="{VIEWSTATE_GEN}" />
            <input type="hidden" id="__EVENTVALIDATION" name="__EVENTVALIDATION" value="{EVENT_VALIDATION}" />
            <input type="submit" id="totpLoginBtn" value="??" />
        </form></body></html>"#
    );
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .and(body_string_contains(format!("t_AccountID={ACCOUNT}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Mock setup ? TOTP POST (the branch under test in each scenario)
// -----------------------------------------------------------------------------

/// TOTP POST ? 302 redirect carrying `akey=?` on the landing URL.
/// Matched by `otpCode1` in the body, which is only present on TOTP
/// submissions.
async fn mount_totp_post_redirects_with_akey(server: &MockServer, akey: &str) {
    let landing = format!("{}/totp-landing?akey={akey}", server.uri());
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .and(body_string_contains("otpCode1="))
        .respond_with(ResponseTemplate::new(302).append_header("Location", landing.as_str()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/totp-landing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("totp landing"))
        .mount(server)
        .await;
}

/// TOTP POST ? 200 with a custom body (used for error-branch tests).
async fn mount_totp_post_with_body(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/login/id-pass_form_newBF.aspx"))
        .and(body_string_contains("otpCode1="))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

/// `return.aspx` shared-tail mock. Copy-of-[`tests/hk_login.rs`]'s
/// variant so this file stays a self-contained crate.
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

/// `GET /after` → `200 OK`. `login_completed` auto-follows redirects
/// (WPF L863 parity), so the 302 above needs a reachable target or
/// reqwest surfaces the 404 as `LoginError::Unknown`.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Clients + helpers
// -----------------------------------------------------------------------------

fn client_for_region(server: &MockServer, region: LoginRegion) -> BeanfunClient {
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

fn hk_client(server: &MockServer) -> BeanfunClient {
    client_for_region(server, LoginRegion::HK)
}

fn creds() -> Credentials {
    Credentials::new(ACCOUNT, PASSWORD)
}

/// Drive `login_hk_regular` until it returns `TotpRequired` and
/// unwrap the challenge. Fails the test loudly if any other outcome
/// surfaces ? the callers pre-mount the TOTP-form response so a
/// different branch would indicate a test-setup bug.
async fn obtain_challenge(client: &BeanfunClient) -> TotpChallenge {
    obtain_challenge_with_service(
        client,
        LoginRegion::HK.default_service_code(),
        LoginRegion::HK.default_service_region(),
    )
    .await
}

/// Variant of [`obtain_challenge`] that lets the caller pick the
/// `service_code` / `service_region` to capture on the resulting
/// `TotpChallenge`. Used by
/// `totp_custom_service_metadata_flows_to_session` to lock in the
/// audit fix that makes service metadata ride on the challenge
/// rather than on a hardcoded `region.default_*()` in `login_totp`.
async fn obtain_challenge_with_service(
    client: &BeanfunClient,
    service_code: &str,
    service_region: &str,
) -> TotpChallenge {
    let err = login_hk_regular(client, &creds(), service_code, service_region)
        .await
        .expect_err("HK Regular must redirect to TOTP challenge in this setup");
    match err {
        LoginError::TotpRequired(challenge) => *challenge,
        other => panic!("expected TotpRequired challenge, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Tests ? happy path and error branches
// -----------------------------------------------------------------------------

#[tokio::test]
async fn totp_happy_path_returns_session() {
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_redirects_with_akey(&server, AKEY).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;

    let session = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect("TOTP happy path must yield a Session");

    assert_eq!(session.region, LoginRegion::HK);
    assert_eq!(session.skey, SKEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
    assert_eq!(session.service_code, LoginRegion::HK.default_service_code());
    assert_eq!(
        session.service_region,
        LoginRegion::HK.default_service_region()
    );
}

#[tokio::test]
async fn totp_custom_service_metadata_flows_to_session() {
    // Audit regression guard for the chunk-3.3.3 fix: the service
    // metadata must ride through from `login_hk_regular`'s parameter
    // list ? captured on `TotpChallenge` ? consumed by `login_totp`
    // ? surfaced on the final `Session`.
    //
    // A regression where `login_totp` falls back to
    // `client.config().region.default_service_code()` would fail
    // the `service_code` assertion because the challenge carries a
    // slot the region would never default to.
    const CUSTOM_SERVICE_CODE: &str = "999999";
    const CUSTOM_SERVICE_REGION: &str = "TZ";

    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_redirects_with_akey(&server, AKEY).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = hk_client(&server);
    let challenge =
        obtain_challenge_with_service(&client, CUSTOM_SERVICE_CODE, CUSTOM_SERVICE_REGION).await;

    let session = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect("TOTP happy path with custom service metadata must yield a Session");

    assert_eq!(session.service_code, CUSTOM_SERVICE_CODE);
    assert_eq!(session.service_region, CUSTOM_SERVICE_REGION);
    // Sanity: other session fields should still carry through
    // unchanged ? the swap only targets service metadata.
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT);
}

#[tokio::test]
async fn totp_advance_check_returns_advance_check_required() {
    // TOTP POST replies with the RELOAD_CAPTCHA_CODE + alert page
    // (WPF `TotpLogin` L359-362).
    let body = "<script>if(window.RELOAD_CAPTCHA_CODE){alert('re-verify');}</script>";
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, body).await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;

    let err = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect_err("RELOAD_CAPTCHA_CODE + alert must trigger advance check");

    match err {
        LoginError::AdvanceCheckRequired { url: None } => {}
        other => panic!("expected AdvanceCheckRequired{{url:None}}, got {other:?}"),
    }
}

#[tokio::test]
async fn totp_msgbox_error_surfaces_server_message() {
    // WPF `TotpLogin` L368-375 ? MsgBox error body.
    let body = r#"<script type="text/javascript">$(function(){MsgBox.Show('OTP ??');});</script>"#;
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, body).await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;

    let err = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect_err("MsgBox body must surface as ServerMessage");

    match err {
        LoginError::ServerMessage(msg) => assert_eq!(msg, "OTP ??"),
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn totp_poll_request_surfaces_device_registration_required() {
    // Chunk 3.3.4 contract: the TOTP `pollRequest` branch ? WPF
    // `TotpLogin` L378-386 ? now surfaces
    // `LoginError::DeviceRegistrationRequired`, preserving the
    // triple `(login_token, poll_url, param)` captured from the
    // server's `pollRequest(...)` script so the caller can drive
    // `login_registered_device`.
    let body = r#"<div>pollRequest("/poll/ashx","TOK_TOTP","extra");</div>"#;
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, body).await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;

    let err = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect_err("pollRequest body must surface as DeviceRegistrationRequired");

    match err {
        LoginError::DeviceRegistrationRequired {
            login_token,
            poll_url,
            param,
        } => {
            assert_eq!(login_token, "TOK_TOTP");
            assert_eq!(poll_url, "/poll/ashx");
            assert_eq!(param, "extra");
        }
        other => panic!("expected DeviceRegistrationRequired, got {other:?}"),
    }
}

#[tokio::test]
async fn totp_unrecognised_body_no_akey_returns_missing_akey() {
    let body = "<html><body>completely unrelated content</body></html>";
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, body).await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;

    let err = login_totp(
        &client, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await
    .expect_err("unrecognized body must surface as MissingAkey");

    assert!(
        matches!(err, LoginError::MissingAkey),
        "expected MissingAkey, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Wire-shape tests ? per-region `__VIEWSTATEENCRYPTED` gating
// -----------------------------------------------------------------------------

/// Extract the recorded TOTP POST body from the wiremock server.
/// Panics if no POST to the TOTP endpoint was captured (indicates a
/// test-setup bug, not a product bug).
async fn recorded_totp_post_body(server: &MockServer) -> String {
    let requests = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    let totp_post = requests
        .iter()
        .find(|req| {
            req.method.as_str() == "POST"
                && req.url.path() == "/login/id-pass_form_newBF.aspx"
                && std::str::from_utf8(&req.body)
                    .map(|s| s.contains("otpCode1="))
                    .unwrap_or(false)
        })
        .expect("at least one TOTP POST must have been captured");
    String::from_utf8(totp_post.body.clone()).expect("TOTP POST body must be UTF-8")
}

#[tokio::test]
async fn totp_hk_post_body_has_six_otps_and_viewstate_encrypted() {
    // Drive the full HK-side flow and let the TOTP POST bounce to
    // an unrecognised body ? we only care about the recorded
    // request body here, not the outcome.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, "irrelevant").await;

    let client = hk_client(&server);
    let challenge = obtain_challenge(&client).await;
    let _ = login_totp(
        &client, &challenge, "100", "200", "300", "400", "500", "600",
    )
    .await;

    let body = recorded_totp_post_body(&server).await;

    // All six OTPs round-trip onto otpCode1..6 with their literal
    // value ? URL-encoded for `=` is `%3D`, but our values contain
    // no reserved chars, so they survive verbatim.
    for (i, value) in ["100", "200", "300", "400", "500", "600"]
        .iter()
        .enumerate()
    {
        let key = format!("otpCode{}", i + 1);
        assert!(
            body.contains(&format!("{key}={value}")),
            "body must contain {key}={value}: {body}"
        );
    }
    // Viewstate trio all forwarded.
    assert!(
        body.contains(&format!("__VIEWSTATE={VIEWSTATE}")),
        "body must contain viewstate: {body}"
    );
    assert!(
        body.contains(&format!("__VIEWSTATEGENERATOR={VIEWSTATE_GEN}")),
        "body must contain viewstate generator: {body}"
    );
    assert!(
        body.contains(&format!("__EVENTVALIDATION={EVENT_VALIDATION}")),
        "body must contain event validation: {body}"
    );
    // HK-only empty `__VIEWSTATEENCRYPTED` field ? WPF L347-348.
    assert!(
        body.contains("__VIEWSTATEENCRYPTED="),
        "HK TOTP body must contain __VIEWSTATEENCRYPTED= (WPF L347-348): {body}"
    );
    // Submit button value ? the CJK `??` URL-encoded as
    // %E7%99%BB%E5%85%A5.
    assert!(
        body.contains("totpLoginBtn=%E7%99%BB%E5%85%A5"),
        "body must contain totpLoginBtn=?? (URL-encoded): {body}"
    );
    // Defensive: HK Regular fields must NOT leak into the TOTP body.
    assert!(
        !body.contains("t_AccountID="),
        "TOTP body must not contain t_AccountID= (that's the HK Regular payload): {body}"
    );
    assert!(
        !body.contains("t_Password="),
        "TOTP body must not contain t_Password=: {body}"
    );
    assert!(
        !body.contains("btn_login="),
        "TOTP body must not contain btn_login= (that's the HK Regular button): {body}"
    );
}

#[tokio::test]
async fn totp_tw_post_body_drops_viewstate_encrypted() {
    // Controlled setup: the challenge is produced by an HK client
    // (there's no TW TOTP producer yet), but we hand it to a TW
    // client for the TOTP submission. The region-conditional branch
    // in `build_totp_form` reads from `client.config().region`, so
    // the TW client's POST must omit `__VIEWSTATEENCRYPTED`.
    let server = MockServer::start().await;
    mount_session_key(&server).await;
    mount_hk_login_page(&server).await;
    mount_hk_credentials_post_returns_totp_form(&server).await;
    mount_totp_post_with_body(&server, "irrelevant").await;

    let hk = hk_client(&server);
    let challenge = obtain_challenge(&hk).await;

    let tw = client_for_region(&server, LoginRegion::TW);
    let _ = login_totp(
        &tw, &challenge, OTPS[0], OTPS[1], OTPS[2], OTPS[3], OTPS[4], OTPS[5],
    )
    .await;

    let body = recorded_totp_post_body(&server).await;

    assert!(
        !body.contains("__VIEWSTATEENCRYPTED"),
        "TW TOTP body must NOT contain __VIEWSTATEENCRYPTED (WPF L347-348 \
         gates on App.LoginRegion == \"HK\"): {body}"
    );
    // Sanity ? the TOTP core fields still made it onto the wire.
    assert!(
        body.contains("otpCode1="),
        "TW TOTP body must still contain otpCode1=: {body}"
    );
    assert!(
        body.contains("totpLoginBtn=%E7%99%BB%E5%85%A5"),
        "TW TOTP body must still contain totpLoginBtn=??: {body}"
    );
}
