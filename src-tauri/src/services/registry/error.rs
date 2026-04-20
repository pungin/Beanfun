//! Typed errors for [`services/registry`][`super`].
//!
//! Only two variants for P9.1: "couldn't open the subkey" and
//! "couldn't read the value". Both carry enough context
//! (`hive\subkey[@value_name]`) to diagnose without re-reading the
//! call site. More variants get added here if / when write-side
//! registry support lands (currently out of scope — writes live in
//! [`crate::services::config`] as Config.xml edits).

use std::io;

use super::Hive;

/// Every failure that a registry read can surface.
///
/// Both variants preserve the originating [`std::io::Error`] via
/// `#[source]` so callers that care about `io::ErrorKind::NotFound`
/// (e.g. "fall through to LKM") can inspect it. The happy-path
/// "missing key / missing value" surface in [`super::read_game_path`]
/// is `Ok(None)`, not this error type — `RegistryError` is for
/// *unexpected* IO failures.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// `RegOpenKeyExW` returned something other than `ERROR_SUCCESS`
    /// or `ERROR_FILE_NOT_FOUND`. Typical cause: permission denied
    /// when the caller lacks read access on the subkey's ACL.
    #[error("failed to open registry key {hive}\\{subkey}")]
    OpenKey {
        hive: Hive,
        subkey: String,
        #[source]
        source: io::Error,
    },

    /// `RegQueryValueExW` returned something other than
    /// `ERROR_SUCCESS` or `ERROR_FILE_NOT_FOUND`. Typical cause: the
    /// value exists but is the wrong type for the caller (e.g. we
    /// asked for `REG_SZ` but the value is `REG_DWORD`).
    #[error("failed to read registry value {hive}\\{subkey}@{value_name}")]
    ReadValue {
        hive: Hive,
        subkey: String,
        value_name: String,
        #[source]
        source: io::Error,
    },
}
