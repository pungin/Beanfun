//! Application updater — detects new Beanfun releases on GitHub via a
//! fallback proxy chain.
//!
//! Ports the legacy `Beanfun/Update/ApplicationUpdater.cs` (294 LOC)
//! into a service-layer shape:
//!
//! - Service-layer only: no MessageBox / `Process.Start` /
//!   `ConfigAppSettings` reads. Those land in P10 Tauri commands +
//!   P11 Vue UI once the top-level `check_update` entry point from
//!   chunk 7.3 returns a structured `UpdateInfo`.
//! - Top-level `check_update` (chunk 7.3) will funnel everything into
//!   an `Option<UpdateInfo>` + `tracing::warn!` log to match WPF
//!   `catch (Exception) { Debug.WriteLine }` (L195-198); lower
//!   layers (`proxy_probe_at` / `fetch_releases_at` / `parse_tag` /
//!   `is_newer_version`) return `Result<_, UpdaterError>` for tests
//!   and discriminating callers.
//!
//! # Layers (P7 complete)
//!
//! | Module               | Responsibility                                                        |
//! | -------------------- | --------------------------------------------------------------------- |
//! | [`error`]            | `UpdaterError` — typed failures across the updater pipeline           |
//! | [`parser`]           | `ParsedVersion` / `parse_tag` / `is_newer_version` (pure, cross-OS)   |
//! | [`mod@proxy_probe`]  | `proxy_probe` / `proxy_probe_at` — proxy discovery (HEAD + strict 2xx)|
//! | [`github`]           | `GitHubRelease` / `Channel` / `fetch_releases` / `select_release`     |
//! | [`checker`]          | `check_update` / `check_update_at` / `UpdateInfo` — top-level pipeline|
//!
//! # Call graph (top-down)
//!
//! ```text
//! check_update(channel, local_version)
//!   └─ proxy_probe()                       (OnceLock-cached)
//!   └─ run_check(prefix, api_url, ua, channel, local_version)
//!        ├─ fetch_releases_at(fetch_url, ua)
//!        ├─ select_release(&releases, channel)
//!        ├─ parse_tag(release.tag_name)
//!        ├─ is_newer_version(local_version, &parsed)
//!        └─ UpdateInfo::from_release(release, parsed, prefix)
//! ```
//!
//! Top-level `check_update` collapses all errors into `Option::None`
//! (matching WPF `catch Exception → Debug.WriteLine` silent policy
//! at L195-198); lower layers preserve typed [`UpdaterError`] for
//! tests and diagnostics.

pub mod checker;
pub mod error;
pub mod github;
pub mod parser;
pub mod proxy_probe;

pub use checker::{check_update, check_update_at, UpdateInfo};
pub use error::UpdaterError;
pub use github::{
    fetch_releases, fetch_releases_at, select_release, Channel, GitHubAsset, GitHubRelease,
    GH_API_RELEASES_URL, GITHUB_ACCEPT_HEADER,
};
pub use parser::{is_newer_version, parse_tag, ParsedVersion};
pub use proxy_probe::{proxy_probe, proxy_probe_at};
