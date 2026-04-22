//! Integration tests for the session keep-alive ping endpoint
//! (`BeanfunClient::ping`), ported from the WPF `pingWorker`
//! keep-alive loop (`MainWindow.xaml.cs` L2322-2368 calling
//! `BeanfunClient.Ping()` at `bfClient.cs` L193-212).
//!
//! The Beanfun portal expires idle sessions server-side; WPF
//! sidesteps this by pinging `echo_token.ashx?webtoken=1` every 60 s
//! so the backend keeps the session warm. These tests pin the
//! wire-shape contract (method, path, query, region-host routing)
//! that [`crate::commands::auth::run_ping_loop`] depends on.
//!
//! | WPF parity detail                                        | Covered by                                       |
//! |----------------------------------------------------------|--------------------------------------------------|
//! | GET verb + `echo_token.ashx` path                        | `ping_issues_get_against_echo_token_path`        |
//! | `webtoken=1` query string (matches WPF URL)              | `ping_attaches_webtoken_one_query_param`         |
//! | 2xx response → `Ok(())`                                  | `ping_returns_ok_on_success_body_is_ignored`     |
//! | 5xx response → `LoginError::Http`                        | `ping_surfaces_http_error_on_5xx`                |
//! | TW routes through `portal_base`                          | `ping_tw_routes_through_portal_base`             |
//! | HK routes through `portal_base` (its own HK host)        | `ping_hk_routes_through_portal_base`             |

use beanfun_lib::services::beanfun::{
    BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

// -----------------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------------

/// Build a [`BeanfunClient`] whose `portal_base` points at `server`
/// so `/beanfun_block/generic_handlers/echo_token.ashx` requests
/// land on the mock. `login_base` / `newlogin_base` are aliased to
/// the same mock so an accidental mis-routing surfaces as an
/// assertion failure on `server.received_requests` (whatever path
/// was hit would still be visible in the log) rather than a
/// connection refused from the real host.
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

/// Three distinct mock servers so we can prove *which* base
/// `ping` routed through. If ping ever drifts onto `login_base` or
/// `newlogin_base` it would fail the tight `received_requests`
/// count assertion below.
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

const ECHO_TOKEN_PATH: &str = "/beanfun_block/generic_handlers/echo_token.ashx";

async fn mount_echo_token(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path(ECHO_TOKEN_PATH))
        .respond_with(ResponseTemplate::new(status).set_body_string("echo-ok"))
        .mount(server)
        .await;
}

// -----------------------------------------------------------------------------
// Wire-shape contract
// -----------------------------------------------------------------------------

#[tokio::test]
async fn ping_issues_get_against_echo_token_path() {
    let server = MockServer::start().await;
    mount_echo_token(&server, 200).await;

    let client = single_server_client(&server, LoginRegion::TW);
    client.ping().await.expect("ping succeeds against 200 mock");

    let requests = server.received_requests().await.expect("log is enabled");
    assert_eq!(
        requests.len(),
        1,
        "ping should issue exactly one request per call",
    );
    let req: &Request = &requests[0];
    assert_eq!(req.method.as_str(), "GET");
    assert_eq!(
        req.url.path(),
        ECHO_TOKEN_PATH,
        "path must match WPF `bfClient.Ping()` URL verbatim",
    );
}

#[tokio::test]
async fn ping_attaches_webtoken_one_query_param() {
    let server = MockServer::start().await;
    // `query_param` matcher will only succeed if the inbound URL
    // carries `webtoken=1` — pin the same query string WPF uses.
    Mock::given(method("GET"))
        .and(path(ECHO_TOKEN_PATH))
        .and(query_param("webtoken", "1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = single_server_client(&server, LoginRegion::TW);
    client.ping().await.expect("ping succeeds");
    // MockServer's Drop assertion (via `.expect(1)`) verifies the
    // `webtoken=1` matcher was hit exactly once.
}

// -----------------------------------------------------------------------------
// Result mapping
// -----------------------------------------------------------------------------

#[tokio::test]
async fn ping_returns_ok_on_success_body_is_ignored() {
    // WPF `BeanfunClient.Ping()` reads the body purely for the
    // `Console.WriteLine` debug trace; the *result* is always
    // best-effort. Our Rust port must not parse / bounds-check the
    // body either — a large response body must NOT surface as a
    // `LoginError::BodyTooLarge` because we're not calling
    // `bounded_text` on the ping path.
    let huge_body = "x".repeat(16 * 1024 * 1024);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ECHO_TOKEN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string(huge_body))
        .mount(&server)
        .await;

    let client = single_server_client(&server, LoginRegion::TW);
    client
        .ping()
        .await
        .expect("ping returns Ok even when body is huge");
}

#[tokio::test]
async fn ping_surfaces_http_error_on_5xx() {
    let server = MockServer::start().await;
    mount_echo_token(&server, 500).await;

    let client = single_server_client(&server, LoginRegion::TW);
    let err = client
        .ping()
        .await
        .expect_err("500 response must surface as LoginError::Http");

    assert!(
        matches!(err, LoginError::Http(_)),
        "5xx should map to LoginError::Http, got {err:?}",
    );
}

// -----------------------------------------------------------------------------
// Region routing
// -----------------------------------------------------------------------------

#[tokio::test]
async fn ping_tw_routes_through_portal_base() {
    let portal = MockServer::start().await;
    let login = MockServer::start().await;
    let newlogin = MockServer::start().await;
    mount_echo_token(&portal, 200).await;
    mount_echo_token(&login, 200).await;
    mount_echo_token(&newlogin, 200).await;

    let client = split_server_client(&portal, &login, &newlogin, LoginRegion::TW);
    client.ping().await.expect("ping succeeds");

    assert_eq!(
        portal.received_requests().await.expect("log enabled").len(),
        1,
        "TW ping must hit portal_base",
    );
    assert!(
        login
            .received_requests()
            .await
            .expect("log enabled")
            .is_empty(),
        "TW ping must not hit login_base",
    );
    assert!(
        newlogin
            .received_requests()
            .await
            .expect("log enabled")
            .is_empty(),
        "TW ping must not hit newlogin_base",
    );
}

#[tokio::test]
async fn ping_hk_routes_through_portal_base() {
    // WPF `bfClient.Ping()` uses `bfweb.hk.beanfun.com` for the HK
    // region. `portal_base` is exactly that host in
    // `Endpoints::hk`, so the HK ping path is simply "portal_base
    // with an HK URL" — same code path, different configured host.
    let portal = MockServer::start().await;
    let login = MockServer::start().await;
    let newlogin = MockServer::start().await;
    mount_echo_token(&portal, 200).await;
    mount_echo_token(&login, 200).await;
    mount_echo_token(&newlogin, 200).await;

    let client = split_server_client(&portal, &login, &newlogin, LoginRegion::HK);
    client.ping().await.expect("ping succeeds");

    assert_eq!(
        portal.received_requests().await.expect("log enabled").len(),
        1,
        "HK ping must hit portal_base (mapped to HK host at runtime)",
    );
    assert!(login
        .received_requests()
        .await
        .expect("log enabled")
        .is_empty(),);
    assert!(newlogin
        .received_requests()
        .await
        .expect("log enabled")
        .is_empty(),);
}
