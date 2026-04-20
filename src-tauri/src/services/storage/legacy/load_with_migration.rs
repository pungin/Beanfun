//! Higher-level `load_records` wrapper that transparently upgrades a
//! legacy NRBF `Users.dat` to JSON before returning.
//!
//! Exists separately from the P5 [`load_records`][ld] because P5
//! deliberately surfaces [`StorageError::LegacyDataDetected`] as a
//! typed error to keep the core storage API migrator-agnostic. This
//! file holds the glue that stitches the two concerns together so
//! Tauri commands (P10) can call one function.
//!
//! [ld]: crate::services::storage::load_records

use std::path::Path;

use super::super::entropy::{REGISTRY_SUBKEY, REGISTRY_VALUE_NAME};
use super::super::error::StorageError;
use super::super::users_dat::{load_records_at, Records};
use super::migrator::migrate_and_save_at;

/// Load [`Records`] from `path`, auto-migrating a legacy NRBF
/// `Users.dat` to JSON format on the fly.
///
/// Behaviour by case (path refers to `%APPDATA%\Beanfun\Users.dat`):
///
/// | On-disk state                                | Result                                            |
/// | -------------------------------------------- | ------------------------------------------------- |
/// | File missing                                 | `Ok(Records::default())` (P5 fall-through)        |
/// | JSON + matching entropy                      | `Ok(records)` (P5 happy path)                     |
/// | Corrupted ciphertext / wrong entropy / UTF-8 | `Ok(Records::default())` + file deleted (P5)      |
/// | NRBF bytes + migrate OK                      | `Ok(records)` + file **rewritten as JSON**        |
/// | NRBF bytes + migrate fails                   | `Ok(Records::default())` + file **preserved**     |
///
/// The last row is the critical fail-soft: a corrupted NRBF file
/// must not be deleted, so the user can retry with a fresh build or
/// recover from backups. Matches WPF
/// `AccountManager.TryAutoMigrateLegacyData` L546-548 which
/// similarly returns empty records without deleting on a
/// `SerializationException`.
///
/// Uses the production entropy registry location; integration tests
/// should call [`load_records_with_legacy_migration_at`].
pub async fn load_records_with_legacy_migration(path: &Path) -> Result<Records, StorageError> {
    load_records_with_legacy_migration_at(path, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME).await
}

/// Lower-level variant — see
/// [`crate::services::storage::save_records_at`] for the rationale
/// behind the `_at` split.
pub async fn load_records_with_legacy_migration_at(
    path: &Path,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<Records, StorageError> {
    match load_records_at(path, entropy_subkey, entropy_value_name).await {
        Ok(records) => Ok(records),
        Err(StorageError::LegacyDataDetected { raw_bytes }) => {
            match migrate_and_save_at(path, &raw_bytes, entropy_subkey, entropy_value_name).await {
                Ok(records) => Ok(records),
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "legacy Users.dat migration failed; preserving file and returning empty records"
                    );
                    Ok(Records::default())
                }
            }
        }
        Err(other) => Err(other),
    }
}
