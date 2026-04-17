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
//! # Layers (chunk 7.1 scope)
//!
//! | Module             | Responsibility                                                       |
//! | ------------------ | -------------------------------------------------------------------- |
//! | [`error`]          | `UpdaterError` — typed failures across the updater pipeline          |
//! | [`parser`]         | `ParsedVersion` / `parse_tag` / `is_newer_version` (pure, cross-OS)  |
//! | [`mod@proxy_probe`] | `proxy_probe` / `proxy_probe_at` — proxy discovery (HEAD + strict 2xx) |
//!
//! Chunks 7.2 (`github.rs` + `Channel`) and 7.3 (`checker.rs`) land
//! in follow-up commits; this module will grow `pub use` re-exports
//! as they arrive.

pub mod error;
pub mod parser;
pub mod proxy_probe;

pub use error::UpdaterError;
pub use parser::{is_newer_version, parse_tag, ParsedVersion};
pub use proxy_probe::{proxy_probe, proxy_probe_at};
