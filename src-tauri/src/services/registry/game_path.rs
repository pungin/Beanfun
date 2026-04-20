//! Registry lookup for a game's installation path.
//!
//! Ports `MainWindow::selectedGameChanged` L574-605's registry branch.
//! The WPF flow, condensed:
//!
//! ```csharp
//! string dir_reg = ...;          // from per-game INI
//! string dir_value_name = ...;   // from per-game INI, default "Path"
//! ModifyRegistry reg = new ModifyRegistry { RootHive = Registry.CurrentUser, SubKey = dir_reg };
//! string value = reg.Read(dir_value_name);
//! if (value != null) { ConfigAppSettings.SetValue(dir_value_name + "." + gameCode, value); }
//! ```
//!
//! [`read_game_path`] produces the same `Option<String>` as
//! `ModifyRegistry.Read` — missing key or missing value both return
//! `Ok(None)` because the WPF launcher treats "nothing in registry"
//! as an ordinary cold-start case, not an error.
//!
//! # Unicode
//!
//! `winreg`'s `get_value::<String, _>` uses the `*W` variants under
//! the hood and reads `REG_SZ` / `REG_EXPAND_SZ` values as UTF-16.
//! If a registry entry happens to hold invalid UTF-16 (unpaired
//! surrogates, odd byte length, etc.), winreg surfaces that as
//! [`io::ErrorKind::InvalidData`] — **not** a silent lossy
//! re-encode — which we forward as [`RegistryError::ReadValue`]
//! with the original `io::Error` preserved via `#[source]`. Callers
//! that care (game-path reads, DPAPI entropy reads, …) therefore
//! either get a clean `String` or a typed error, never a garbled
//! half-decoded path.

use std::io;

use super::error::RegistryError;
use super::Hive;

/// Read a string value from the Windows registry.
///
/// Returns:
/// - `Ok(Some(value))` — key and value both present, value read as
///   a UTF-8 `String`.
/// - `Ok(None)` — key is missing, or key is present but the value is
///   missing. Either case is normal (e.g. "no game installed yet").
/// - `Err(RegistryError::OpenKey)` — unexpected failure opening
///   `<hive>\<subkey>` (permission denied, IO error, …).
/// - `Err(RegistryError::ReadValue)` — key opened but reading
///   `value_name` failed with something other than NotFound (wrong
///   type, IO error, …).
///
/// # WPF parity
///
/// The happy path mirrors `ModifyRegistry.Read` L73-99:
/// > if `OpenSubKey` returns `null` or `GetValue` returns `null`, the
/// > C# code returns an empty string `""`. WPF callers then check
/// > `if (value != null && value != "")`.
///
/// The Rust API collapses "missing key" / "missing value" / `""` into
/// the same `Ok(None)` variant because Rust callers would otherwise
/// have to re-implement the `.IsNullOrEmpty` check everywhere.
pub fn read_game_path(
    hive: Hive,
    subkey: &str,
    value_name: &str,
) -> Result<Option<String>, RegistryError> {
    let root = hive.as_reg_key();

    let key = match root.open_subkey(subkey) {
        Ok(k) => k,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(RegistryError::OpenKey {
                hive,
                subkey: subkey.to_string(),
                source: e,
            })
        }
    };

    match key.get_value::<String, _>(value_name) {
        Ok(s) if s.is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(RegistryError::ReadValue {
            hive,
            subkey: subkey.to_string(),
            value_name: value_name.to_string(),
            source: e,
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `HKEY_CURRENT_USER\Environment` exists on every Windows system
    /// and has a `TEMP` value — a stable probe for "happy path reads
    /// do return `Some`".
    #[test]
    fn read_known_present_value_returns_some() {
        let got = read_game_path(Hive::CurrentUser, "Environment", "TEMP")
            .expect("read_game_path should not error");
        let v = got.expect("HKCU\\Environment@TEMP should be present");
        assert!(!v.is_empty(), "TEMP should have a non-empty path");
    }

    /// Missing subkey → `Ok(None)`. Use an unmistakable GUID-like
    /// nonce that no sane program could have registered.
    #[test]
    fn read_missing_subkey_returns_none() {
        let got = read_game_path(
            Hive::CurrentUser,
            r"SOFTWARE\__BEANFUN_NEXT_P9_NONCE_1D0F2A7E__",
            "Whatever",
        )
        .expect("should not error");
        assert!(got.is_none(), "expected None, got {got:?}");
    }

    /// Key exists but value doesn't → `Ok(None)`.
    #[test]
    fn read_missing_value_in_existing_key_returns_none() {
        let got = read_game_path(
            Hive::CurrentUser,
            "Environment",
            "__BEANFUN_NEXT_P9_UNLIKELY_VALUE__",
        )
        .expect("should not error");
        assert!(got.is_none(), "expected None, got {got:?}");
    }

    /// Smoke-test LocalMachine branch — `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`
    /// exists on every Windows install and its `ProductName` is a
    /// non-empty `REG_SZ`.
    #[test]
    fn read_hklm_known_value() {
        let got = read_game_path(
            Hive::LocalMachine,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "ProductName",
        )
        .expect("should not error");
        let v = got.expect("HKLM ProductName should be present");
        assert!(
            v.to_ascii_lowercase().contains("windows"),
            "ProductName should mention Windows, got {v:?}"
        );
    }

    #[test]
    fn hive_display_name_matches_reg_syntax() {
        assert_eq!(Hive::CurrentUser.display_name(), "HKEY_CURRENT_USER");
        assert_eq!(Hive::LocalMachine.display_name(), "HKEY_LOCAL_MACHINE");
        assert_eq!(format!("{}", Hive::CurrentUser), "HKEY_CURRENT_USER");
    }
}
