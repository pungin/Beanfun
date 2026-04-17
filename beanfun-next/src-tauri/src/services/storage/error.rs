//! Typed error enum for the storage layer.
//!
//! Currently scopes to chunk 5.1 (DPAPI + entropy); chunks 5.2 (Users.dat)
//! and 5.3 (Config.xml) will append further variants below.
//!
//! # Design
//!
//! - DPAPI errors are carried as a plain `String` (from `windows::core::Error::to_string`)
//!   rather than as the concrete `windows::core::Error` type, so the enum
//!   stays free of the `windows` dependency on non-Windows builds.
//! - Registry errors reuse `std::io::Error` since `winreg` already wraps
//!   the Win32 error codes into `io::Error`; they get their own variant
//!   (not `#[from]` to avoid silent propagation) so caller logs can
//!   distinguish registry failures from generic file I/O in later chunks.
//! - `EntropyMissing` is **not** an I/O error — it is the documented
//!   first-time-run signal that callers should react to by generating a
//!   fresh [`crate::services::storage::Entropy`] and writing it back.

use thiserror::Error;

/// Typed failure surface for the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// DPAPI `CryptProtectData` / `CryptUnprotectData` call failed.
    ///
    /// `operation` carries the human-readable API name for logs (e.g.
    /// `"CryptProtectData"`); `message` is the stringified
    /// `windows::core::Error`.
    #[error("DPAPI {operation} failed: {message}")]
    Dpapi {
        /// Human-readable Win32 API name; constant per call site.
        operation: &'static str,
        /// Stringified Win32 error for diagnostics.
        message: String,
    },

    /// Registry read / write under `HKCU\SOFTWARE\BEANFUN` failed for a
    /// reason other than `NotFound` (which maps to [`Self::EntropyMissing`]).
    #[error("registry I/O error: {0}")]
    Registry(#[source] std::io::Error),

    /// Entropy value is not present in the registry — typically a
    /// first-time run. Callers should generate a new
    /// [`crate::services::storage::Entropy`] and persist it.
    #[error("entropy value not found in registry")]
    EntropyMissing,

    /// Entropy value is present but did not match the expected
    /// `[A-Z0-9]{{8}}` shape produced by
    /// [`crate::services::storage::Entropy::generate`]. Callers should
    /// regenerate.
    #[error("entropy value has invalid shape")]
    EntropyShape,
}
