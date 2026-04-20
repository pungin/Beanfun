//! Legacy `.NET` BinaryFormatter (NRBF) interop — **read-only**.
//!
//! P6 responsibility: parse the legacy `Users.dat` payload that the
//! WPF build of Beanfun used to write with
//! `BinaryFormatter.Serialize(oldRecords)`, and expose it as a pure
//! domain model ([`LegacyPayload`]) so the application layer
//! (`services::storage::legacy`, chunk 6.2 — not yet implemented)
//! can convert it into the modern [`Records`][rec] shape and re-save
//! as JSON.
//!
//! Scope is intentionally narrow — **only** the two classes Beanfun
//! ever serialized:
//!
//! - `Beanfun.Records` (7 fields, current WPF shape)
//! - `Beanfun.AccountRecords` (6 fields, pre-`accountNameList` shape
//!   retained for backwards compat with installs that never re-saved)
//!
//! Any other root class → [`NrbfError::UnsupportedClass`]. We refuse
//! to execute arbitrary NRBF graphs, matching the security posture of
//! .NET 9's `NrbfDecoder` (read-only, no type activation).
//!
//! # Why we don't enable the `nrbf` crate's `serde` feature
//!
//! The upstream `nrbf` crate has optional serde integration that can
//! auto-unwrap `System.Collections.Generic.List<T>` into `Vec<T>`,
//! but it hard-codes the member count (`_items` / `_size` /
//! `_version`) to **exactly 3** and falls through for anything else.
//! .NET Framework can ship `List<T>` with an extra `_syncRoot`
//! optional field (4 members) depending on the runtime version that
//! wrote the stream. To stay robust across WPF runtimes we walk
//! `nrbf::value::Object::members` ourselves and tolerate both
//! shapes — see [`nrbf::parse_legacy_payload`].
//!
//! # WPF parity reference
//!
//! | Concern                           | WPF `AccountManager.cs` | Here                                                             |
//! | --------------------------------- | ----------------------- | ---------------------------------------------------------------- |
//! | Root class detection              | `L501-503`              | `parse_legacy_payload` class-name match                          |
//! | `List<string>` → `Vec<String>`    | implicit via `BinaryFormatter.Deserialize` + reflection | `nrbf::extract_list_of_strings`                                  |
//! | `null` list field → empty list    | `accRecInit` fallback   | `extract_list_*` treat `Value::Null` as `Vec::new()`             |
//! | `null` list element → `""`        | JSON round-trip         | `extract_list_of_strings` short-circuits to `String::new()`      |
//! | Legacy `AccountRecords` (6 field) | `L513-521`              | [`LegacyPayload::AccountRecords`] variant                        |
//! | Current `Records` (7 field)       | `L506-512`              | [`LegacyPayload::Records`] variant                               |
//! | Upgrade-save to JSON              | `L526 storeRecord()`    | chunk 6.2 `services::storage::legacy::migrate_and_save`          |
//! | Migration failure → empty records | `L546-548 catch`        | chunk 6.2 `load_records_with_legacy_migration` warn + `Default`  |
//! | `MessageBoxShow` success toast    | `L536`                  | not ported — service layer is UI-free; left for P10/P11          |
//!
//! [rec]: crate::services::storage::Records

pub mod error;
pub mod nrbf;

pub use error::NrbfError;
pub use nrbf::{parse_legacy_payload, LegacyAccountRecords, LegacyPayload, LegacyRecords};
