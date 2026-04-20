//! Typed error for the legacy `Users.dat` migration pipeline — the
//! boundary between [`crate::core::legacy`]'s NRBF parsing and the
//! P5 storage layer's JSON save.
//!
//! See [`LegacyMigrateError`] for variant rationale.

use thiserror::Error;

use super::super::error::StorageError;
use crate::core::legacy::NrbfError;

/// Failure surface for `migrate_legacy_payload` / `migrate_and_save`.
///
/// Intentionally *not* merged into [`StorageError`] — the parse
/// concern belongs to `core::legacy`, the save concern belongs to
/// `services::storage`; fusing them would pull `NrbfError` into
/// every P5 caller's error-handling match.
#[derive(Debug, Error)]
pub enum LegacyMigrateError {
    /// NRBF parse of the raw ciphertext-decoded bytes failed. The
    /// legacy `Users.dat` on disk is untouched; caller should treat
    /// this as "migration impossible", preserve the file (WPF
    /// `AccountManager.TryAutoMigrateLegacyData` L546-548 fail-soft)
    /// and return empty records.
    #[error("legacy Users.dat NRBF parse failed: {0}")]
    Nrbf(#[from] NrbfError),

    /// Migrator converted the payload successfully, but the
    /// follow-up [`crate::services::storage::save_records`] call that
    /// overwrites `Users.dat` with the JSON wire format failed. The
    /// in-memory [`Records`][rec] is lost from the caller's
    /// perspective because the on-disk state did not transition; UI
    /// should surface "migration incomplete — original file
    /// preserved" and next load will retry.
    ///
    /// [rec]: crate::services::storage::Records
    #[error("save-after-migrate failed: {0}")]
    Storage(#[from] StorageError),
}
