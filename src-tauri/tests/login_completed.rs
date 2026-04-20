//! End-to-end integration tests for the shared login-tail step
//! (`login/completed.rs`).
//!
//! This flow ports WPF `LoginCompleted` (L838-882) — the fixed
//! five-field POST to `return.aspx` that every non-QR login funnels
//! through. The unit tests in the source module already lock down the
//! form shape; this file covers:
//!
//! - the request is sent to `portal_base/beanfun_block/bflogin/return.aspx`
//!   (not `login_base`) with the exact WPF field values,
//! - the `bfWebToken` cookie that reqwest records in the shared
//!   cookie jar during the auto-followed redirect chain lands on
//!   `Session.web_token` (WPF parity with L863-868's
//!   `UploadString` → auto-follow → `GetCookie("bfWebToken")`),
//! - the region / account metadata arguments end up on `Session`
//!   verbatim (no silent rewriting),
//! - `MissingWebToken` surfaces when the cookie jar has no
//!   `bfWebToken` after the redirect chain settles.
//!
//! Because `login_completed` follows redirects (WPF parity), every
//! `return.aspx` mock here also mounts a `GET /after` landing page
//! so reqwest's auto-follow has somewhere to land without surfacing
//! a 404 as `LoginError::Unknown`. See [`mount_after_landing`].

use beanfun_lib::services::beanfun::{
    login::login_completed, BeanfunClient, ClientConfig, Endpoints, LoginError, LoginRegion,
};
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION_KEY: &str = "SKEY_COMPLETED";
const AKEY: &str = "AKEY_COMPLETED";
const ACCOUNT_ID: &str = "alice";
const SERVICE_CODE: &str = "610074";
const SERVICE_REGION: &str = "T9";
const WEB_TOKEN: &str = "BFWT_completed_happy";

/// Mount a `return.aspx` mock that echoes back a `bfWebToken` cookie on
/// a 302 response — matching the shape WPF's reference server uses.
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

/// Mount a `return.aspx` mock that intentionally omits the `bfWebToken`
/// cookie, so we can prove `MissingWebToken` propagates.
async fn mount_return_aspx_without_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str()),
        )
        .mount(server)
        .await;
    mount_after_landing(server).await;
}

/// `GET /after` → `200 OK` landing page. Required because
/// `login_completed` follows 302s via [`BeanfunClient::http`] (WPF
/// parity with L863's `UploadString` auto-follow), so the mocked 302
/// above points to `/after` which must exist or reqwest surfaces the
/// 404 as `LoginError::Unknown`. The body is intentionally empty —
/// `login_completed` discards the response body, it only cares about
/// the cookie jar state after the chain settles.
async fn mount_after_landing(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(server)
        .await;
}

/// Build a [`BeanfunClient`] whose portal_base / login_base / newlogin_base
/// all point at `server`. The region is a parameter because
/// `login_completed` reads it from `client.config().region` to stamp
/// the resulting `Session`.
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

// -----------------------------------------------------------------------------
// Happy path
// -----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_tw_returns_session_with_web_token() {
    let server = MockServer::start().await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server, LoginRegion::TW);
    let session = login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .expect("happy path must succeed");

    assert_eq!(session.region, LoginRegion::TW);
    assert_eq!(session.skey, SESSION_KEY);
    assert_eq!(session.web_token, WEB_TOKEN);
    assert_eq!(session.account_id, ACCOUNT_ID);
    assert_eq!(session.service_code, SERVICE_CODE);
    assert_eq!(session.service_region, SERVICE_REGION);
}

#[tokio::test]
async fn happy_path_hk_stamps_hk_region_on_session() {
    // The only HK-specific behaviour of `login_completed` is that the
    // region on the resulting Session is HK — the wire shape itself is
    // region-agnostic (both TW and HK POST to the same
    // `beanfun_block/bflogin/return.aspx` path, just on different
    // `portal_base` hosts).
    let server = MockServer::start().await;
    mount_return_aspx_with_token(&server, WEB_TOKEN).await;

    let client = client_for(&server, LoginRegion::HK);
    let session = login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .expect("HK happy path must succeed");

    assert_eq!(
        session.region,
        LoginRegion::HK,
        "region stamped on Session should come from client.config().region"
    );
}

// -----------------------------------------------------------------------------
// Wire-shape verification
// -----------------------------------------------------------------------------

#[tokio::test]
async fn post_body_contains_session_key_and_akey_url_encoded() {
    // Confirm the exact values land on the wire. `.form(&Vec<…>)` in
    // reqwest emits `application/x-www-form-urlencoded`, so checking
    // the raw substring is a fair proxy for "the server sees this
    // field=value pair".
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        // Field order is enforced by the unit test in the source
        // module; here we only assert that the key=value pairs appear.
        .and(body_string_contains("SessionKey=SKEY_COMPLETED"))
        .and(body_string_contains("AuthKey=AKEY_COMPLETED"))
        .and(body_string_contains("ServiceAccountSN=0"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str())
                .append_header(
                    "Set-Cookie",
                    format!("bfWebToken={WEB_TOKEN}; Path=/").as_str(),
                ),
        )
        .mount(&server)
        .await;
    mount_after_landing(&server).await;

    let client = client_for(&server, LoginRegion::TW);
    login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .expect("POST with correct body must succeed");
}

#[tokio::test]
async fn post_body_has_empty_service_code_and_region_fields() {
    // WPF L856-857 intentionally sends empty values for these fields,
    // even when it has real service codes in scope. The substring
    // `ServiceCode=&` nails down that the field is present AND empty
    // (as opposed to absent or carrying a value).
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .and(body_string_contains("ServiceCode=&"))
        .and(body_string_contains("ServiceRegion=&"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str())
                .append_header(
                    "Set-Cookie",
                    format!("bfWebToken={WEB_TOKEN}; Path=/").as_str(),
                ),
        )
        .mount(&server)
        .await;
    mount_after_landing(&server).await;

    let client = client_for(&server, LoginRegion::TW);
    login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        // Deliberately pass *non-empty* service codes to the Rust
        // function to confirm they land on `Session` but NOT on the
        // wire — mirroring WPF's behaviour.
        ACCOUNT_ID,
        "999999",
        "Z9",
    )
    .await
    .expect("POST with empty service-code wire values must succeed");
}

// -----------------------------------------------------------------------------
// Error propagation
// -----------------------------------------------------------------------------

#[tokio::test]
async fn missing_web_token_surfaces_login_error_variant() {
    let server = MockServer::start().await;
    mount_return_aspx_without_token(&server).await;

    let client = client_for(&server, LoginRegion::TW);
    let err = login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .expect_err("missing cookie must error");

    assert!(
        matches!(err, LoginError::MissingWebToken),
        "expected MissingWebToken, got {err:?}"
    );
}

// -----------------------------------------------------------------------------
// Redirect-chain cookie read (WPF L863-868 parity)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn bfwebtoken_set_on_late_redirect_hop_still_lands_on_session() {
    // WPF's `LoginCompleted` auto-follows redirects (L863 `UploadString`
    // + `WebClient`'s default `AllowAutoRedirect = true`) and then reads
    // `bfWebToken` from the *cookie jar* afterwards (L868
    // `GetCookie("bfWebToken")`). An earlier draft of this module
    // piggy-backed on `post_return_aspx` (no-redirect + immediate
    // `Set-Cookie` scrape), which caused a live regression on
    // 2026-04-16: beanfun's TW server set `bfWebToken` on a LATER
    // hop in the chain, invisible to the first-302 scrape.
    //
    // This test locks the fix: the initial 302 carries no
    // `bfWebToken`; the redirect target (`/after`) is where the cookie
    // is finally set. With auto-redirect + jar read we capture it.
    // With the old "scrape the first 302" strategy we would NOT, and
    // this test would fail with `MissingWebToken`.
    let server = MockServer::start().await;

    // First hop: 302 → /after, but NO `bfWebToken` Set-Cookie.
    Mock::given(method("POST"))
        .and(path("/beanfun_block/bflogin/return.aspx"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("Location", format!("{}/after", server.uri()).as_str()),
        )
        .mount(&server)
        .await;

    // Second hop (`/after`): 200 with the canonical `bfWebToken`.
    // Mirrors beanfun's observed traffic where the final landing
    // page is where the portal-scoped `bfWebToken` arrives.
    Mock::given(method("GET"))
        .and(path("/after"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header(
                    "Set-Cookie",
                    format!("bfWebToken={WEB_TOKEN}; Path=/; HttpOnly").as_str(),
                )
                .set_body_string(""),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, LoginRegion::TW);
    let session = login_completed(
        &client,
        SESSION_KEY,
        AKEY,
        ACCOUNT_ID,
        SERVICE_CODE,
        SERVICE_REGION,
    )
    .await
    .expect("late-hop cookie must still land on Session");

    assert_eq!(
        session.web_token, WEB_TOKEN,
        "cookie set on the final hop must be captured via the jar, \
         not just the first-302 Set-Cookie scrape"
    );
}
