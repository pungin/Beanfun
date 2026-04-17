//! Local secure storage layer — DPAPI, registry entropy, and the
//! `Users.dat` JSON store. The `Config.xml` AppSettings store is a
//! sibling module under [`crate::services::config`] (chunk 5.3) — it
//! shares no Win32 surface with this layer so the two are kept apart.
//!
//! Ports the legacy C# storage surface under `Beanfun/Helper/`:
//!
//! - [`AccountManager`][wpf-acm] `readRawData` / `writeRawData` →
//!   [`dpapi`] + [`entropy`] + [`users_dat`].
//! - [`AccountManager`][wpf-acm] `loadRecord` / `storeRecord` /
//!   `importRecord` / `exportRecord` / `accRecInit` → [`users_dat`].
//! - [`ModifyRegistry`][wpf-reg] `Read` / `Write` against
//!   `HKCU\SOFTWARE\BEANFUN` → [`entropy`].
//!
//! [wpf-acm]: ../../../../../../../Beanfun/Helper/AccountManager.cs
//! [wpf-reg]: ../../../../../../../Beanfun/Helper/ModifyRegistry.cs
//!
//! # Platform
//!
//! DPAPI, the registry helpers, and the IO-bearing `Users.dat`
//! save/load APIs are Windows-only. The [`dpapi`] module is gated
//! `#[cfg(target_os = "windows")]`; [`entropy::Entropy::generate`] /
//! shape parsing and the [`users_dat::parse_records`] /
//! [`users_dat::export_records`] pure-logic helpers are
//! cross-platform so unit tests can run anywhere.
//!
//! # Layers (current chunk scope)
//!
//! | Module        | Responsibility                                                           |
//! |---------------|--------------------------------------------------------------------------|
//! | [`error`]     | `StorageError` — typed failures across storage operations                |
//! | [`dpapi`]     | `dpapi_protect` / `dpapi_unprotect` — `CurrentUser`-scope API            |
//! | [`entropy`]   | `Entropy(String)` — 8-char `[A-Z0-9]` DPAPI salt + registry              |
//! | [`users_dat`] | `Records` / `save_records` / `load_records` / `import` / `export`        |
//! | [`legacy`]    | P6 migrator — NRBF `Users.dat` → JSON auto-upgrade on load               |

pub mod entropy;
pub mod error;
pub mod legacy;
pub mod users_dat;

#[cfg(target_os = "windows")]
pub mod dpapi;

pub use entropy::Entropy;
pub use error::StorageError;
pub use legacy::{migrate_legacy_payload, LegacyMigrateError};
pub use users_dat::{export_records, parse_records, Account, Records};

#[cfg(target_os = "windows")]
pub use dpapi::{dpapi_protect, dpapi_unprotect};
#[cfg(target_os = "windows")]
pub use entropy::{read_from_registry, write_to_registry};
#[cfg(target_os = "windows")]
pub use legacy::{
    load_records_with_legacy_migration, load_records_with_legacy_migration_at, migrate_and_save,
    migrate_and_save_at,
};
#[cfg(target_os = "windows")]
pub use users_dat::{
    default_users_dat_path, import_records, import_records_at, load_records, load_records_at,
    save_records, save_records_at,
};
