//! Top-level `check_update` composition — wires the 7.1/7.2 primitives
//! (`proxy_probe` / `fetch_releases` / `select_release` / `parse_tag` /
//! `is_newer_version`) into a single service-layer entry point that
//! P10 Tauri commands will call.
//!
//! # WPF parity
//!
//! Mirrors `Beanfun/Update/ApplicationUpdater.cs` `RunCheck` (L114-199)
//! structurally, with two deliberate departures:
//!
//! - **No UI**: the original `MessageBox.Show` / `Process.Start` calls
//!   (L150-191) are **not** in this layer. This module only produces
//!   an [`UpdateInfo`] value the UI can render. The split matches the
//!   service-layer charter established across P5/P6/P7.1/P7.2.
//! - **No concurrency guard**: WPF's `Interlocked.CompareExchange` guard
//!   (L93-94) protecting against simultaneous "startup probe +
//!   About-page click" lives in the Tauri command layer; service is
//!   single-call.
//!
//! Everything else matches WPF byte-for-byte:
//!
//! | WPF line          | Behaviour                                               | This module                            |
//! | ----------------- | ------------------------------------------------------- | -------------------------------------- |
//! | L116              | `proxy = GetProxy()` (cached)                           | [`check_update`] → [`super::proxy_probe()`] |
//! | L117              | `url = proxy + "https://…/releases"`                    | `run_check` `format!("{prefix}{api_url}")`   |
//! | L121-127          | `WebClient.DownloadData` + `User-Agent` + `Accept` headers | [`super::fetch_releases_at`]        |
//! | L127              | `JsonConvert.DeserializeObject<List<GitHubRelease>>`    | [`super::fetch_releases_at`] → `serde_json` |
//! | L128-131          | `release = GetLastRelease(releases)` + null-bail        | [`super::select_release`]              |
//! | L135-137          | `Regex.Match` + `!match.Success → return`               | [`super::parse_tag`]                   |
//! | L145              | `newVerDisplay = "{major}.{minor}.{patch}({timestamp})"` | [`UpdateInfo::from_release`]          |
//! | L148              | `IsNewerVersion(App.AssemblyVersion, …)` gate           | [`super::is_newer_version`]            |
//! | L169-172          | `downloadUrl = (assets[0] ? proxy+url : tag page)`      | [`UpdateInfo::from_release`]           |
//! | L195-198          | `catch Exception → Debug.WriteLine` silent              | each `.ok()?` + `tracing::warn!`       |
//!
//! # Silent-on-error policy
//!
//! WPF's entire `RunCheck` body sits inside `try { … } catch (Exception
//! ex) { Debug.WriteLine("Update check failed: " + ex.Message); }`
//! (L119-198). Any fault — network, proxy loop, JSON decode, regex
//! mismatch — is silently swallowed and the function returns without
//! surfacing anything to the UI. We preserve the same behaviour by
//! returning `Option<UpdateInfo>` where `None` means "no update
//! available **or** something went wrong silently", matching the WPF
//! `show ? "No Updates Found" : no-op` UX contract.
//!
//! Tests and future discriminating callers can still drop down one
//! layer and call [`super::fetch_releases_at`] / [`super::parse_tag`]
//! / [`super::proxy_probe_at`] directly to get typed
//! [`super::UpdaterError`] values. The collapse-to-`None` happens only
//! at this top-level composition site.
//!
//! # `download_url` proxy asymmetry
//!
//! WPF L169-172 is intentionally non-symmetric about proxy prefixing:
//!
//! ```csharp
//! string downloadUrl =
//!     (release.Assets != null && release.Assets.Count > 0)
//!         ? proxy + release.Assets[0].BrowserDownloadUrl
//!         : $"https://github.com/pungin/Beanfun/releases/tag/{release.TagName}";
//! ```
//!
//! - Asset branch: **prepends `proxy`**, so e.g. when `proxy =
//!   "https://ghproxy.vip/"` the download is routed through the
//!   third-party mirror.
//! - Fallback branch: **never prepends `proxy`**, so the release page
//!   URL is always direct `github.com/...`.
//!
//! The asymmetry is a WPF design choice (probably because the fallback
//! is opened in the user's default browser via `Process.Start`, and a
//! proxied release-page URL displays ugly), not a bug. We mirror it
//! byte-for-byte here.

use std::borrow::Cow;

use super::github::{fetch_releases_at, select_release, Channel, GitHubRelease};
use super::parser::{is_newer_version, parse_tag, ParsedVersion};
use super::proxy_probe::{proxy_probe, proxy_probe_at};
use super::{UpdaterError, GH_API_RELEASES_URL};

/// Result of a successful update check where a newer release was found.
///
/// UI callers render this directly — `new_version_display` goes into
/// the "Detect New Version {0}" message header, `body` renders as
/// release notes Markdown, `download_url` is what the "Download" button
/// opens, and `tag_name` is retained for diagnostics / telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// Human-readable version string produced by
    /// `format!("{major}.{minor}.{patch}({timestamp})")` — matches
    /// WPF L145 `newVerDisplay`.
    pub new_version_display: String,
    /// Markdown body the maintainer wrote on the release page.
    /// Forwarded verbatim from [`GitHubRelease::body`].
    pub body: String,
    /// Direct link to the first binary asset (with proxy prefix applied
    /// when one was discovered) or — if the release has no assets — a
    /// fallback release-tag page URL on `github.com`. See the
    /// "`download_url` proxy asymmetry" section in the [module
    /// docs][self] for why the fallback is never proxied.
    pub download_url: String,
    /// Original release tag, e.g. `v5.8.3.2604011114`. Retained for
    /// logging, analytics, and for the UI layer to link to the release
    /// page even when the asset-download URL was provided.
    pub tag_name: String,
}

impl UpdateInfo {
    /// Build [`UpdateInfo`] from a release + its parsed version + the
    /// proxy prefix that was in effect during the probe. Pure
    /// (synchronous) so tests of the presentation layer can construct
    /// it without a running wiremock.
    ///
    /// The `proxy_prefix` convention mirrors [`mod@super::proxy_probe`]:
    /// empty string means "direct access, no proxy".
    pub fn from_release(
        release: &GitHubRelease,
        parsed: &ParsedVersion,
        proxy_prefix: &str,
    ) -> Self {
        let display = format!(
            "{}.{}.{}({})",
            parsed.major, parsed.minor, parsed.patch, parsed.timestamp,
        );

        let download_url = match release.assets.first() {
            Some(asset) => format!("{proxy_prefix}{}", asset.browser_download_url),
            None => format!(
                "https://github.com/pungin/Beanfun/releases/tag/{}",
                release.tag_name,
            ),
        };

        Self {
            new_version_display: display,
            body: release.body.clone(),
            download_url,
            tag_name: release.tag_name.clone(),
        }
    }
}

/// Production-flavoured update check.
///
/// Uses the cached [`super::proxy_probe()`] so only one HEAD probe
/// sequence runs per process lifetime (matching WPF `Lazy<string>
/// _cachedProxy` L24-27), then delegates to an internal `run_check` helper with the
/// hard-coded GitHub URL and a Cargo-versioned `User-Agent`.
///
/// Callers that need URL-level injection (tests, future proxy-switching
/// features) should use [`check_update_at`] instead.
///
/// `local_version` is passed through to [`is_newer_version`]: for
/// end-users this is whatever the launcher displays as its version
/// (typically the display form `X.Y.Z(timestamp)` produced by WPF's
/// `App.ConvertVersion` equivalent — see the `parser` module docs for
/// the two-path comparator and when each path triggers).
pub async fn check_update(channel: Channel, local_version: &str) -> Option<UpdateInfo> {
    let proxy_prefix = proxy_probe().await;
    let ua = production_user_agent();
    run_check(
        proxy_prefix,
        GH_API_RELEASES_URL,
        &ua,
        channel,
        local_version,
    )
    .await
}

/// URL-injected variant of [`check_update`] for tests and diagnostics.
///
/// Bypasses the process-wide proxy cache (each call runs its own probe
/// sequence against the supplied URLs) so integration tests get
/// deterministic behaviour without having to reset a global static.
/// In production this would waste a handful of HEAD requests per
/// check, which is why [`check_update`] exists as the cached wrapper.
///
/// `probe_direct_url` and `probe_proxies` are forwarded to
/// [`proxy_probe_at`]; `api_releases_url` is the URL that receives the
/// actual `GET` (whichever proxy prefix wins is prepended onto it);
/// `user_agent` is sent on both the HEAD probe and the GET fetch.
pub async fn check_update_at(
    probe_direct_url: &str,
    probe_proxies: &[&str],
    api_releases_url: &str,
    channel: Channel,
    local_version: &str,
    user_agent: &str,
) -> Option<UpdateInfo> {
    let proxy_prefix = proxy_probe_at(probe_direct_url, probe_proxies).await;
    run_check(
        &proxy_prefix,
        api_releases_url,
        user_agent,
        channel,
        local_version,
    )
    .await
}

/// Core pipeline shared by [`check_update`] and [`check_update_at`].
///
/// Not public: the two entry points above differ only in how they
/// discover `proxy_prefix` (cached OnceLock vs fresh per-call), so
/// keeping the pipeline itself in one place is the DRY-correct shape.
///
/// `proxy_prefix` is the empty string for direct access or a proxy
/// base URL ending in `/` (matches [`mod@super::proxy_probe`]
/// convention).
async fn run_check(
    proxy_prefix: &str,
    api_releases_url: &str,
    user_agent: &str,
    channel: Channel,
    local_version: &str,
) -> Option<UpdateInfo> {
    let fetch_url = format!("{proxy_prefix}{api_releases_url}");

    let releases = match fetch_releases_at(&fetch_url, user_agent).await {
        Ok(rs) => rs,
        Err(err) => {
            log_and_discard("fetch_releases", err);
            return None;
        }
    };

    let release = match select_release(&releases, channel) {
        Some(r) => r,
        None => {
            tracing::info!(
                ?channel,
                release_count = releases.len(),
                "update check: no release matched channel filter (up-to-date or empty feed)"
            );
            return None;
        }
    };

    let parsed = match parse_tag(&release.tag_name) {
        Ok(p) => p,
        Err(err) => {
            log_and_discard("parse_tag", err);
            return None;
        }
    };

    if !is_newer_version(local_version, &parsed) {
        tracing::info!(
            local = %local_version,
            remote = %release.tag_name,
            "update check: local version is up-to-date"
        );
        return None;
    }

    Some(UpdateInfo::from_release(release, &parsed, proxy_prefix))
}

/// Log a non-fatal updater error at `warn` level and drop it on the
/// floor. Matches the WPF `catch Exception → Debug.WriteLine` pattern
/// (L195-198) — see the module-level "Silent-on-error policy" section
/// for the rationale.
fn log_and_discard(stage: &str, err: UpdaterError) {
    tracing::warn!(
        stage = stage,
        error = %err,
        "update check: silent failure"
    );
}

/// WPF-matching `User-Agent` string used by the production
/// [`check_update`]. Kept as a helper (rather than inlined into
/// [`check_update`]) so both [`mod@super::proxy_probe`] and this module
/// produce byte-identical UAs, and so future UA changes happen in one
/// place.
fn production_user_agent() -> Cow<'static, str> {
    Cow::Owned(format!("Beanfun(V{})", env!("CARGO_PKG_VERSION")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::updater::github::{GitHubAsset, GITHUB_ACCEPT_HEADER};
    use pretty_assertions::assert_eq;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_release(tag: &str, prerelease: bool, with_asset: bool) -> GitHubRelease {
        GitHubRelease {
            name: format!("Beanfun {tag}"),
            tag_name: tag.to_owned(),
            prerelease,
            body: "- bugfixes\n- perf".to_owned(),
            assets: if with_asset {
                vec![GitHubAsset {
                    browser_download_url: format!(
                        "https://github.com/pungin/Beanfun/releases/download/{tag}/Setup.exe"
                    ),
                }]
            } else {
                Vec::new()
            },
        }
    }

    #[test]
    fn update_info_new_version_display_matches_wpf_format() {
        let release = sample_release("v5.8.3.2604011114", false, true);
        let parsed = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 3,
            timestamp: "2604011114".to_owned(),
        };
        let info = UpdateInfo::from_release(&release, &parsed, "");
        assert_eq!(info.new_version_display, "5.8.3(2604011114)");
        assert_eq!(info.tag_name, "v5.8.3.2604011114");
        assert_eq!(info.body, "- bugfixes\n- perf");
    }

    #[test]
    fn update_info_download_url_prepends_proxy_for_asset_branch() {
        let release = sample_release("v5.8.3.2604011114", false, true);
        let parsed = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 3,
            timestamp: "2604011114".to_owned(),
        };
        let info = UpdateInfo::from_release(&release, &parsed, "https://ghproxy.vip/");
        assert_eq!(
            info.download_url,
            "https://ghproxy.vip/https://github.com/pungin/Beanfun/releases/download/v5.8.3.2604011114/Setup.exe"
        );
    }

    #[test]
    fn update_info_download_url_no_proxy_for_asset_when_direct() {
        let release = sample_release("v5.8.3.2604011114", false, true);
        let parsed = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 3,
            timestamp: "2604011114".to_owned(),
        };
        let info = UpdateInfo::from_release(&release, &parsed, "");
        assert_eq!(
            info.download_url,
            "https://github.com/pungin/Beanfun/releases/download/v5.8.3.2604011114/Setup.exe"
        );
    }

    #[test]
    fn update_info_download_url_fallback_skips_proxy_per_wpf_asymmetry() {
        // WPF L171-172 asymmetry lock-in: fallback page URL is NEVER
        // prefixed with proxy, even when the probe found one. Any
        // future "make this consistent" refactor should trip this
        // test and prompt a discussion.
        let release = sample_release("v5.8.3.2604011114", false, false);
        let parsed = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 3,
            timestamp: "2604011114".to_owned(),
        };
        let info = UpdateInfo::from_release(&release, &parsed, "https://ghproxy.vip/");
        assert_eq!(
            info.download_url, "https://github.com/pungin/Beanfun/releases/tag/v5.8.3.2604011114",
            "fallback URL must NOT include proxy prefix — matches WPF L172 behaviour"
        );
    }

    // ---- Async end-to-end tests via wiremock --------------------

    async fn mount_probe_ok(server: &MockServer) {
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200))
            .mount(server)
            .await;
    }

    async fn mount_releases(server: &MockServer, body: &str) {
        Mock::given(method("GET"))
            .and(path("/releases"))
            .and(header("accept", GITHUB_ACCEPT_HEADER))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn check_update_at_happy_path_returns_update_info_with_newer_version() {
        let server = MockServer::start().await;
        mount_probe_ok(&server).await;
        mount_releases(
            &server,
            r#"[
                {
                    "tag_name": "v5.8.4.2604020000",
                    "prerelease": false,
                    "body": "critical fix",
                    "assets": [{"browser_download_url": "https://github.com/pungin/Beanfun/releases/download/v5.8.4.2604020000/Setup.exe"}]
                }
            ]"#,
        )
        .await;

        let got = check_update_at(
            &server.uri(),
            &[],
            &format!("{}/releases", server.uri()),
            Channel::Stable,
            "5.8.3(2604011114)",
            "test-ua",
        )
        .await
        .expect("new version must be detected");

        assert_eq!(got.tag_name, "v5.8.4.2604020000");
        assert_eq!(got.new_version_display, "5.8.4(2604020000)");
        assert_eq!(got.body, "critical fix");
        // Direct path succeeded so proxy_prefix = "" and asset URL is verbatim.
        assert_eq!(
            got.download_url,
            "https://github.com/pungin/Beanfun/releases/download/v5.8.4.2604020000/Setup.exe"
        );
    }

    #[tokio::test]
    async fn check_update_at_returns_none_when_local_matches_latest() {
        let server = MockServer::start().await;
        mount_probe_ok(&server).await;
        mount_releases(
            &server,
            r#"[{"tag_name":"v5.8.3.2604011114","prerelease":false}]"#,
        )
        .await;

        let got = check_update_at(
            &server.uri(),
            &[],
            &format!("{}/releases", server.uri()),
            Channel::Stable,
            "5.8.3(2604011114)",
            "test-ua",
        )
        .await;

        assert!(
            got.is_none(),
            "same-timestamp display form must short-circuit to None"
        );
    }

    #[tokio::test]
    async fn check_update_at_returns_none_when_tag_does_not_match_regex() {
        let server = MockServer::start().await;
        mount_probe_ok(&server).await;
        mount_releases(
            &server,
            r#"[{"tag_name":"definitely-not-semver","prerelease":false}]"#,
        )
        .await;

        let got = check_update_at(
            &server.uri(),
            &[],
            &format!("{}/releases", server.uri()),
            Channel::Stable,
            "5.8.3(2604011114)",
            "test-ua",
        )
        .await;

        assert!(got.is_none(), "unparseable tag must swallow to None");
    }

    #[tokio::test]
    async fn check_update_at_returns_none_on_fetch_failure() {
        let server = MockServer::start().await;
        mount_probe_ok(&server).await;
        // Releases endpoint returns 500 — fetch_releases_at surfaces
        // UpdaterError::Fetch, check_update_at should swallow.
        Mock::given(method("GET"))
            .and(path("/releases"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let got = check_update_at(
            &server.uri(),
            &[],
            &format!("{}/releases", server.uri()),
            Channel::Stable,
            "5.8.3(2604011114)",
            "test-ua",
        )
        .await;

        assert!(got.is_none());
    }

    #[tokio::test]
    async fn check_update_at_returns_none_when_list_is_empty() {
        let server = MockServer::start().await;
        mount_probe_ok(&server).await;
        mount_releases(&server, "[]").await;

        let got = check_update_at(
            &server.uri(),
            &[],
            &format!("{}/releases", server.uri()),
            Channel::Stable,
            "5.8.3(2604011114)",
            "test-ua",
        )
        .await;

        assert!(got.is_none(), "empty releases list must return None");
    }
}
