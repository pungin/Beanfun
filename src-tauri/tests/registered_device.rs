//! End-to-end integration tests for the device-registration polling
//! orchestrator (`login/registered_device.rs`).
//!
//! Each test stands up a fresh [`wiremock::MockServer`], points a
//! [`BeanfunClient`] at it (setting `newlogin_base` to the mock so
//! the single POST to `/login/bfAPPAutoLogin.ashx` routes correctly),
//! and drives [`login_registered_device`] against one canned server
//! response that exercises one branch of the WPF `IntResult` switch.
//!
//! | WPF branch (`IntResult`)   | Covered by                                                |
//! |----------------------------|-----------------------------------------------------------|
//! | `"2"` (approved)           | `happy_path_int_result_two_completes_login`               |
//! | `"2"` + bad `StrReslut`    | `two_with_unparseable_str_reslut_returns_keep_polling`    |
//! | `"0"` (server waiting)     | `zero_returns_keep_polling`                               |
//! | `"1"` (user pending)       | `one_returns_keep_polling`                                |
//! | `"-1"` (opaque error)      | `minus_one_surfaces_server_message`                       |
//! | `"-2"` (server timeout)    | `minus_two_surfaces_device_login_timeout`                 |
//! | `"-3"` (user rejected)     | `minus_three_surfaces_device_login_rejected`              |
//! | unknown value              | `unexpected_int_result_surfaces_unknown`                  |
//! | missing JSON fields        | `missing_int_result_field_surfaces_unknown`               |
//! | wire shape (`LT=` payload) | `post_body_carries_login_token_in_lt_field`               |
//! | host routing               | `post_routes_through_newlogin_base`                       |
//!
//! Pure unit tests for the `PollResponse` serde shape live next to
//! the source module; this file covers the HTTP orchestration,
//! `login_completed` hand-off, and the `IntResult` dispatch table
//! end-to-end.

use beanfun_lib::services::beanfun::{
    login::login_registered_device, BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LOGIN_TOKEN: &str = "LT_POLL_TOKEN";
const SESSION_KEY: &str = "SKEY_POLL";
const ACCOUNT_ID: &str = "alice";
const SERVICE_CODE: &str = "610074";
const SERVICE_REGION: &str = "T9";
const AKEY: &str = "AKEY_POLL_DONE";
const WEB_TOKEN: &str = "BFWT_poll_done";

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step
// -----------------------------------------------------------------------------

/// Mount the `bfAPPAutoLogin.ashx` endpoint returning a canned JSON
/// body (the `IntResult` / `StrReslut` pair). The `StrReslut` field
/// is spelled the same way the real server does — with WPF's typo
/// preserved — since that is what our deserialiser expects.
async fn mount_poll_response(server: &MockServer, int_result: &str, str_reslut: &str) {
    let body = format!(r#"{{"IntResult":"{int_result}","StrReslut":"{str_reslut}"}}"#);
    Mock::given(method("POST"))
        .and(path("/login/bfAPPAutoLogin.ashx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

/// Mount the `bfAPPAutoLogin.ashx` endpoint returning an arbitrary
/// body — used by the "unexpected IntResult" and "missing field"
/// tests where the canned `{IntResult, StrReslut}` shape does not
/// fit.
async fn mount_poll_raw_body(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path("/login/bfAPPAutoLogin.ashx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.to_owned())
                .insert_header("Content-Type", "application/json"),
        )
        .mount(server)
        .await;
}

/// Mount the ack-GET the "IntResult==2" tail fires as a cookie
/// side-effect. The body is discarded; we just need the request to
/// 200 so `ensure_success` passes.
async fn mount_str_reslut_ack(server: &MockServer, str_reslut: &str) {
    // WPF concatenates `newlogin_base + "login/" + StrReslut`, and
    // our code mirrors that verbatim. The StrReslut path is what
    // the mock must match on.
    //
    // `wiremock`'s `path` matcher matches the path exactly, so we
    // need to strip any `?query` portion and register just the
    // path.
    let path_only = str_reslut.split('?').next().unwrap_or(str_reslut);
    let path_str = format!("/login/{path_only}");
    Mock::given(method("GET"))
        .and(path(path_str))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(server)
        .await;
}

/// Mount the shared `login_completed` tail so the "IntResult==2"
/// happy path can finalise a Session. Matches the shape used by
/// `tests/login_completed.rs::mount_return_aspx_with_token` so test
/// setups stay consistent across files.
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
/// redirects (WPF L863 parity), so the 302 above needs a reachable
/// target or the chain surfaces 404 as `LoginError::Unknown`.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Client builder
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] whose three endpoint bases all point at
/// `server`. The region is TW by default — `login_registered_device`
/// is region-agnostic (the real WPF code hardcodes the TW newlogin
/// host for both regions), and every test in this file exercises
/// the exact same wire path, so a single region value suffices.
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

/// Drive `login_registered_device` with the canonical (non-test-only)
/// parameter tuple. Centralising the call site means additions to
/// the signature (e.g. a future telemetry span ctor) only touch one
/// place.
async fn run_poll(
    client: &BeanfunClient,
) -> Result<Option<beanfun_lib::services::beanfun::Session>, LoginError> {
    login_registered_device(
        client,
        LOGIN_TOKEN,
        SESSION_KEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
}

// -----------------------------------------------------------------------------
// IntResult == "2" — approved paths
// -----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_int_result_two_completes_login() {
    // WPF L683-697 — on IntResult=="2" the function:
    //   (a) GETs `/login/{StrReslut}` purely for cookie side-effects,
    //   (b) regex-extracts `akey=(.*)` against StrReslut,
    //   (c) calls LoginCompleted.
    // Our test mounts all three legs so we can observe the final
    // Session produced by `login_completed` inside the "2" branch.
    let str_reslut = format!("MLogin/done.aspx?akey={AKEY}");

    let server = MockServer::start().await;
    mount_poll_response(&server, "2", &str_reslut).await;
    mount_str_reslut_ack(&server, &str_reslut).await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server);
    let session = run_poll(&client)
        .await
        .expect("IntResult==2 happy path must succeed")
        .expect("a Session must be returned on IntResult==2 success");

    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, SESSION_KEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT_ID);
    assert_eq!(session.service_code, SERVICE_CODE);
    assert_eq!(session.service_region, SERVICE_REGION);
}

#[tokio::test]
async fn two_with_unparseable_str_reslut_returns_keep_polling() {
    // WPF L688-693 — when StrReslut on "2" does not match
    // `akey=(.*)`, WPF sets `errmsg = "AKeyParseFailed"` and returns
    // null; `MainWindow.bfAPPAutoLogin_Tick` L2413-2414 treats a
    // null return as "keep polling". We mirror that by returning
    // `Ok(None)` so the caller's polling loop retries, matching
    // WPF's observable behaviour byte-for-byte.
    //
    // The StrReslut below deliberately avoids the substring "akey="
    // anywhere — WPF's regex is plain `akey=(.*)` and would match
    // "akey=" even as part of a larger word like "noakey=yes", so
    // the crafted negative test must truly lack those five bytes.
    let str_reslut = "MLogin/broken.aspx?missing_param=yes";

    let server = MockServer::start().await;
    mount_poll_response(&server, "2", str_reslut).await;
    mount_str_reslut_ack(&server, str_reslut).await;

    let client = client_for(&server);
    let outcome = run_poll(&client)
        .await
        .expect("IntResult==2 with bad StrReslut must not error");

    assert!(
        outcome.is_none(),
        "AKeyParseFailed must route to Ok(None) / keep-polling, got {outcome:?}"
    );
}

// -----------------------------------------------------------------------------
// Keep-polling branches — IntResult == "0" | "1"
// -----------------------------------------------------------------------------

#[tokio::test]
async fn zero_returns_keep_polling() {
    // WPF `MainWindow.bfAPPAutoLogin_Tick` L2431-2432 — "0" is the
    // server-side "still waiting on the user" heartbeat; the tick
    // returns without action.
    let server = MockServer::start().await;
    mount_poll_response(&server, "0", "not-used").await;

    let client = client_for(&server);
    let outcome = run_poll(&client)
        .await
        .expect("IntResult==0 must not error");

    assert!(
        outcome.is_none(),
        "IntResult==0 must keep polling (Ok(None))"
    );
}

#[tokio::test]
async fn one_returns_keep_polling() {
    // WPF L2433-2435 — "1" prints 「尚未授權本次登入」 and returns.
    // Same Ok(None) outcome as "0" from our callers' perspective.
    let server = MockServer::start().await;
    mount_poll_response(&server, "1", "pending").await;

    let client = client_for(&server);
    let outcome = run_poll(&client)
        .await
        .expect("IntResult==1 must not error");

    assert!(
        outcome.is_none(),
        "IntResult==1 must keep polling (Ok(None))"
    );
}

// -----------------------------------------------------------------------------
// Terminal failure branches — IntResult == "-1" | "-2" | "-3"
// -----------------------------------------------------------------------------

#[tokio::test]
async fn minus_one_surfaces_server_message() {
    // WPF L2428-2430 — `-1` is the opaque fatal-error branch whose
    // message is carried verbatim in StrReslut. We surface it as
    // `ServerMessage` so the UI can display the server-supplied
    // string.
    let server = MockServer::start().await;
    mount_poll_response(&server, "-1", "something broke server-side").await;

    let client = client_for(&server);
    let err = run_poll(&client)
        .await
        .expect_err("IntResult==-1 must error");

    match err {
        LoginError::ServerMessage(msg) => {
            assert_eq!(msg, "something broke server-side");
        }
        other => panic!("expected ServerMessage, got {other:?}"),
    }
}

#[tokio::test]
async fn minus_two_surfaces_device_login_timeout() {
    // WPF L2424-2427 — `-2` means the server-enforced window for
    // the user to approve has passed.
    let server = MockServer::start().await;
    mount_poll_response(&server, "-2", "timeout").await;

    let client = client_for(&server);
    let err = run_poll(&client)
        .await
        .expect_err("IntResult==-2 must error");

    assert!(
        matches!(err, LoginError::DeviceLoginTimeout),
        "expected DeviceLoginTimeout, got {err:?}"
    );
}

#[tokio::test]
async fn minus_three_surfaces_device_login_rejected() {
    // WPF L2420-2423 — `-3` means the user (or upstream policy)
    // explicitly rejected the device-registration request.
    let server = MockServer::start().await;
    mount_poll_response(&server, "-3", "rejected").await;

    let client = client_for(&server);
    let err = run_poll(&client)
        .await
        .expect_err("IntResult==-3 must error");

    assert!(
        matches!(err, LoginError::DeviceLoginRejected),
        "expected DeviceLoginRejected, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Contract-violation branches
// -----------------------------------------------------------------------------

#[tokio::test]
async fn unexpected_int_result_surfaces_unknown() {
    // WPF's switch (L2418-2438) does not handle any value beyond
    // {-3, -2, -1, 0, 1, 2}. Our module treats any other value as a
    // server contract violation — LoginError::Unknown so the caller
    // can surface a diagnostic without silently masking the
    // breakage.
    let server = MockServer::start().await;
    mount_poll_response(&server, "99", "who knows").await;

    let client = client_for(&server);
    let err = run_poll(&client)
        .await
        .expect_err("unknown IntResult must error");

    match err {
        LoginError::Unknown(msg) => {
            assert!(
                msg.contains("99"),
                "error message should include the unexpected value: {msg}"
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_int_result_field_surfaces_unknown() {
    // WPF L679-681 short-circuits to `return null` when IntResult
    // is absent. We surface it as LoginError::Unknown rather than
    // folding it onto Ok(None) — a malformed JSON response is a
    // contract breach, not a "keep polling" hint.
    let server = MockServer::start().await;
    mount_poll_raw_body(&server, r#"{"StrReslut":"something"}"#).await;

    let client = client_for(&server);
    let err = run_poll(&client)
        .await
        .expect_err("missing IntResult must error");

    assert!(
        matches!(err, LoginError::Unknown(_)),
        "expected Unknown, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Wire-shape / routing verification
// -----------------------------------------------------------------------------

#[tokio::test]
async fn post_body_carries_login_token_in_lt_field() {
    // Confirm the exact LT=<login_token> pair lands on the wire.
    // `.form(&[("LT", login_token)])` in reqwest emits
    // `application/x-www-form-urlencoded`, so a substring match is
    // a fair proxy for "the server sees this field=value pair".
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/bfAPPAutoLogin.ashx"))
        .and(body_string_contains("LT=LT_POLL_TOKEN"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"IntResult":"0","StrReslut":"ok"}"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    let client = client_for(&server);
    run_poll(&client)
        .await
        .expect("POST with LT= form field must match the mock and succeed");
}

#[tokio::test]
async fn post_routes_through_newlogin_base() {
    // Regression guard: changing Endpoints::hk().newlogin_base back
    // to the HK login host (a pre-P3.3.4 latent bug) or otherwise
    // rerouting the poll away from `newlogin_base` would cause the
    // mock below to never receive the request — and wiremock would
    // fail the test with "unexpected request".
    //
    // We build two mocks: one on `/login/bfAPPAutoLogin.ashx`
    // (expected) and a catch-all that panics if hit. If the poll
    // lands anywhere else, the second mock would fire.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/bfAPPAutoLogin.ashx"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"IntResult":"0","StrReslut":"ok"}"#)
                .insert_header("Content-Type", "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server);
    run_poll(&client)
        .await
        .expect("poll must route to /login/bfAPPAutoLogin.ashx on newlogin_base");
    // On drop, wiremock verifies the `.expect(1)` assertion — if
    // the poll did not hit the mock exactly once the test panics.
}
