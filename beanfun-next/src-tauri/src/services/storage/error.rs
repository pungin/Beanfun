//! Typed error enum for the storage layer.
//!
//! Currently scopes to chunks 5.1 (DPAPI + entropy) and 5.2 (Users.dat);
//! chunk 5.3 (Config.xml) will append further variants below.
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
//! - [`StorageError::Io`] / [`StorageError::Json`] /
//!   [`StorageError::LegacyDataDetected`] are added in chunk 5.2
//!   (Users.dat). DPAPI / registry / UTF-8 / JSON parse failures
//!   during `load_records` are intentionally *not* propagated — they
//!   are caught internally and treated as "first-time run" matching
//!   WPF `AccountManager.readRawData`'s single-catch-all
//!   (`Beanfun/Helper/AccountManager.cs` L226-229). The remaining
//!   variants surface only the errors that callers can meaningfully
//!   react to.

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

    /// Generic file I/O failure on the `Users.dat` (or, in chunk 5.3,
    /// `Config.xml`) path — read / write / metadata / `mkdir_p`.
    /// Distinct from [`Self::Registry`] so caller logs can pinpoint
    /// the failure surface.
    #[error("storage I/O error: {0}")]
    Io(#[source] std::io::Error),

    /// JSON serialization (save) or deserialization (parse / import)
    /// failed. `parse_records` and `import_records` propagate this for
    /// genuinely malformed input; `save_records` / `export_records`
    /// surface the rare case where `serde_json::to_string` fails.
    ///
    /// Note that `load_records` does **not** propagate this variant —
    /// JSON parse failure on the on-disk plaintext triggers the
    /// base64 / legacy fallback, see
    /// [`crate::services::storage::users_dat::load_records`].
    #[error("JSON encode/decode failed: {0}")]
    Json(#[source] serde_json::Error),

    /// The on-disk `Users.dat` (or imported blob) plaintext failed
    /// `serde_json::from_str` but successfully `BASE64.decode`d — i.e.
    /// it is the legacy WPF `BinaryFormatter` (NRBF) wire format from
    /// before the JSON migration.
    ///
    /// `raw_bytes` is the decoded ciphertext; the P6 NRBF migrator
    /// will take it from here. Callers without an NRBF migrator must
    /// fall back to returning an empty
    /// [`crate::services::storage::Records`] **without** deleting the
    /// file (matching WPF `AccountManager.TryAutoMigrateLegacyData`).
    #[error("legacy BinaryFormatter data detected ({} bytes)", raw_bytes.len())]
    LegacyDataDetected {
        /// Base64-decoded raw bytes of the legacy NRBF stream.
        raw_bytes: Vec<u8>,
    },
}
