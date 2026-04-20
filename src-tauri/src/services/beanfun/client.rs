//! Wrapper around `reqwest::Client` that carries the per-session cookie
//! jar, region-aware endpoint list, and safety hardening (timeout, body
//! cap, UA, TLS).
//!
//! # Why two underlying clients?
//!
//! `reqwest::Client` sets the redirect policy at client construction time,
//! not per request. The WPF flow requires one specific call
//! (`POST return.aspx`) to **not** follow the 302 so we can grab the
//! `Set-Cookie: bfWebToken=…` header before it is swallowed by the redirect
//! hop. We therefore hold two clients that share the **same cookie store**:
//! a default "follow redirects" one for every normal call, and a
//! no-redirect one for the Set-Cookie-capturing call.
//!
//! # Cookie jar
//!
//! Cookies are stored in an [`reqwest_cookie_store::CookieStoreMutex`]
//! wrapped in `Arc`. Both underlying reqwest clients register the same
//! `Arc` as their `cookie_provider`, so cookies observed on one are
//! immediately visible to the other. The jar is therefore **per
//! [`BeanfunClient`] instance** — creating a second client is the supported
//! way to isolate two concurrent login sessions.
//!
//! # Safety hardening applied at construction
//!
//! - `.timeout(config.timeout)` — no request can hang forever (default 30s).
//! - `.user_agent(config.user_agent)` — matches the WPF string so the
//!   server accepts us.
//! - Default TLS backend is `rustls-tls` (configured in `Cargo.toml`);
//!   certificate validation is **always** on.
//! - [`BeanfunClient::bounded_text`] streams the response body chunk by
//!   chunk and bails once the running total exceeds
//!   `config.max_body_size`, preventing OOM from a malicious / stuck
//!   server.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Response;
use reqwest_cookie_store::CookieStoreMutex;
use url::Url;

use super::error::LoginError;

/// User-Agent string matching a real Chrome browser on Windows.
/// The HK portal (`bfweb.hk.beanfun.com`) performs a browser check and
/// redirects to a "browser not supported" page if the UA doesn't look
/// like a modern browser. The WPF client used a truncated UA that
/// happened to work, but the HK server has since tightened its check.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// Default per-request timeout. 30 s matches what a human expects before
/// they give up and hit the button again; long enough for the occasional
/// slow redirect, short enough that a stuck socket does not freeze the
/// UI.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default response-body cap. Beanfun's biggest legitimate response is the
/// game-server-account list page at a few hundred KB; 16 MiB leaves a
/// generous headroom while still preventing a runaway / hostile stream
/// from exhausting memory.
pub const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// Which region the login flow targets.
///
/// Cookies, portal URL, login host and even some response shapes differ
/// between the TW and HK endpoints, so the region is a first-class part of
/// the client configuration rather than a runtime flag on individual
/// calls.
///
/// # IPC exposure (P10.2 Q4=C hybrid — data-only path)
///
/// This enum is pure data (no secrets, no resources) so it rides the
/// Q4=A path: a [`serde::Serialize`] / [`serde::Deserialize`] /
/// [`specta::Type`] derive applied here lets the command layer
/// reference [`LoginRegion`] directly in DTOs (e.g.
/// `commands::dto::SessionInfo`) without needing a shadow type.
///
/// Serde represents the variants as their unit names — the frontend
/// sees a `"TW" | "HK"` union type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum LoginRegion {
    /// Taiwan — `tw.beanfun.com` portal, `login.beanfun.com` login host.
    TW,
    /// Hong Kong — `bfweb.hk.beanfun.com` portal, `login.hk.beanfun.com`
    /// login host.
    HK,
}

impl LoginRegion {
    /// Default service-code for MapleStory launches in this region
    /// (`"610074"` — matches the WPF `service_code` default across all
    /// `Login()` / `LoginCompleted()` call sites).
    pub const fn default_service_code(self) -> &'static str {
        "610074"
    }

    /// Default service-region for MapleStory launches in this region
    /// (`"T9"` for both TW and HK — matches WPF defaults).
    pub const fn default_service_region(self) -> &'static str {
        "T9"
    }
}

/// Set of base URLs used by one login flow.
///
/// Split out from [`ClientConfig`] so tests can swap in a wiremock server
/// (via [`Endpoints::custom`]) without having to set up a full config.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// Login host: `https://login.beanfun.com/` (TW) or
    /// `https://login.hk.beanfun.com/` (HK). Every `/Login/…` path joins
    /// onto this base.
    pub login_base: Url,
    /// Portal host: `https://tw.beanfun.com/` (TW) or
    /// `https://bfweb.hk.beanfun.com/` (HK). Holds `beanfun_block/bflogin`
    /// and the `return.aspx` redirect target.
    pub portal_base: Url,
    /// Auxiliary host used by the device-registration polling flow
    /// (`CheckIsRegisteDevice` / `bfAPPAutoLogin.ashx`) and the
    /// generic-handler logout endpoints.
    ///
    /// **Both regions point at `https://tw.newlogin.beanfun.com/`**
    /// because WPF `BeanfunClient.Login.cs::CheckIsRegisteDevice`
    /// L675-676 hardcodes that exact URL regardless of
    /// `App.LoginRegion`. The HK flow triggers the same polling
    /// endpoint when the server rendered a `pollRequest(...)` script
    /// on either the HK Regular or TOTP branch (L273-281 / L378-386),
    /// so HK must route the poll back to the TW newlogin host to
    /// match the WPF reference byte-for-byte.
    pub newlogin_base: Url,
}

impl Endpoints {
    /// Hardcoded production endpoints for the TW login flow.
    pub fn tw() -> Self {
        Self {
            login_base: Url::parse("https://login.beanfun.com/").expect("static URL"),
            portal_base: Url::parse("https://tw.beanfun.com/").expect("static URL"),
            newlogin_base: Url::parse("https://tw.newlogin.beanfun.com/").expect("static URL"),
        }
    }

    /// Hardcoded production endpoints for the HK login flow.
    ///
    /// `newlogin_base` intentionally points at the **TW** newlogin
    /// host — see the [`Endpoints::newlogin_base`] doc comment for
    /// why this is WPF-correct despite looking cross-region.
    pub fn hk() -> Self {
        Self {
            login_base: Url::parse("https://login.hk.beanfun.com/").expect("static URL"),
            portal_base: Url::parse("https://bfweb.hk.beanfun.com/").expect("static URL"),
            newlogin_base: Url::parse("https://tw.newlogin.beanfun.com/").expect("static URL"),
        }
    }

    /// Build an `Endpoints` from explicit base URLs — test-only escape
    /// hatch used by wiremock integration tests so we can route the three
    /// hosts onto one or more mock servers.
    pub fn custom(
        login_base: impl TryInto<Url, Error = url::ParseError>,
        portal_base: impl TryInto<Url, Error = url::ParseError>,
        newlogin_base: impl TryInto<Url, Error = url::ParseError>,
    ) -> Result<Self, LoginError> {
        Ok(Self {
            login_base: login_base
                .try_into()
                .map_err(|e| LoginError::InvalidUrl(format!("login_base: {e}")))?,
            portal_base: portal_base
                .try_into()
                .map_err(|e| LoginError::InvalidUrl(format!("portal_base: {e}")))?,
            newlogin_base: newlogin_base
                .try_into()
                .map_err(|e| LoginError::InvalidUrl(format!("newlogin_base: {e}")))?,
        })
    }
}

/// Fully-specified configuration for one [`BeanfunClient`] instance.
///
/// Construct via [`ClientConfig::for_region`] for production, or tweak
/// individual fields (e.g. `endpoints`, `timeout`) for tests.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Which region's flow we're targeting (drives a few behaviour
    /// branches inside the login orchestrator).
    pub region: LoginRegion,
    /// The base URLs the flow talks to.
    pub endpoints: Endpoints,
    /// Per-request timeout — applied to every `reqwest` call made through
    /// this client.
    pub timeout: Duration,
    /// Upper bound on the number of bytes we are willing to buffer from a
    /// single response body; see [`BeanfunClient::bounded_text`].
    pub max_body_size: usize,
    /// User-Agent header to send on every request. Matching the WPF string
    /// is what keeps the server happy.
    pub user_agent: String,
}

impl ClientConfig {
    /// Production config for `region` with all the defaults described in
    /// the module docs.
    pub fn for_region(region: LoginRegion) -> Self {
        let endpoints = match region {
            LoginRegion::TW => Endpoints::tw(),
            LoginRegion::HK => Endpoints::hk(),
        };
        Self {
            region,
            endpoints,
            timeout: DEFAULT_TIMEOUT,
            max_body_size: DEFAULT_MAX_BODY_SIZE,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }
}

impl Default for ClientConfig {
    /// Defaults to the TW production configuration.
    fn default() -> Self {
        Self::for_region(LoginRegion::TW)
    }
}

/// Beanfun HTTP session. Holds two reqwest clients (redirect / no-redirect)
/// sharing one cookie store, plus the config they were built from.
///
/// Cheap to clone: `reqwest::Client` is already `Arc`-based internally and
/// the cookie store is behind an `Arc<Mutex<_>>`. Two clones observe the
/// same cookie jar and are therefore the **same** logical session.
#[derive(Debug, Clone)]
pub struct BeanfunClient {
    http: reqwest::Client,
    http_no_redirect: reqwest::Client,
    cookie_store: Arc<CookieStoreMutex>,
    config: Arc<ClientConfig>,
}

impl BeanfunClient {
    /// Build a new client from `config`.
    ///
    /// Fails with [`LoginError::Http`] only when reqwest refuses to build
    /// the underlying client (e.g. if TLS init fails on the host), which
    /// in practice should never happen on a supported OS.
    pub fn new(config: ClientConfig) -> Result<Self, LoginError> {
        let cookie_store = Arc::new(CookieStoreMutex::default());
        let http = build_http_client(&config, Arc::clone(&cookie_store), true)?;
        let http_no_redirect = build_http_client(&config, Arc::clone(&cookie_store), false)?;

        Ok(Self {
            http,
            http_no_redirect,
            cookie_store,
            config: Arc::new(config),
        })
    }

    /// HTTP client that follows redirects — the default for every call.
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// HTTP client that does **not** follow redirects. Used for the single
    /// `return.aspx` POST where we need to read `Set-Cookie` on the 302
    /// response before the redirect hop drops the cookie.
    pub fn http_no_redirect(&self) -> &reqwest::Client {
        &self.http_no_redirect
    }

    /// Read-only view of the config this client was built with.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Shared reference to the cookie store.
    ///
    /// Most callers shouldn't need this — outbound requests pick up
    /// cookies from the jar automatically, and inbound `Set-Cookie`
    /// headers populate it automatically. The accessor exists as
    /// an escape hatch for the rare case that needs direct jar
    /// access without going through reqwest's request loop.
    ///
    /// Currently the only caller is `tests/login_then_logout.rs`,
    /// which inspects the jar after [`super::login::logout()`]
    /// returns to lock the deliberate "logout never clears the jar"
    /// policy — see the "Cookie jar" section in
    /// [`mod@super::login::logout`]'s module docs for the rationale.
    /// Future flows that need jar inspection (e.g. multi-session
    /// diagnostics) can use this same accessor.
    pub fn cookie_store(&self) -> Arc<CookieStoreMutex> {
        Arc::clone(&self.cookie_store)
    }

    /// Build a URL rooted at `endpoints.login_base`, e.g.
    /// `login_url("Login/Index")` →
    /// `https://login.beanfun.com/Login/Index`.
    pub(crate) fn login_url(&self, path: &str) -> Result<url::Url, LoginError> {
        self.config
            .endpoints
            .login_base
            .join(path)
            .map_err(|e| LoginError::InvalidUrl(format!("login URL `{path}`: {e}")))
    }

    /// Build a `login_base`-rooted URL with a `pSKey=…` query parameter
    /// appended, URL-encoding the value for us. The vast majority of
    /// login calls need this shape.
    pub(crate) fn login_url_with_skey(
        &self,
        path: &str,
        skey: &str,
    ) -> Result<url::Url, LoginError> {
        let mut url = self.login_url(path)?;
        url.query_pairs_mut().append_pair("pSKey", skey);
        Ok(url)
    }

    /// Build a URL rooted at `endpoints.portal_base`, e.g.
    /// `portal_url("beanfun_block/bflogin/return.aspx")` →
    /// `https://tw.beanfun.com/beanfun_block/bflogin/return.aspx`.
    pub(crate) fn portal_url(&self, path: &str) -> Result<url::Url, LoginError> {
        self.config
            .endpoints
            .portal_base
            .join(path)
            .map_err(|e| LoginError::InvalidUrl(format!("portal URL `{path}`: {e}")))
    }

    /// Build a URL rooted at `endpoints.newlogin_base`, e.g.
    /// `newlogin_url("generic_handlers/erase_token.ashx")` →
    /// `https://tw.newlogin.beanfun.com/generic_handlers/erase_token.ashx`.
    /// Mirrors the existing [`Self::login_url`] / [`Self::portal_url`]
    /// pattern; first user is the logout flow's `erase_token.ashx`
    /// POST and the TW-region `logout.aspx` GET.
    pub(crate) fn newlogin_url(&self, path: &str) -> Result<url::Url, LoginError> {
        self.config
            .endpoints
            .newlogin_base
            .join(path)
            .map_err(|e| LoginError::InvalidUrl(format!("newlogin URL `{path}`: {e}")))
    }

    /// Read `resp`'s body as UTF-8, capping the accumulated bytes at
    /// [`ClientConfig::max_body_size`].
    ///
    /// Streaming chunk-by-chunk (rather than calling `resp.text().await`)
    /// means we **cannot** OOM even if the server advertises a small
    /// `Content-Length` and then streams gigabytes of noise. We also
    /// honour a truthful `Content-Length` header as an early-abort hint.
    pub async fn bounded_text(&self, resp: Response) -> Result<String, LoginError> {
        let cap = self.config.max_body_size;

        if let Some(reported) = resp.content_length() {
            let reported = reported as usize;
            if reported > cap {
                return Err(LoginError::BodyTooLarge {
                    limit: cap,
                    actual: reported,
                });
            }
        }

        let mut resp = resp;
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            if buf.len().saturating_add(chunk.len()) > cap {
                return Err(LoginError::BodyTooLarge {
                    limit: cap,
                    actual: buf.len() + chunk.len(),
                });
            }
            buf.extend_from_slice(&chunk);
        }

        String::from_utf8(buf).map_err(|_| LoginError::InvalidUtf8)
    }
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Build a `reqwest::Client` sharing `cookie_store`.
///
/// `follow_redirects = true` yields the default `Policy::default()` (up to
/// ~10 hops); `false` yields `Policy::none()` for the Set-Cookie capture
/// site. All other settings (timeout, UA, cookies) are identical across
/// both clients so cookies observed on one flow transparently to the other.
fn build_http_client(
    config: &ClientConfig,
    cookie_store: Arc<CookieStoreMutex>,
    follow_redirects: bool,
) -> Result<reqwest::Client, LoginError> {
    let redirect_policy = if follow_redirects {
        reqwest::redirect::Policy::default()
    } else {
        reqwest::redirect::Policy::none()
    };

    reqwest::Client::builder()
        .cookie_provider(cookie_store)
        .timeout(config.timeout)
        .user_agent(&config.user_agent)
        .redirect(redirect_policy)
        .build()
        .map_err(LoginError::Http)
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_tw_has_production_urls() {
        let ep = Endpoints::tw();
        assert_eq!(ep.login_base.as_str(), "https://login.beanfun.com/");
        assert_eq!(ep.portal_base.as_str(), "https://tw.beanfun.com/");
        assert_eq!(
            ep.newlogin_base.as_str(),
            "https://tw.newlogin.beanfun.com/"
        );
    }

    #[test]
    fn endpoints_hk_has_production_urls() {
        let ep = Endpoints::hk();
        assert_eq!(ep.login_base.as_str(), "https://login.hk.beanfun.com/");
        assert_eq!(ep.portal_base.as_str(), "https://bfweb.hk.beanfun.com/");
        // WPF `CheckIsRegisteDevice` L675-676 hardcodes
        // tw.newlogin.beanfun.com even when App.LoginRegion == "HK",
        // so the HK endpoint set must route the device-poll host to
        // the TW newlogin server to preserve WPF byte-parity.
        assert_eq!(
            ep.newlogin_base.as_str(),
            "https://tw.newlogin.beanfun.com/"
        );
    }

    #[test]
    fn endpoints_custom_accepts_valid_urls() {
        let ep = Endpoints::custom(
            "http://127.0.0.1:8081/",
            "http://127.0.0.1:8082/",
            "http://127.0.0.1:8083/",
        )
        .expect("mock URLs must parse");
        assert_eq!(ep.login_base.as_str(), "http://127.0.0.1:8081/");
    }

    #[test]
    fn default_config_targets_tw() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.region, LoginRegion::TW);
        assert_eq!(cfg.user_agent, DEFAULT_USER_AGENT);
        assert_eq!(cfg.timeout, DEFAULT_TIMEOUT);
        assert_eq!(cfg.max_body_size, DEFAULT_MAX_BODY_SIZE);
    }

    #[test]
    fn client_constructs_and_exposes_config() {
        let client = BeanfunClient::new(ClientConfig::default()).expect("client builds");
        assert_eq!(client.config().region, LoginRegion::TW);
    }

    #[test]
    fn login_url_joins_onto_login_base() {
        let client = BeanfunClient::new(ClientConfig::default()).unwrap();
        let url = client.login_url("Login/Index").unwrap();
        assert_eq!(url.as_str(), "https://login.beanfun.com/Login/Index");
    }

    #[test]
    fn login_url_with_skey_url_encodes_value() {
        let client = BeanfunClient::new(ClientConfig::default()).unwrap();
        let url = client
            .login_url_with_skey("Login/Index", "A B/C=D")
            .unwrap();
        // url crate encodes ` ` → `+`, `/` → `%2F`, `=` → `%3D` via
        // `form_urlencoded` semantics. We assert on the shape, not the
        // exact byte-for-byte, so a future url bump that switches `+` to
        // `%20` would not spuriously break the test.
        let encoded = url.query().unwrap();
        assert!(encoded.starts_with("pSKey="));
        assert!(!encoded.contains(' '), "space must be encoded: {encoded}");
        assert!(!encoded.contains('/'), "slash must be encoded: {encoded}");
    }

    #[test]
    fn portal_url_joins_onto_portal_base() {
        let client = BeanfunClient::new(ClientConfig::default()).unwrap();
        let url = client
            .portal_url("beanfun_block/bflogin/return.aspx")
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://tw.beanfun.com/beanfun_block/bflogin/return.aspx"
        );
    }

    #[test]
    fn newlogin_url_joins_onto_newlogin_base() {
        let client = BeanfunClient::new(ClientConfig::default()).unwrap();
        let url = client
            .newlogin_url("generic_handlers/erase_token.ashx")
            .unwrap();
        // Both TW and HK Endpoints point newlogin_base at the same TW
        // host — see the `Endpoints::newlogin_base` doc for why HK is
        // intentionally cross-region here.
        assert_eq!(
            url.as_str(),
            "https://tw.newlogin.beanfun.com/generic_handlers/erase_token.ashx"
        );
    }

    #[test]
    fn region_default_service_codes_match_wpf_constants() {
        // WPF Login.cs uses these exact string literals at every call site.
        assert_eq!(LoginRegion::TW.default_service_code(), "610074");
        assert_eq!(LoginRegion::TW.default_service_region(), "T9");
        assert_eq!(LoginRegion::HK.default_service_code(), "610074");
        assert_eq!(LoginRegion::HK.default_service_region(), "T9");
    }
}
