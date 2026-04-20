//! GitHub Releases API client and release-selection policy.
//!
//! Ports the second half of WPF `ApplicationUpdater.cs`:
//!
//! - Release JSON schema (L64-86)
//! - `GetLastRelease` selection policy (L201-214)
//! - `DownloadData` call with GitHub-specific headers (L121-127)
//!
//! # Responsibilities
//!
//! | This module          | WPF reference                          |
//! | -------------------- | -------------------------------------- |
//! | [`GitHubRelease`]    | `class GitHubRelease` L64-80           |
//! | [`GitHubAsset`]      | `class GitHubAsset` L82-86             |
//! | [`Channel`]          | `updateChannel` string at L203         |
//! | [`fetch_releases_at`] | `client.DownloadData(url)` L125 + headers L123-124 |
//! | [`fetch_releases`]   | `var url = proxy + "https://…/releases"` L117 composition |
//! | [`select_release`]   | `GetLastRelease` L201-214              |
//!
//! # GitHub API headers
//!
//! WPF sends exactly these two headers on the releases GET:
//!
//! - `User-Agent: Beanfun(V{App.AssemblyVersion})` — required; GitHub
//!   rejects unidentified clients with a 403.
//! - `Accept: application/vnd.github.v3+json` — pins the response shape
//!   so a future API v4 migration can't silently change the JSON we
//!   parse.
//!
//! We preserve both verbatim. The `User-Agent` chunk substitutes the
//! Cargo package version because threading `App.AssemblyVersion` through
//! the call chain adds no value — the updater is a service-layer
//! concern, not an app-identity one, and any server-side allow-list
//! WPF was validated against cares about the `Beanfun(V…)` shape, not
//! the exact version digits.
//!
//! # Stable vs Beta
//!
//! WPF stores `updateChannel` in `Config.xml` and reads it via
//! `ConfigAppSettings.GetValue("updateChannel", "Stable")` (L203). The
//! selection policy (L206-213):
//!
//! - `"Beta"` or `"Preview"` → return the **first** release regardless
//!   of its `prerelease` flag (so a Beta user picking up the latest
//!   draft wins over a stable from last month).
//! - anything else → walk the list, return the first release with
//!   `prerelease == false`.
//!
//! We model this as a binary [`Channel`] enum rather than a free string
//! because the two-state semantic maps cleanly and we reject unknown
//! channel strings early at parse time instead of silently defaulting
//! deep inside `select_release`.

use serde::{Deserialize, Deserializer};

use super::error::UpdaterError;

fn null_as_empty<'de, D: Deserializer<'de>>(de: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(de).map(|o| o.unwrap_or_default())
}

/// One GitHub release as returned by
/// `https://api.github.com/repos/{owner}/{repo}/releases`.
///
/// Only the four fields WPF consumed (`name`, `tag_name`, `prerelease`,
/// `body`, `assets`) are modelled. Extra fields GitHub may add in the
/// future are silently ignored — `serde` defaults to allowing unknown
/// fields, which is what we want here so a new GitHub field doesn't
/// break deserialisation.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GitHubRelease {
    /// Human-readable title the maintainer set on the release page
    /// (e.g. "5.8.3 — critical login fix"). Not currently shown in
    /// the legacy UI but we keep it for parity and for future rich
    /// release-note rendering in the Vue shell.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub name: String,

    /// The git tag the release is attached to (e.g. `v5.8.3.2604011114`).
    /// This is what [`super::parse_tag`] regex-matches on.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub tag_name: String,

    /// `true` if the maintainer ticked "This is a pre-release" on the
    /// release page. [`select_release`] uses this to gate Stable-channel
    /// users from draft builds.
    #[serde(default)]
    pub prerelease: bool,

    /// Markdown body the maintainer wrote in the release description.
    /// WPF feeds this straight into the MessageBox; the Vue shell will
    /// render it as Markdown once P11 lands.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,

    /// Binary assets attached to the release (e.g. the setup EXE).
    /// [`Self::assets`].first() is the "the download link we hand the
    /// user" per WPF L169-172.
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

/// One binary asset attached to a [`GitHubRelease`]. Only the download
/// URL matters to the updater — the rest of GitHub's asset metadata
/// (size, download count, content type…) stays ignored.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GitHubAsset {
    /// Public HTTPS URL that serves the asset bytes. Clients prepend
    /// the proxy prefix (if any) before calling `Process.Start` on it —
    /// see WPF L169-172.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub browser_download_url: String,
}

/// Which release channel the user is subscribed to.
///
/// Mirrors WPF `updateChannel` config value. Binary rather than
/// three-state because `"Preview"` (WPF) aliases `"Beta"` via L204 —
/// the distinction never reached the selection logic, so preserving
/// it here would be noise.
///
/// # IPC exposure (P10.3 D4)
///
/// Derives [`serde::Serialize`] + [`serde::Deserialize`] +
/// [`specta::Type`] so the `check_update` Tauri command can accept
/// the channel choice from the frontend without a DTO wrapper.
/// Unit variants serialize as plain JSON strings (`"Stable"` /
/// `"Beta"`), matching the WPF `updateChannel` config value shape
/// so settings pages can bind to a single string without a decoder.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum Channel {
    /// Stable-only: skip releases with `prerelease == true`.
    Stable,
    /// Prerelease-inclusive: return whichever release is newest,
    /// draft or not. Matches WPF `"Beta"` and `"Preview"` both.
    Beta,
}

impl Channel {
    /// Parse a WPF-style `updateChannel` config string.
    ///
    /// `"Beta"` and `"Preview"` map to [`Self::Beta`] (preserving the
    /// WPF L204 `"Beta" || "Preview"` disjunction byte-for-byte),
    /// **anything else** (including `""`, `"Stable"`, or a typo) maps
    /// to [`Self::Stable`]. Mirrors WPF L203 default of
    /// `GetValue("updateChannel", "Stable")` + the L204 comparison
    /// chain.
    ///
    /// Case-sensitive on purpose: WPF uses `string.Equals(s)` (no
    /// `StringComparison.OrdinalIgnoreCase`) so `"beta"` (lowercase)
    /// would silently fall through to Stable, and we preserve that
    /// quirk for 1:1 parity.
    pub fn from_config_value(value: &str) -> Self {
        if value == "Beta" || value == "Preview" {
            Self::Beta
        } else {
            Self::Stable
        }
    }
}

impl Default for Channel {
    /// Stable is the WPF default when the config key is missing.
    fn default() -> Self {
        Self::Stable
    }
}

/// GitHub API endpoint that serves the release list.
///
/// Verbatim from WPF L117:
/// `"https://api.github.com/repos/pungin/beanfun/releases"`. Callers
/// prepend a proxy prefix (empty for direct access) before passing
/// this into [`fetch_releases_at`]; the proxy convention
/// (`{proxy}{full_url}`) matches [`mod@super::proxy_probe`]'s contract.
pub const GH_API_RELEASES_URL: &str = "https://api.github.com/repos/pungin/beanfun/releases";

/// `Accept` header the GitHub v3 API expects. Pinning the API version
/// insulates us from a future GitHub v4-default migration silently
/// changing the JSON shape.
pub const GITHUB_ACCEPT_HEADER: &str = "application/vnd.github.v3+json";

/// Fetch the repository's releases from an arbitrary base URL.
///
/// Decoupling the base URL from the production constant lets tests
/// spin up a wiremock server and pass its URI directly, without
/// touching globals or environment variables. [`fetch_releases`] is
/// the production-flavoured wrapper that plugs [`GH_API_RELEASES_URL`]
/// into this function.
///
/// `user_agent` is forwarded as the HTTP `User-Agent` header. GitHub
/// requires a non-empty UA and returns 403 otherwise, so missing it
/// would bubble up as [`UpdaterError::Fetch`] (which is the honest
/// answer — the request genuinely failed).
///
/// Returns [`UpdaterError::Fetch`] for any transport fault, 4xx/5xx
/// status (via [`reqwest::Response::error_for_status`]), or body-read
/// error. Returns [`UpdaterError::JsonDecode`] if the body came back
/// 2xx but `serde_json` rejected it — that separation lets tests tell
/// a transport failure apart from a malformed payload.
pub async fn fetch_releases_at(
    base_url: &str,
    user_agent: &str,
) -> Result<Vec<GitHubRelease>, UpdaterError> {
    let client = reqwest::Client::builder()
        .user_agent(user_agent)
        .build()
        .map_err(UpdaterError::Fetch)?;

    let response = client
        .get(base_url)
        .header(reqwest::header::ACCEPT, GITHUB_ACCEPT_HEADER)
        .send()
        .await
        .map_err(UpdaterError::Fetch)?
        .error_for_status()
        .map_err(UpdaterError::Fetch)?;

    let bytes = response.bytes().await.map_err(UpdaterError::Fetch)?;
    serde_json::from_slice(&bytes).map_err(UpdaterError::JsonDecode)
}

/// Production-flavoured wrapper around [`fetch_releases_at`]: plugs in
/// [`GH_API_RELEASES_URL`] prefixed with `proxy_prefix` (empty string
/// means direct access — mirrors [`mod@super::proxy_probe`]'s return
/// convention) and the WPF-compatible `User-Agent` string
/// `Beanfun(V{CARGO_PKG_VERSION})`.
///
/// Callers that need fine-grained control (tests, tools, future
/// diagnostics) should call [`fetch_releases_at`] directly.
pub async fn fetch_releases(proxy_prefix: &str) -> Result<Vec<GitHubRelease>, UpdaterError> {
    let url = format!("{proxy_prefix}{GH_API_RELEASES_URL}");
    let ua = format!("Beanfun(V{})", env!("CARGO_PKG_VERSION"));
    fetch_releases_at(&url, &ua).await
}

/// Pick the release a user on `channel` should see, given the list
/// GitHub returned (which is newest-first by convention).
///
/// Returns `None` iff `releases` is empty, or if the channel is
/// [`Channel::Stable`] and every release in the list is marked
/// prerelease — matches WPF L213 `return null`.
///
/// Complexity is O(n) in the worst case (Stable channel with every
/// release marked prerelease); average case is O(1) (latest release
/// is usually Stable).
pub fn select_release(releases: &[GitHubRelease], channel: Channel) -> Option<&GitHubRelease> {
    match channel {
        Channel::Beta => releases.first(),
        Channel::Stable => releases.iter().find(|r| !r.prerelease),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_release(tag: &str, prerelease: bool) -> GitHubRelease {
        GitHubRelease {
            name: format!("Release {tag}"),
            tag_name: tag.to_owned(),
            prerelease,
            body: String::new(),
            assets: Vec::new(),
        }
    }

    #[test]
    fn channel_from_config_value_maps_beta_and_preview_to_beta() {
        assert_eq!(Channel::from_config_value("Beta"), Channel::Beta);
        assert_eq!(Channel::from_config_value("Preview"), Channel::Beta);
    }

    #[test]
    fn channel_from_config_value_maps_stable_and_unknown_to_stable() {
        assert_eq!(Channel::from_config_value("Stable"), Channel::Stable);
        assert_eq!(Channel::from_config_value(""), Channel::Stable);
        assert_eq!(Channel::from_config_value("nonsense"), Channel::Stable);
    }

    #[test]
    fn channel_from_config_value_is_case_sensitive_matching_wpf() {
        // WPF uses `string.Equals` without `OrdinalIgnoreCase`, so
        // "beta" (lowercase) silently falls through to Stable. We
        // preserve the quirk for 1:1 parity — flipping to
        // case-insensitive would mean diverging from the WPF
        // reference, which should be an explicit decision.
        assert_eq!(Channel::from_config_value("beta"), Channel::Stable);
        assert_eq!(Channel::from_config_value("BETA"), Channel::Stable);
    }

    #[test]
    fn channel_default_is_stable() {
        assert_eq!(Channel::default(), Channel::Stable);
    }

    #[test]
    fn select_release_stable_skips_prereleases_and_returns_first_stable() {
        let releases = vec![
            make_release("v5.8.4.2604020000", true),
            make_release("v5.8.3.2604011114", false),
            make_release("v5.8.2.2604010000", false),
        ];
        let picked = select_release(&releases, Channel::Stable).expect("has stable");
        assert_eq!(picked.tag_name, "v5.8.3.2604011114");
    }

    #[test]
    fn select_release_beta_always_returns_first() {
        let releases = vec![
            make_release("v5.8.4.2604020000", true),
            make_release("v5.8.3.2604011114", false),
        ];
        let picked = select_release(&releases, Channel::Beta).expect("has release");
        assert_eq!(picked.tag_name, "v5.8.4.2604020000");
    }

    #[test]
    fn select_release_stable_returns_none_when_every_release_is_prerelease() {
        let releases = vec![
            make_release("v5.8.4.2604020000", true),
            make_release("v5.8.3.2604011114", true),
        ];
        assert!(select_release(&releases, Channel::Stable).is_none());
    }

    #[test]
    fn select_release_any_channel_returns_none_on_empty_list() {
        let releases: Vec<GitHubRelease> = Vec::new();
        assert!(select_release(&releases, Channel::Stable).is_none());
        assert!(select_release(&releases, Channel::Beta).is_none());
    }

    #[test]
    fn github_release_deserialises_from_real_api_shape() {
        // Spot-check against a real GitHub releases body (trimmed).
        // Asserts: extra fields are ignored, missing optional fields
        // default to sensible values, and the nested `assets` array
        // picks up via the top-level `#[serde(default)]`.
        let json = r#"[
            {
                "name": "5.8.3 — login fix",
                "tag_name": "v5.8.3.2604011114",
                "prerelease": false,
                "body": "- patch the login flow\n- bump deps",
                "assets": [
                    {
                        "browser_download_url": "https://github.com/pungin/Beanfun/releases/download/v5.8.3.2604011114/Beanfun.Setup.exe",
                        "size": 1234567,
                        "download_count": 42
                    }
                ],
                "author": { "login": "pungin" },
                "draft": false
            },
            {
                "name": "5.8.4 nightly",
                "tag_name": "v5.8.4.2604020000",
                "prerelease": true,
                "body": "draft — do not use",
                "assets": []
            }
        ]"#;

        let releases: Vec<GitHubRelease> =
            serde_json::from_str(json).expect("real-shape JSON must parse");
        assert_eq!(releases.len(), 2);

        let r0 = &releases[0];
        assert_eq!(r0.tag_name, "v5.8.3.2604011114");
        assert!(!r0.prerelease);
        assert_eq!(r0.assets.len(), 1);
        assert_eq!(
            r0.assets[0].browser_download_url,
            "https://github.com/pungin/Beanfun/releases/download/v5.8.3.2604011114/Beanfun.Setup.exe"
        );

        let r1 = &releases[1];
        assert_eq!(r1.tag_name, "v5.8.4.2604020000");
        assert!(r1.prerelease);
        assert!(r1.assets.is_empty());
    }

    #[tokio::test]
    async fn fetch_releases_at_happy_path_parses_body_and_sends_accept_header() {
        let server = MockServer::start().await;

        // Wiremock asserts the request carries our UA + Accept header
        // verbatim; if we dropped either, WPF's contract with GitHub
        // would break and this test would fail.
        let body = r#"[
            {"tag_name":"v5.8.3.2604011114","prerelease":false}
        ]"#;
        Mock::given(method("GET"))
            .and(path("/releases"))
            .and(header("accept", GITHUB_ACCEPT_HEADER))
            .and(header("user-agent", "Beanfun(V9.9.9)"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;

        let got = fetch_releases_at(&format!("{}/releases", server.uri()), "Beanfun(V9.9.9)")
            .await
            .expect("happy path");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].tag_name, "v5.8.3.2604011114");
    }

    #[tokio::test]
    async fn fetch_releases_at_non_2xx_returns_fetch_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403).set_body_string("rate limited"))
            .mount(&server)
            .await;

        let err = fetch_releases_at(&format!("{}/releases", server.uri()), "test-ua")
            .await
            .expect_err("403 must surface as UpdaterError::Fetch");
        assert!(
            matches!(err, UpdaterError::Fetch(_)),
            "expected Fetch variant, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_releases_at_malformed_body_returns_json_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let err = fetch_releases_at(&format!("{}/releases", server.uri()), "test-ua")
            .await
            .expect_err("bad body must surface as JsonDecode");
        assert!(
            matches!(err, UpdaterError::JsonDecode(_)),
            "expected JsonDecode variant, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_releases_at_transport_failure_is_fetch_error() {
        // Non-listening port → connect refused → Fetch error, not
        // JsonDecode. Locks the error-discrimination contract.
        let err = fetch_releases_at("http://127.0.0.1:1/releases", "test-ua")
            .await
            .expect_err("connect refused must surface as Fetch");
        assert!(
            matches!(err, UpdaterError::Fetch(_)),
            "expected Fetch variant, got {err:?}"
        );
    }

    #[test]
    fn gh_api_releases_url_matches_wpf_literal() {
        // WPF L117 verbatim — any drive-by edit would silently retarget
        // our updater to a different repo.
        assert_eq!(
            GH_API_RELEASES_URL,
            "https://api.github.com/repos/pungin/beanfun/releases"
        );
    }

    #[test]
    fn github_accept_header_pins_v3() {
        assert_eq!(GITHUB_ACCEPT_HEADER, "application/vnd.github.v3+json");
    }
}
