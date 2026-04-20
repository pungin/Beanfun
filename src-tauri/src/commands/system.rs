//! System-level commands — `version` / `ping` / `open_url`.
//!
//! Smoke commands (`version`, `ping`) exercise the full IPC pipeline
//! (frontend → Tauri dispatcher → `#[tauri::command]` → specta
//! binding → serde round-trip) end-to-end without involving any
//! Beanfun-specific domain logic; getting them green at
//! infrastructure commit time is the fastest way to surface wiring
//! regressions. [`open_url`] is the first real functional command in
//! this module — a thin wrapper over
//! [`crate::services::system::open_url()`] that ports three WPF
//! `Process.Start(..., UseShellExecute = true)` sites
//! (`ApplicationUpdater` download link, `About` GitHub/mailto
//! buttons, `MainWindow.runGame` update prompt) under one uniform
//! `system.*` surface.
//!
//! # Design notes
//!
//! - [`version`] is **synchronous + infallible**, the simplest shape
//!   Tauri accepts. It proves the command dispatcher and specta
//!   binding work for struct return types.
//! - [`ping`] is **async + fallible + blocks inside
//!   [`tokio::task::spawn_blocking`]** — the pattern every Win32
//!   wrapper in P10.2+ will use (Win32 APIs are overwhelmingly
//!   synchronous; running them on the async executor directly stalls
//!   the reactor). A 60 ms sleep is enough to prove an `await` point
//!   actually suspends without being a noticeable nuisance during
//!   interactive testing.
//! - [`open_url`] delegates to
//!   [`crate::services::system::open_url()`] which handles scheme
//!   allowlisting + `spawn_blocking` isolation internally; the
//!   command stays a three-line thin wrapper (P10.3-Q1 = A:
//!   "command = IPC boundary, service = business logic"). We do
//!   **not** use `tauri-plugin-opener` from the backend because its
//!   Rust API requires `AppHandle`, which would re-couple the
//!   service layer to the Tauri runtime (see
//!   [`crate::services::system`] module doc).
//!
//! # `system.*` codes introduced here
//!
//! - `system.spawn_blocking_failed` — shared with
//!   [`crate::services::system::SystemError::SpawnBlockingFailed`];
//!   emitted both from the ad-hoc [`ping`] path (no service error
//!   intermediate) and from [`open_url`] via the `SystemError →
//!   CommandError` impl. See
//!   [module-level docs][crate::commands::error] for the full
//!   `system.*` code table.
//! - `system.invalid_url` / `system.open_url_failed` — minted by the
//!   `SystemError → CommandError` conversion in
//!   [`crate::commands::error`]; this module adds no extra codes.

use serde::Serialize;
use specta::Type;

use crate::commands::error::CommandError;
use crate::services::system;

/// Compile-time build metadata returned by [`version`].
///
/// `app` is this crate's own version (derived from `Cargo.toml` via
/// the `CARGO_PKG_VERSION` environment variable Cargo sets at build
/// time); `tauri` is the Tauri framework version the binary was
/// compiled against — useful in bug reports to confirm the IPC
/// dispatcher expected by the frontend matches what's running.
#[derive(Debug, Clone, Serialize, Type)]
pub struct VersionInfo {
    /// Our own crate version (`env!("CARGO_PKG_VERSION")` at compile
    /// time).
    pub app: String,
    /// Tauri framework version ([`tauri::VERSION`] at compile time).
    pub tauri: String,
}

/// Return the static build metadata. Infallible; no parameters; no
/// state.
///
/// Intended as the simplest possible Tauri command — if this
/// doesn't round-trip correctly, nothing else will. Also serves as
/// the canonical example of a sync `#[tauri::command]` with a
/// structured return type for future documentation.
#[tauri::command]
#[specta::specta]
pub fn version() -> VersionInfo {
    VersionInfo {
        app: env!("CARGO_PKG_VERSION").to_string(),
        tauri: tauri::VERSION.to_string(),
    }
}

/// Round-trip an input string through a blocking worker thread.
///
/// Exercises the canonical Win32-wrapping pattern: the closure runs
/// on a [`tokio::task::spawn_blocking`] pool worker (not the reactor
/// thread), sleeps for 60 ms to prove the `await` point genuinely
/// suspends, then returns `"pong: {input}"`.
///
/// Failure path: if the blocking task panics or is cancelled the
/// [`tokio::task::JoinError`] is mapped to
/// `system.spawn_blocking_failed`. Should never happen in steady
/// state — the closure is a sleep + `format!` with no fallible ops —
/// but the code path needs to exist so the pattern is complete for
/// the real Win32 wrappers in P10.2+.
#[tauri::command]
#[specta::specta]
pub async fn ping(message: String) -> Result<String, CommandError> {
    tokio::task::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        format!("pong: {message}")
    })
    .await
    .map_err(|err| {
        CommandError::new(
            "system.spawn_blocking_failed",
            format!("blocking task panicked or was cancelled: {err}"),
        )
    })
}

/// Open `url` in the user's default handler (browser for http/https,
/// mail client for mailto).
///
/// Thin wrapper over [`crate::services::system::open_url()`] — the
/// service layer enforces the scheme allowlist (`http` / `https` /
/// `mailto`) and wraps the synchronous [`open::that`] call in
/// [`tokio::task::spawn_blocking`], so this command stays a
/// single-line delegation (P10.3-Q1 = A decision: command layer is
/// strictly the IPC boundary, business logic lives in `services/`).
///
/// # Errors
///
/// - `system.invalid_url` — URL is empty, missing a scheme, or uses
///   a scheme outside the allowlist. Rejected before any OS call.
/// - `system.open_url_failed` — OS opener (`ShellExecuteW` /
///   `LSOpenCFURLRef` / `xdg-open`) returned an I/O error.
/// - `system.spawn_blocking_failed` — the blocking task hosting
///   [`open::that`] panicked or was cancelled (should not happen in
///   steady state).
#[tauri::command]
#[specta::specta]
pub async fn open_url(url: String) -> Result<(), CommandError> {
    system::open_url(&url).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_cargo_and_tauri_versions() {
        let info = version();
        assert_eq!(info.app, env!("CARGO_PKG_VERSION"));
        assert!(
            !info.tauri.is_empty(),
            "tauri::VERSION must not be empty at build time"
        );
    }

    #[tokio::test]
    async fn ping_echoes_message_with_pong_prefix() {
        let response = ping("hello".to_string())
            .await
            .expect("ping should succeed under normal conditions");
        assert_eq!(response, "pong: hello");
    }

    #[tokio::test]
    async fn ping_round_trips_unicode_payload() {
        // The blocking worker `format!` should not lose non-ASCII
        // bytes; this guards against accidental ASCII-only handling
        // in the pattern every Win32 wrapper in P10.2+ inherits.
        let response = ping("你好".to_string())
            .await
            .expect("ping should handle unicode");
        assert_eq!(response, "pong: 你好");
    }

    // -----------------------------------------------------------------
    // open_url — error-path symbol tests. The happy path (actually
    // launching the default browser) is covered by the service-layer
    // `services::system::open_url` tests plus future integration
    // tests; here we only assert the command correctly surfaces
    // scheme-allowlist rejection through the `SystemError →
    // CommandError` path so the IPC contract stays intact.
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn open_url_rejects_empty_url_as_system_invalid_url() {
        let err = open_url(String::new())
            .await
            .expect_err("empty URL must be rejected by the service layer");
        assert_eq!(err.code, "system.invalid_url");
    }

    #[tokio::test]
    async fn open_url_rejects_file_scheme_as_system_invalid_url() {
        let err = open_url("file:///C:/Windows/System32/cmd.exe".to_string())
            .await
            .expect_err("file:// must be rejected");
        assert_eq!(err.code, "system.invalid_url");
    }

    #[tokio::test]
    async fn open_url_rejects_javascript_scheme_as_system_invalid_url() {
        let err = open_url("javascript:alert(1)".to_string())
            .await
            .expect_err("javascript: must be rejected");
        assert_eq!(err.code, "system.invalid_url");
    }
}
