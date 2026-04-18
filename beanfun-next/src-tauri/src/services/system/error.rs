//! Typed failure surface for the system service.
//!
//! Each variant maps to a distinct `system.*` code on the
//! [`crate::commands::error::CommandError`] boundary so the frontend
//! can branch on cause without parsing free-form error messages.
//!
//! # Variants
//!
//! | Variant                              | Cause                                                 |
//! | ------------------------------------ | ----------------------------------------------------- |
//! | [`SystemError::InvalidUrl`]          | URL is empty / missing scheme / scheme not in allowlist |
//! | [`SystemError::OpenFailed`]          | OS opener returned an I/O error while launching the URL |
//! | [`SystemError::SpawnBlockingFailed`] | `tokio::task::spawn_blocking` panicked or was cancelled |

use thiserror::Error;

/// Typed error for the system service (currently only `open_url`;
/// future `open_folder` / `show_in_finder` additions will extend
/// this enum rather than introducing parallel error types, so every
/// service-layer call shares one code namespace).
#[derive(Debug, Error)]
pub enum SystemError {
    /// URL failed basic validation — empty, missing scheme, or using
    /// a scheme outside the allowlist (`http` / `https` / `mailto`).
    /// Guards against `file://` info-leak, `javascript:` XSS-like
    /// surfaces, `data:` binary payloads, and custom URI handlers
    /// the user did not expect to be invokable from the frontend.
    #[error("invalid URL `{url}`: {reason}")]
    InvalidUrl {
        /// The URL as received from the caller (verbatim, un-sanitised
        /// — useful in error messages so the user sees what was
        /// rejected without guessing).
        url: String,
        /// Human-readable reason the URL failed validation.
        reason: String,
    },

    /// The OS-level opener (`ShellExecuteW` on Windows,
    /// `LSOpenCFURLRef` on macOS, `xdg-open` on Linux) returned an
    /// I/O error while trying to launch the URL. Typical causes:
    /// default handler for `mailto:` not configured, browser binary
    /// moved, permission denied.
    #[error("failed to open URL `{url}`: {source}")]
    OpenFailed {
        /// The URL that failed to open.
        url: String,
        /// The underlying I/O error from [`open::that`].
        #[source]
        source: std::io::Error,
    },

    /// The [`tokio::task::spawn_blocking`] wrapper that hosts the
    /// synchronous [`open::that`] call panicked or was cancelled.
    /// Should not happen in steady state (the closure contains only
    /// a single `open::that` call) but we surface it as a distinct
    /// variant so the command layer can emit
    /// `system.spawn_blocking_failed` rather than conflating it with
    /// a real opener failure.
    #[error("blocking task panicked or was cancelled: {0}")]
    SpawnBlockingFailed(#[source] tokio::task::JoinError),
}
