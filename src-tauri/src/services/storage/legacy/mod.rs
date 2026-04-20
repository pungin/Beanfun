//! Legacy `Users.dat` migration — the P6 chunk 6.2 I/O layer.
//!
//! Bridges [`crate::core::legacy`]'s pure NRBF parser (chunk 6.1)
//! with the P5 [`crate::services::storage::users_dat`] JSON save
//! path, so a WPF-era `Users.dat` transparently upgrades to the new
//! format on first read.
//!
//! # Responsibility split
//!
//! | Module               | Role                                                                     |
//! | -------------------- | ------------------------------------------------------------------------ |
//! | [`error`]            | [`LegacyMigrateError`] — boundary between `NrbfError` and `StorageError` |
//! | [`migrator`]         | Pure [`migrate_legacy_payload`] + Windows [`migrate_and_save`] (auto-save)  |
//! | [`load_with_migration`] | [`load_records_with_legacy_migration`] — P10 Tauri entry point           |
//!
//! The migrator deliberately calls [`crate::services::storage::save_records`]
//! synchronously-after-parse so the user only experiences the
//! upgrade once — aligns with WPF `AccountManager.TryAutoMigrateLegacyData`
//! L526 (`storeRecord()` immediately after a successful
//! deserialise).
//!
//! # WPF parity reference
//!
//! Source: `Beanfun/Helper/AccountManager.cs::TryAutoMigrateLegacyData`
//! (the `catch (Exception) when (ex is SerializationException ||
//! ex is InvalidCastException)` path).
//!
//! | Concern                             | WPF line   | Here                                                        |
//! | ----------------------------------- | ---------- | ----------------------------------------------------------- |
//! | Detect legacy binary payload        | L494-504   | P5 `load_records` → `StorageError::LegacyDataDetected`      |
//! | `BinaryFormatter.Deserialize`       | L506-512   | [`migrate_legacy_payload`] (via chunk 6.1 `parse_legacy_payload`) |
//! | `JsonConvert.SerializeObject` bridge | L513-521 | skipped — direct `WireRecords::normalize` (chunk 6.x decision D) |
//! | `accRecInit` padding                | L522       | `WireRecords::normalize` via `records_from_wire_lists`      |
//! | `storeRecord` immediate save        | L526       | [`migrate_and_save`] inner `save_records` call              |
//! | Success toast (`MessageBoxShow`)    | L536       | **not ported** — service layer is UI-free; `tracing::info!` |
//! | `SerializationException` fail-soft  | L546-548   | [`load_records_with_legacy_migration`] warn + `Records::default()` + file preserved |
//!
//! # What this module does *not* do
//!
//! - **No delete-on-failure**: a migrate failure preserves the
//!   legacy file so the user can retry (backup, newer build, etc).
//!   The P5 `load_records` catch-all only deletes on
//!   **ciphertext corruption**, not on "valid ciphertext +
//!   malformed NRBF".
//! - **No UI signalling**: chunk 6.2 stays service-layer only;
//!   notifying the user that a migration happened is a P10/P11
//!   concern.
//! - **No arbitrary NRBF acceptance**: chunk 6.1 already gates the
//!   root class to `Beanfun.Records` / `Beanfun.AccountRecords`; a
//!   foreign `.NET` type planted in `Users.dat` surfaces as
//!   [`NrbfError::UnsupportedClass`][usc], which the migrator
//!   wraps in `LegacyMigrateError::Nrbf` and
//!   `load_records_with_legacy_migration` fail-softs on.
//!
//! [usc]: crate::core::legacy::NrbfError::UnsupportedClass

pub mod error;
pub mod migrator;

#[cfg(target_os = "windows")]
pub mod load_with_migration;

pub use error::LegacyMigrateError;
pub use migrator::migrate_legacy_payload;

#[cfg(target_os = "windows")]
pub use load_with_migration::{
    load_records_with_legacy_migration, load_records_with_legacy_migration_at,
};
#[cfg(target_os = "windows")]
pub use migrator::{migrate_and_save, migrate_and_save_at};
