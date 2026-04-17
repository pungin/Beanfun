//! End-to-end integration tests for `services::updater::check_update_at`.
//!
//! Scenarios covered (one test each, keyed to the P7 acceptance
//! checklist in `Todo.md`):
//!
//! 1. Direct probe OK + has newer version ⇒ UpdateInfo with no proxy
//! 2. Direct probe OK + up-to-date ⇒ `None`
//! 3. Direct fail → first proxy succeeds ⇒ fetch goes via that proxy
//! 4. First two proxies fail → third succeeds ⇒ fetch via third
//! 5. Every probe target fails ⇒ `None` (fetch attempt against direct
//!    URL still 500s so fallback is `None`-safe)
//! 6. Stable channel skips prerelease releases
//! 7. Beta channel returns prerelease releases
//! 8. Pre-5.8-style local version (`"5.7.0(2503010000)"`) compared via
//!    Path A — newer remote wins end-to-end
//!
//! # Why a dedicated integration file?
//!
//! The in-crate `cfg(test)` tests in `checker.rs` already exercise the
//! single-server happy path, but cannot ergonomically spin up multiple
//! wiremock servers or model proxy fallback — that would muddle the
//! unit-test scope. These integration tests sit in `tests/` where each
//! scenario gets its own process-level dependency graph with several
//! [`wiremock::MockServer`] instances and exercises the full
//! [`check_update_at`] pipeline in a more realistic shape.

use beanfun_next_lib::services::updater::{check_update_at, Channel};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Seed a release-list JSON body given a newest-first list of
/// `(tag_name, prerelease, has_asset)` tuples. Keeps the test data
/// compact without introducing a `format!`-per-test boilerplate.
fn releases_json(entries: &[(&str, bool, bool)]) -> String {
    let mut json = String::from("[");
    for (i, (tag, prerelease, has_asset)) in entries.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let assets = if *has_asset {
            format!(
                r#"[{{"browser_download_url":"https://github.com/pungin/Beanfun/releases/download/{tag}/Setup.exe"}}]"#
            )
        } else {
            "[]".to_owned()
        };
        json.push_str(&format!(
            r#"{{"tag_name":"{tag}","prerelease":{prerelease},"body":"release {tag}","assets":{assets}}}"#
        ));
    }
    json.push(']');
    json
}

/// Mount a HEAD responder on `server` that returns `status` for every
/// path. Used to simulate "proxy alive" (200) or "proxy dead" (500).
async fn mount_probe(server: &MockServer, status: u16) {
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

/// Mount a GET responder that replies with `body` as 200 JSON, for any
/// path. Pairs with [`mount_probe`] when the same server needs to play
/// both probe target and release-feed host in proxy-scenario tests.
async fn mount_releases_catchall(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Dummy `api_releases_url` used when the fetch actually goes through
/// a proxy. The string itself is never DNS-resolved — only
/// prefixed by the proxy URL and matched path-wise by the proxy mock
/// via `mount_releases_catchall` — so a `.invalid` host is the
/// lowest-risk placeholder.
const DUMMY_API_URL: &str = "https://api.github.com.invalid/releases";

#[tokio::test]
async fn direct_ok_with_newer_release_returns_update_info_without_proxy() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 200).await;
    mount_releases_catchall(
        &direct,
        &releases_json(&[("v5.8.4.2604020000", false, true)]),
    )
    .await;

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[],
        &format!("{}/releases", direct.uri()),
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await
    .expect("newer release must be detected");

    assert_eq!(got.tag_name, "v5.8.4.2604020000");
    assert_eq!(got.new_version_display, "5.8.4(2604020000)");
    assert_eq!(
        got.download_url,
        "https://github.com/pungin/Beanfun/releases/download/v5.8.4.2604020000/Setup.exe",
        "direct probe → empty proxy prefix → asset URL unchanged"
    );
}

#[tokio::test]
async fn direct_ok_with_no_newer_release_returns_none() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 200).await;
    mount_releases_catchall(
        &direct,
        &releases_json(&[("v5.8.3.2604011114", false, true)]),
    )
    .await;

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[],
        &format!("{}/releases", direct.uri()),
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await;

    assert!(
        got.is_none(),
        "identical version must be reported as up-to-date"
    );
}

#[tokio::test]
async fn direct_fail_falls_through_to_first_proxy_and_fetches_through_it() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 500).await;

    // proxy: probe 200 + releases JSON on any GET path (fetch URL will
    // be `{proxy}{DUMMY_API_URL}`, which hits this server with path
    // `/https://api.github.com.invalid/releases`).
    let proxy = MockServer::start().await;
    mount_probe(&proxy, 200).await;
    mount_releases_catchall(
        &proxy,
        &releases_json(&[("v5.8.4.2604020000", false, true)]),
    )
    .await;

    let proxy_prefix = format!("{}/", proxy.uri());
    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[proxy_prefix.as_str()],
        DUMMY_API_URL,
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await
    .expect("proxy fallback must produce UpdateInfo");

    assert_eq!(got.tag_name, "v5.8.4.2604020000");
    // The asset download URL must gain the proxy prefix — WPF L171.
    assert_eq!(
        got.download_url,
        format!(
            "{proxy_prefix}https://github.com/pungin/Beanfun/releases/download/v5.8.4.2604020000/Setup.exe"
        ),
    );
}

#[tokio::test]
async fn first_two_proxies_fail_then_third_succeeds() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 502).await;

    let bad_a = MockServer::start().await;
    mount_probe(&bad_a, 502).await;

    let bad_b = MockServer::start().await;
    mount_probe(&bad_b, 504).await;

    let good = MockServer::start().await;
    mount_probe(&good, 200).await;
    mount_releases_catchall(&good, &releases_json(&[("v5.9.0.2604030000", false, true)])).await;

    let bad_a_prefix = format!("{}/", bad_a.uri());
    let bad_b_prefix = format!("{}/", bad_b.uri());
    let good_prefix = format!("{}/", good.uri());

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[
            bad_a_prefix.as_str(),
            bad_b_prefix.as_str(),
            good_prefix.as_str(),
        ],
        DUMMY_API_URL,
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await
    .expect("third proxy must be selected and used");

    assert_eq!(got.tag_name, "v5.9.0.2604030000");
    // Asset URL must be prefixed by the GOOD proxy, not one of the bad
    // ones — order-preservation of the proxy walk matters.
    assert!(
        got.download_url.starts_with(&good_prefix),
        "download_url must start with the working proxy's prefix, got: {}",
        got.download_url
    );
    assert!(!got.download_url.starts_with(&bad_a_prefix));
    assert!(!got.download_url.starts_with(&bad_b_prefix));
}

#[tokio::test]
async fn all_probes_fail_returns_none() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 503).await;
    // Fetch will still fire against `{empty}{fetch_url}` = `fetch_url`
    // (per proxy_probe_at's "all failed ⇒ empty prefix" convention),
    // so ensure the direct-URL's GET also fails to lock the downstream
    // None.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&direct)
        .await;

    let bad = MockServer::start().await;
    mount_probe(&bad, 503).await;

    let bad_prefix = format!("{}/", bad.uri());
    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[bad_prefix.as_str()],
        &format!("{}/releases", direct.uri()),
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await;

    assert!(got.is_none(), "every probe failing must collapse to None");
}

#[tokio::test]
async fn stable_channel_skips_prerelease_and_returns_first_stable() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 200).await;
    // Newest-first list: a prerelease v5.9 beta, then the latest
    // stable v5.8.4. Stable channel must skip the beta and pick v5.8.4.
    mount_releases_catchall(
        &direct,
        &releases_json(&[
            ("v5.9.0.2604030000", true, true),
            ("v5.8.4.2604020000", false, true),
        ]),
    )
    .await;

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[],
        &format!("{}/releases", direct.uri()),
        Channel::Stable,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await
    .expect("stable must find v5.8.4");

    assert_eq!(
        got.tag_name, "v5.8.4.2604020000",
        "Stable must skip prereleases even when they sort newer"
    );
}

#[tokio::test]
async fn beta_channel_picks_prerelease_when_newest() {
    let direct = MockServer::start().await;
    mount_probe(&direct, 200).await;
    mount_releases_catchall(
        &direct,
        &releases_json(&[
            ("v5.9.0.2604030000", true, true),
            ("v5.8.4.2604020000", false, true),
        ]),
    )
    .await;

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[],
        &format!("{}/releases", direct.uri()),
        Channel::Beta,
        "5.8.3(2604011114)",
        "test-ua",
    )
    .await
    .expect("beta must find v5.9.0 beta");

    assert_eq!(
        got.tag_name, "v5.9.0.2604030000",
        "Beta channel must return the newest release regardless of prerelease flag"
    );
}

#[tokio::test]
async fn pre_5_8_display_form_local_compares_correctly_against_new_timestamp_remote() {
    // Local = "5.7.0(2503010000)" — older display-form shape that
    // pre-dates WPF's P5.8 pivot to always emitting a patch digit.
    // Remote = v5.8.0.2604011114 (newer major.minor AND later
    // timestamp). Path A must pick up on the major/minor bump via
    // the packed u128 comparator.
    let direct = MockServer::start().await;
    mount_probe(&direct, 200).await;
    mount_releases_catchall(
        &direct,
        &releases_json(&[("v5.8.0.2604011114", false, true)]),
    )
    .await;

    let got = check_update_at(
        &format!("{}/", direct.uri()),
        &[],
        &format!("{}/releases", direct.uri()),
        Channel::Stable,
        "5.7.0(2503010000)",
        "test-ua",
    )
    .await
    .expect("pre-5.8 display form must still detect newer remote");

    assert_eq!(got.tag_name, "v5.8.0.2604011114");
}
