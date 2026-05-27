//! Read-only Windows registry helpers for the launcher.
//!
//! Ports the **read** side of `Beanfun/Helper/ModifyRegistry.cs`
//! (`ModifyRegistry.Read`, L73-99), specifically the game-path lookup
//! driven by `MainWindow::selectedGameChanged` L574-605. That flow:
//!
//! 1. Reads `dir_reg` + `dir_value_name` from the per-game INI (P11 Config).
//! 2. Reads the requested hive/subkey candidates for
//!    `<dir_reg>@<dir_value_name>`.
//! 3. If present, seeds `ConfigAppSettings` (Config.xml) with the value
//!    so future launches don't need the registry at all.
//!
//! Step 3 is **not** in this module — writing the game path is a
//! `Config.xml` concern handled by [`crate::services::config`] (P11),
//! not a registry write-back. WPF only writes registry for DPAPI
//! entropy (which lives in [`crate::services::storage::entropy`]) and
//! never for game paths.
//!
//! # Why both [`Hive`] variants are active
//!
//! Some INI rows carry an explicit `HKEY_LOCAL_MACHINE\` prefix while
//! some legacy installs seed per-user values. The command layer builds
//! candidate hives from the INI value; this module keeps the read
//! primitive small and hive-explicit.
//!
//! # Layers
//!
//! | Module        | Responsibility                                       |
//! |---------------|------------------------------------------------------|
//! | [`error`]     | `RegistryError` — typed failures across reads        |
//! | [`game_path`] | `read_game_path` — HKCU/HKLM value lookup            |

pub mod error;
pub mod game_path;

pub use error::RegistryError;
pub use game_path::read_game_path;

/// Which Windows registry root (hive) a read targets.
///
/// Wraps the two `HKEY_*` roots the WPF launcher ever touches:
/// `HKEY_CURRENT_USER` (the real game-path source — see module docs)
/// and `HKEY_LOCAL_MACHINE` (legacy installers / future callers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

impl Hive {
    /// Wrap the `HKEY_*` constant in a [`winreg::RegKey`] predef
    /// handle. `winreg` knows not to close predef handles so the
    /// returned `RegKey` is safe to drop.
    pub(crate) fn as_reg_key(self) -> winreg::RegKey {
        match self {
            Hive::CurrentUser => winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER),
            Hive::LocalMachine => winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE),
        }
    }

    /// Human-readable name (`"HKEY_CURRENT_USER"` etc.) — used in
    /// [`RegistryError`]'s `Display` output and in module docs.
    pub fn display_name(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKEY_CURRENT_USER",
            Hive::LocalMachine => "HKEY_LOCAL_MACHINE",
        }
    }
}

impl std::fmt::Display for Hive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}
