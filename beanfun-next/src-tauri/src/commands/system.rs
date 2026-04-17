//! System-level smoke commands — `version` + `ping`.
//!
//! These two commands exist primarily to exercise the full IPC
//! pipeline (frontend → Tauri dispatcher → `#[tauri::command]` →
//! specta binding → serde round-trip) end-to-end without involving
//! any Beanfun-specific domain logic. Every feature pair after
//! P10.1 (auth / launcher / storage / …) inherits the same
//! plumbing, so getting these two green at infrastructure commit
//! time is the fastest way to surface wiring regressions.
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
//!
//! # `system.*` codes introduced here
//!
//! - `system.spawn_blocking_failed` — [`tokio::task::JoinError`]
//!   surfaced when the blocking task panicked or was cancelled.
//!   Should not happen in steady state; worth a distinct code so the
//!   frontend can treat it as a hard-stop rather than a retriable
//!   domain failure.

use serde::Serialize;
use specta::Type;

use crate::commands::error::CommandError;

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
}
