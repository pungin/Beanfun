//! End-to-end integration tests for the logout flow
//! (`login/logout.rs`).
//!
//! Each test stands up one or more [`wiremock::MockServer`]s, points
//! a [`BeanfunClient`] at them, and drives [`logout`] against canned
//! response chains that exercise one branch of the WPF
//! `BeanfunClient.Logout()` flow (`BeanfunClient.Login.cs` L884-909).
//!
//! | WPF branch / wire-shape detail                                | Covered by                                                       |
//! |---------------------------------------------------------------|------------------------------------------------------------------|
//! | TW happy path → all 3 endpoints hit                           | `tw_happy_path_hits_all_three_steps`                             |
//! | HK happy path → 2 endpoints hit, erase_token skipped          | `hk_happy_path_hits_two_steps_and_skips_erase_token`             |
//! | step 2 wire shape — Region-correct host + service query param | `step2_logout_aspx_carries_service_999999_t0_query`              |
//! | step 3 wire shape — body `web_token=1` + form Content-Type    | `step3_erase_token_posts_web_token_one_with_form_content_type`   |
//! | step 1 fails → still attempts step 2 + step 3                 | `step1_failure_still_attempts_remaining_steps_and_returns_err`   |
//! | step 2 fails → still attempts step 3                          | `step2_failure_still_attempts_step3_and_returns_err`             |
//! | step 3 fails → returns the step 3 error                       | `step3_failure_returns_err`                                      |
//! | All 3 fail → returns FIRST error (root-cause)                 | `multi_step_failure_returns_first_error_not_last`                |
//! | TW step 2 host = newlogin_base                                | `tw_step2_routes_through_newlogin_base_host`                     |
//! | HK step 2 host = login_base                                   | `hk_step2_routes_through_login_base_host`                        |

use beanfun_lib::services::beanfun::{
    login::logout, BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// -----------------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] whose `login_base` / `portal_base` /
/// `newlogin_base` all point at one shared `server`. Used by tests
/// that don't need to distinguish which base a given request went
/// through (i.e. anything except the `*_step2_routes_through_*`
/// pair below).
fn single_server_client(server: &MockServer, region: LoginRegion) -> BeanfunClient {
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

/// Build a client whose three bases point at three different servers,
/// so per-base routing is observable via each server's
/// `received_requests` log. Used by the step-2 routing tests where
/// the whole point is to prove WPF's region-dependent host choice
/// is preserved.
fn split_server_client(
    portal_server: &MockServer,
    login_server: &MockServer,
    newlogin_server: &MockServer,
    region: LoginRegion,
) -> BeanfunClient {
    let url = |s: &MockServer| Url::parse(&format!("{}/", s.uri())).expect("mock URL parses");
    let endpoints = Endpoints {
        login_base: url(login_server),
        portal_base: url(portal_server),
        newlogin_base: url(newlogin_server),
    };
    let mut cfg = ClientConfig::for_region(region);
    cfg.endpoints = endpoints;
    BeanfunClient::new(cfg).expect("client builds")
}

// -----------------------------------------------------------------------------
// Mock setup helpers — one per protocol step, with status parameterised
// so failure-path tests can swap 200 → 500 in a single call site.
// -----------------------------------------------------------------------------

async fn mount_step1(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path("/generic_handlers/remove_bflogin_session.ashx"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mount_step2(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path("/logout.aspx"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mount_step3(server: &MockServer, status: u16) {
    Mock::given(method("POST"))
        .and(path("/generic_handlers/erase_token.ashx"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Happy paths
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_happy_path_hits_all_three_steps() {
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 200).await;
    mount_step3(&server, 200).await;
    let client = single_server_client(&server, LoginRegion::TW);

    logout(&client).await.expect("TW happy path must succeed");

    let received = server.received_requests().await.expect("requests recorded");
    let paths: Vec<_> = received.iter().map(|r| r.url.path().to_owned()).collect();
    assert_eq!(
        paths,
        vec![
            "/generic_handlers/remove_bflogin_session.ashx".to_owned(),
            "/logout.aspx".to_owned(),
            "/generic_handlers/erase_token.ashx".to_owned(),
        ],
        "TW logout must hit all 3 endpoints in WPF L898/L899/L904 order"
    );
}

#[tokio::test]
async fn hk_happy_path_hits_two_steps_and_skips_erase_token() {
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 200).await;
    // Deliberately NOT mounting step 3 — if HK accidentally calls it,
    // the wiremock 404 would propagate as `LoginError::Unknown`. The
    // explicit `paths` assertion below belt-and-braces this.
    let client = single_server_client(&server, LoginRegion::HK);

    logout(&client).await.expect("HK happy path must succeed");

    let received = server.received_requests().await.expect("requests recorded");
    let paths: Vec<_> = received.iter().map(|r| r.url.path().to_owned()).collect();
    assert_eq!(
        paths,
        vec![
            "/generic_handlers/remove_bflogin_session.ashx".to_owned(),
            "/logout.aspx".to_owned(),
        ],
        "HK logout must hit only 2 endpoints (WPF L900 `if (App.LoginRegion == \"TW\")` skips step 3)"
    );
}

// -----------------------------------------------------------------------------
// Wire-shape assertions
// -----------------------------------------------------------------------------

fn header_value<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
    req.headers.get(name).and_then(|v| v.to_str().ok())
}

#[tokio::test]
async fn step2_logout_aspx_carries_service_999999_t0_query() {
    // WPF L899 hardcodes `?service=999999_T0` on the URL. Our
    // `query_pairs_mut().append_pair("service", "999999_T0")` should
    // round-trip to the same query string on the wire.
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 200).await;
    mount_step3(&server, 200).await;
    let client = single_server_client(&server, LoginRegion::TW);

    logout(&client).await.expect("happy roundtrip");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/logout.aspx")
        .expect("step 2 request was sent");

    assert_eq!(
        req.url.query(),
        Some("service=999999_T0"),
        "step 2 must carry the `service=999999_T0` sentinel WPF hardcodes"
    );
}

#[tokio::test]
async fn step3_erase_token_posts_web_token_one_with_form_content_type() {
    // WPF L902-907: `payload.Add("web_token", "1")` then
    // `UploadString(..., payload)`. `.NET`'s NameValueCollection +
    // UploadString URL-encodes as `web_token=1` and sets
    // Content-Type to `application/x-www-form-urlencoded`.
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 200).await;
    mount_step3(&server, 200).await;
    let client = single_server_client(&server, LoginRegion::TW);

    logout(&client).await.expect("happy roundtrip");

    let received = server.received_requests().await.expect("requests recorded");
    let req = received
        .iter()
        .find(|r| r.url.path() == "/generic_handlers/erase_token.ashx")
        .expect("step 3 request was sent");

    let body_str = std::str::from_utf8(&req.body).expect("form body is utf-8");
    assert_eq!(
        body_str, "web_token=1",
        "step 3 body must be exactly `web_token=1` (WPF L903 sentinel value)"
    );
    assert_eq!(
        header_value(req, "Content-Type"),
        Some("application/x-www-form-urlencoded"),
        "`.form()` must set the form Content-Type matching WPF UploadString",
    );
}

// -----------------------------------------------------------------------------
// Best-effort failure paths
// -----------------------------------------------------------------------------

#[tokio::test]
async fn step1_failure_still_attempts_remaining_steps_and_returns_err() {
    // Step 1 returns 500 → ensure_success collapses to LoginError::Unknown.
    // Per the best-effort policy (module docs), steps 2 and 3 must
    // STILL run; the returned error is the step-1 failure (first
    // encountered).
    let server = MockServer::start().await;
    mount_step1(&server, 500).await;
    mount_step2(&server, 200).await;
    mount_step3(&server, 200).await;
    let client = single_server_client(&server, LoginRegion::TW);

    let err = logout(&client).await.expect_err("step 1 5xx must surface");
    match err {
        LoginError::Unknown(msg) => assert!(
            msg.contains("remove_bflogin_session"),
            "first error must be the step 1 failure; got: {msg}"
        ),
        other => panic!("expected LoginError::Unknown, got {other:?}"),
    }

    // All 3 endpoints must still have been hit (best-effort).
    let received = server.received_requests().await.unwrap();
    let paths: Vec<_> = received.iter().map(|r| r.url.path().to_owned()).collect();
    assert!(
        paths.contains(&"/logout.aspx".to_owned()),
        "step 2 must run even after step 1 fails; got paths: {paths:?}"
    );
    assert!(
        paths.contains(&"/generic_handlers/erase_token.ashx".to_owned()),
        "step 3 must run even after step 1 fails; got paths: {paths:?}"
    );
}

#[tokio::test]
async fn step2_failure_still_attempts_step3_and_returns_err() {
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 500).await;
    mount_step3(&server, 200).await;
    let client = single_server_client(&server, LoginRegion::TW);

    let err = logout(&client).await.expect_err("step 2 5xx must surface");
    match err {
        LoginError::Unknown(msg) => assert!(
            msg.contains("logout.aspx"),
            "first error must be the step 2 failure; got: {msg}"
        ),
        other => panic!("expected LoginError::Unknown, got {other:?}"),
    }

    let received = server.received_requests().await.unwrap();
    assert!(
        received
            .iter()
            .any(|r| r.url.path() == "/generic_handlers/erase_token.ashx"),
        "step 3 must still run after step 2 fails (best-effort)"
    );
}

#[tokio::test]
async fn step3_failure_returns_err() {
    let server = MockServer::start().await;
    mount_step1(&server, 200).await;
    mount_step2(&server, 200).await;
    mount_step3(&server, 500).await;
    let client = single_server_client(&server, LoginRegion::TW);

    let err = logout(&client).await.expect_err("step 3 5xx must surface");
    match err {
        LoginError::Unknown(msg) => assert!(
            msg.contains("erase_token"),
            "error must mention the failing step (erase_token); got: {msg}"
        ),
        other => panic!("expected LoginError::Unknown, got {other:?}"),
    }
}

#[tokio::test]
async fn multi_step_failure_returns_first_error_not_last() {
    // All 3 steps fail with distinct status codes. Per the
    // first-error policy, the returned error must mention step 1
    // (`remove_bflogin_session`), not step 3 (`erase_token`). This is
    // the canonical lock for the "root cause is more diagnostic"
    // design choice.
    let server = MockServer::start().await;
    mount_step1(&server, 500).await;
    mount_step2(&server, 502).await;
    mount_step3(&server, 503).await;
    let client = single_server_client(&server, LoginRegion::TW);

    let err = logout(&client).await.expect_err("all steps fail");
    match err {
        LoginError::Unknown(msg) => {
            assert!(
                msg.contains("remove_bflogin_session"),
                "first-error policy: must surface step 1's error, got: {msg}"
            );
            assert!(
                !msg.contains("erase_token"),
                "first-error policy: must NOT surface step 3's later error, got: {msg}"
            );
        }
        other => panic!("expected LoginError::Unknown, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Region-dependent host routing for step 2
// -----------------------------------------------------------------------------

#[tokio::test]
async fn tw_step2_routes_through_newlogin_base_host() {
    // WPF L887-891: TW sets `loginHost = "tw.newlogin.beanfun.com"`,
    // which maps to our `newlogin_base`. We use three separate
    // mock servers so that "step 2 went to newlogin_base" is
    // observable as a request log on that server (and
    // simultaneously its absence on `login_base`).
    let portal = MockServer::start().await;
    let login = MockServer::start().await;
    let newlogin = MockServer::start().await;

    mount_step1(&portal, 200).await;
    // step 2 mounted only on `newlogin` — if logout incorrectly
    // routes through `login`, the `login` server returns 404 and
    // the test fails via the surfaced error.
    mount_step2(&newlogin, 200).await;
    mount_step3(&newlogin, 200).await;

    let client = split_server_client(&portal, &login, &newlogin, LoginRegion::TW);

    logout(&client).await.expect("TW routing must succeed");

    let newlogin_paths: Vec<_> = newlogin
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(
        newlogin_paths.contains(&"/logout.aspx".to_owned()),
        "TW step 2 must route through newlogin_base; got newlogin paths: {newlogin_paths:?}"
    );

    let login_paths: Vec<_> = login
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(
        login_paths.is_empty(),
        "TW logout must NOT touch login_base; got login paths: {login_paths:?}"
    );
}

#[tokio::test]
async fn hk_step2_routes_through_login_base_host() {
    // WPF L893-896: HK sets `loginHost = "login.hk.beanfun.com"`,
    // which maps to our `login_base`. Same routing-observability
    // setup as the TW test above, mirrored.
    let portal = MockServer::start().await;
    let login = MockServer::start().await;
    let newlogin = MockServer::start().await;

    mount_step1(&portal, 200).await;
    // step 2 mounted only on `login` — if HK incorrectly routes
    // through `newlogin`, the test fails. step 3 (TW-only) must
    // never run for HK so we mount it nowhere.
    mount_step2(&login, 200).await;

    let client = split_server_client(&portal, &login, &newlogin, LoginRegion::HK);

    logout(&client).await.expect("HK routing must succeed");

    let login_paths: Vec<_> = login
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(
        login_paths.contains(&"/logout.aspx".to_owned()),
        "HK step 2 must route through login_base; got login paths: {login_paths:?}"
    );

    let newlogin_paths: Vec<_> = newlogin
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(
        newlogin_paths.is_empty(),
        "HK logout must NOT touch newlogin_base (no step 3 for HK either); got newlogin paths: {newlogin_paths:?}"
    );
}
