//! Proxy discovery for GitHub API access — ports the WPF
//! `ApplicationUpdater.cs` `TryProbe` / `DiscoverProxy` / `GetProxy`
//! (L15-62) to async Rust.
//!
//! # Algorithm
//!
//! 1. `HEAD https://api.github.com` with a 5-second timeout. A strict
//!    2xx response means direct access works → return `""` (empty
//!    string, meaning "no proxy needed", same convention as WPF).
//! 2. Otherwise walk [`GH_PROXIES`] in order and try
//!    `HEAD {proxy}https://api.github.com` against each. First proxy to
//!    answer with a strict 2xx wins → return that proxy's prefix.
//! 3. If everything fails, return `""` and let the actual `releases`
//!    GET downstream fail with a clear network error — matches WPF
//!    L59 which also returns empty-string on total failure.
//!
//! # Why strict 2xx and not just "any successful-looking response"?
//!
//! WPF uses `req.GetResponse()` which throws `WebException` on 3xx/4xx/5xx
//! status codes; `TryProbe` wraps the call in `catch` so anything other
//! than 2xx is treated as a failure. We preserve the exact same
//! semantic via [`reqwest::Response::error_for_status`].
//!
//! Concretely, this matters for proxies that answer the probe URL with
//! a captive-portal / maintenance HTML page at status 200 (fine — we
//! accept and retry against real traffic), or with a 301 redirect
//! inside the proxy chain (bad — [`reqwest::Client`] with default
//! redirect policy would silently follow, so we'd get a 200 for the
//! wrong host; `error_for_status()` is safer than checking `is_success()`
//! on the final response because it short-circuits once we see any
//! non-2xx).
//!
//! # Caching
//!
//! [`proxy_probe`] stores the first-discovered proxy prefix in a static
//! [`OnceLock`] so the probe loop fires at most once per process
//! (mirroring WPF's `Lazy<string>` with
//! `LazyThreadSafetyMode.ExecutionAndPublication` at L24-27). A benign
//! race window exists where two concurrent `proxy_probe()` callers
//! both execute [`proxy_probe_at`] before either stores; this wastes
//! at most a handful of HEAD requests and both callers converge on the
//! same cached answer via `OnceLock::get_or_init`'s "first write wins"
//! semantic, so no further synchronisation is needed.
//!
//! # Testability
//!
//! [`proxy_probe_at`] takes the URLs as parameters so integration tests
//! can spin up wiremock servers on ephemeral ports and pass their URIs
//! directly — no environment variable dance and no global mutation.
//! See the unit tests at the bottom of this file for the pattern.

use std::sync::OnceLock;
use std::time::Duration;

use super::error::UpdaterError;

/// Upstream direct URL for GitHub's public API.
///
/// Used both as the probe target (HEAD-requested for a 2xx) and as the
/// URL proxies prepend to their own host — so the composed probe URL
/// for `proxy` becomes `{proxy}{DIRECT_URL}` verbatim.
pub const DIRECT_URL: &str = "https://api.github.com";

/// Ordered list of third-party GitHub-API proxies tried when the
/// direct probe fails. Matches WPF `GH_PROXIES` L15-20 byte-for-byte,
/// including the trailing slashes (which are load-bearing: the
/// composed probe URL is `{proxy}{DIRECT_URL}`, so dropping the slash
/// would produce invalid URLs like `https://ghproxy.viphttps://…`).
pub const GH_PROXIES: [&str; 3] = [
    "https://ghproxy.vip/",
    "https://ghproxy.net/",
    "https://ghfast.top/",
];

/// Per-HEAD timeout (5 s) — matches WPF `ProbeTimeoutMs = 5000` at
/// L22. Short enough that an unreachable proxy doesn't stall the whole
/// check loop; long enough to tolerate a slow TLS handshake on a
/// legitimately reachable endpoint.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Discover the best GitHub-API access path and cache the answer for
/// the rest of the process lifetime.
///
/// Returns `""` (direct access works) or a proxy prefix like
/// `"https://ghproxy.vip/"` (concatenate with a GitHub URL to route
/// through the proxy). After the first call the answer is served from
/// a static [`OnceLock`] — see the [module docs][self] for the race
/// semantics and why they are acceptable.
pub async fn proxy_probe() -> &'static str {
    static CACHED: OnceLock<String> = OnceLock::new();
    if let Some(cached) = CACHED.get() {
        return cached.as_str();
    }

    let discovered = proxy_probe_at(DIRECT_URL, &GH_PROXIES).await;
    CACHED.get_or_init(|| discovered).as_str()
}

/// Lower-level variant that takes the direct URL and proxy list as
/// parameters. Lets tests inject wiremock URIs while production code
/// calls [`proxy_probe`] with the hard-coded constants above.
///
/// Returns:
///
/// - `""` if the direct probe succeeds or everything fails (mirroring
///   WPF `DiscoverProxy` L48-50 and L59 respectively).
/// - The proxy prefix of the first proxy that answered with strict 2xx.
///
/// Never panics and never propagates transport errors — a failed
/// client-build or a failed HEAD is treated exactly like WPF's
/// `catch → return false`, i.e. "this path doesn't work, try the next".
pub async fn proxy_probe_at(direct_url: &str, proxies: &[&str]) -> String {
    let client = match build_probe_client() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    if head_probe_ok(&client, direct_url).await {
        return String::new();
    }

    for proxy in proxies {
        let probe_url = format!("{proxy}{direct_url}");
        if head_probe_ok(&client, &probe_url).await {
            return (*proxy).to_owned();
        }
    }

    String::new()
}

/// HEAD `url` with the probe client and collapse the result into
/// `true`/`false`. Strict 2xx only — see the [module docs][self] for
/// why.
async fn head_probe_ok(client: &reqwest::Client, url: &str) -> bool {
    match client.head(url).send().await {
        Ok(resp) => resp.error_for_status().is_ok(),
        Err(_) => false,
    }
}

/// Build a short-timeout reqwest client for the probe loop. Uses a
/// WPF-matching User-Agent (`"Beanfun(V{version})"`) so any server-side
/// allow-list that WPF was validated against continues to accept us,
/// and a 5-second total timeout per request so an unreachable proxy
/// can't stall the sequence.
fn build_probe_client() -> Result<reqwest::Client, UpdaterError> {
    reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .user_agent(probe_user_agent())
        .build()
        .map_err(UpdaterError::Probe)
}

/// User-Agent string the probe sends. Format mirrors WPF `TryProbe`
/// L36: `"Beanfun(V{App.AssemblyVersion})"`. We substitute the Cargo
/// package version so the string identifies us in upstream logs
/// without having to thread a runtime version through the call chain.
fn probe_user_agent() -> String {
    format!("Beanfun(V{})", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// `async` helper: register a HEAD handler on `server` that always
    /// responds with `status`.
    async fn mount_head(server: &MockServer, status: u16) {
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(status))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn proxy_probe_at_returns_empty_when_direct_url_responds_200() {
        let direct = MockServer::start().await;
        mount_head(&direct, 200).await;

        let got = proxy_probe_at(&format!("{}/", direct.uri()), &[]).await;
        assert_eq!(got, "");
    }

    #[tokio::test]
    async fn proxy_probe_at_picks_first_successful_proxy_when_direct_fails() {
        let direct = MockServer::start().await;
        mount_head(&direct, 500).await;

        let proxy_a = MockServer::start().await;
        mount_head(&proxy_a, 502).await;

        let proxy_b = MockServer::start().await;
        mount_head(&proxy_b, 200).await;

        let proxy_a_prefix = format!("{}/", proxy_a.uri());
        let proxy_b_prefix = format!("{}/", proxy_b.uri());
        let proxies: Vec<&str> = vec![proxy_a_prefix.as_str(), proxy_b_prefix.as_str()];

        let got = proxy_probe_at(&format!("{}/", direct.uri()), &proxies).await;
        assert_eq!(got, proxy_b_prefix);
    }

    #[tokio::test]
    async fn proxy_probe_at_returns_empty_when_everything_fails() {
        let direct = MockServer::start().await;
        mount_head(&direct, 503).await;

        let proxy_a = MockServer::start().await;
        mount_head(&proxy_a, 503).await;

        let proxy_a_prefix = format!("{}/", proxy_a.uri());
        let proxies: Vec<&str> = vec![proxy_a_prefix.as_str()];

        let got = proxy_probe_at(&format!("{}/", direct.uri()), &proxies).await;
        assert_eq!(got, "");
    }

    #[tokio::test]
    async fn proxy_probe_at_rejects_non_2xx_even_if_body_is_present() {
        // Anti-regression: a 404 with an HTML body should NOT be treated
        // as "probe succeeded". reqwest's response object would happily
        // have a status of 404 + a body; we must reject on
        // `error_for_status`.
        let direct = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(404).set_body_string("<html>not found</html>"))
            .mount(&direct)
            .await;

        let got = proxy_probe_at(&format!("{}/", direct.uri()), &[]).await;
        assert_eq!(got, "");
    }

    #[tokio::test]
    async fn proxy_probe_at_treats_transport_error_as_probe_failure() {
        // A non-listening URL — connect refused — must be treated as
        // failure, not propagate an error.
        let got = proxy_probe_at("http://127.0.0.1:1/", &["http://127.0.0.1:2/"]).await;
        assert_eq!(got, "");
    }

    #[test]
    fn gh_proxies_constant_matches_wpf_literal() {
        // Byte-for-byte audit: the WPF reference at L15-20 uses these
        // three proxies in this exact order, with trailing slashes.
        assert_eq!(
            GH_PROXIES,
            [
                "https://ghproxy.vip/",
                "https://ghproxy.net/",
                "https://ghfast.top/",
            ]
        );
    }

    #[test]
    fn direct_url_constant_matches_wpf_literal() {
        assert_eq!(DIRECT_URL, "https://api.github.com");
    }

    #[test]
    fn probe_timeout_matches_wpf_5000ms() {
        assert_eq!(PROBE_TIMEOUT, Duration::from_millis(5000));
    }

    #[test]
    fn probe_user_agent_matches_wpf_shape() {
        let ua = probe_user_agent();
        assert!(ua.starts_with("Beanfun(V"), "unexpected UA: {ua}");
        assert!(ua.ends_with(')'), "unexpected UA: {ua}");
        // Ensure the version chunk between the brackets is non-empty.
        let version = ua.trim_start_matches("Beanfun(V").trim_end_matches(')');
        assert!(!version.is_empty(), "empty version in UA: {ua}");
    }
}
