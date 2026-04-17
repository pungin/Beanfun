//! Local secure storage layer — DPAPI, registry entropy, and (in later P5
//! chunks) the `Users.dat` / `Config.xml` wrappers.
//!
//! Ports the legacy C# storage surface under `Beanfun/Helper/`:
//!
//! - [`AccountManager`][wpf-acm] `readRawData` / `writeRawData` → [`dpapi`]
//!   + [`entropy`] + (chunk 5.2) `users_dat` (not yet added).
//! - [`ModifyRegistry`][wpf-reg] `Read` / `Write` against
//!   `HKCU\SOFTWARE\BEANFUN` → [`entropy`].
//!
//! [wpf-acm]: ../../../../../../../Beanfun/Helper/AccountManager.cs
//! [wpf-reg]: ../../../../../../../Beanfun/Helper/ModifyRegistry.cs
//!
//! # Platform
//!
//! DPAPI and the registry helpers are Windows-only. The [`dpapi`] module
//! is gated `#[cfg(target_os = "windows")]`; [`entropy::Entropy::generate`]
//! and shape parsing are cross-platform so pure-logic tests can run
//! anywhere, but [`entropy::read_from_registry`] /
//! [`entropy::write_to_registry`] are Windows-only.
//!
//! # Layers (current chunk scope)
//!
//! | Module      | Responsibility                                                |
//! |-------------|---------------------------------------------------------------|
//! | [`error`]   | `StorageError` — typed failures across storage operations     |
//! | [`dpapi`]   | `dpapi_protect` / `dpapi_unprotect` — `CurrentUser`-scope API |
//! | [`entropy`] | `Entropy(String)` — 8-char `[A-Z0-9]` DPAPI salt + registry   |
//!
//! Later chunks (5.2 Users.dat, 5.3 Config.xml) extend this listing.

pub mod entropy;
pub mod error;

#[cfg(target_os = "windows")]
pub mod dpapi;

pub use entropy::Entropy;
pub use error::StorageError;

#[cfg(target_os = "windows")]
pub use dpapi::{dpapi_protect, dpapi_unprotect};
#[cfg(target_os = "windows")]
pub use entropy::{read_from_registry, write_to_registry};
