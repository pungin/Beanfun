//! Typed errors for [`services/game`][`super`].
//!
//! Declared up-front (chunk 8.1) so the enum shape is stable across
//! 8.1 / 8.2. Variants that only the LocaleRemulator path
//! ([`super::launcher`] does not produce them) are gated behind
//! `#[cfg(windows)]` where relevant — the enum still compiles on
//! non-Windows so cross-platform unit tests for [`super::launcher`]
//! primitives can run.
//!
//! # WPF mapping
//!
//! | Variant                         | WPF origin                                                                         |
//! | ------------------------------- | ---------------------------------------------------------------------------------- |
//! | [`PathEmpty`]                   | `MainWindow.xaml.cs` L1748 — `gamePath == ""` short-circuit                        |
//! | [`PathNotFound`]                | `MainWindow.xaml.cs` L1748 — `!File.Exists(gamePath)` short-circuit                |
//! | [`PathNonAscii`]                | `MainWindow.xaml.cs` L1753-1762 — UTF-16 code-unit `> 128` → `MsgGamePathHaveWChar`|
//! | [`LocaleRemulatorRelease`]      | `App.xaml.cs` L144-151 + `MainWindow.xaml.cs` L1905-1909 (`ReleaseResource == -1`)  |
//! | [`LocaleRemulatorSha256Mismatch`] | **beanfun-next exclusive** — WPF only length-checked; SHA-256 rejection is new    |
//! | [`ShellExecute`]                | `MainWindow.xaml.cs` L1935 `proc.Start()` throwing via `UseShellExecute = runas`   |
//! | [`Spawn`]                       | `MainWindow.xaml.cs` L1890 `Process.Start(startInfo)` throwing on Normal mode      |
//!
//! [`PathEmpty`]: GameError::PathEmpty
//! [`PathNotFound`]: GameError::PathNotFound
//! [`PathNonAscii`]: GameError::PathNonAscii
//! [`LocaleRemulatorRelease`]: GameError::LocaleRemulatorRelease
//! [`LocaleRemulatorSha256Mismatch`]: GameError::LocaleRemulatorSha256Mismatch
//! [`ShellExecute`]: GameError::ShellExecute
//! [`Spawn`]: GameError::Spawn

use std::path::PathBuf;

/// Every failure that [`services/game`][`super`] can surface.
///
/// Chunk 8.1 only produces the first three + [`GameError::Spawn`];
/// the LocaleRemulator-only variants are declared here to keep the
/// enum shape stable when chunk 8.2 wires them up (avoids a second
/// breaking change to public `GameError`).
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// Game path was not configured yet — `Settings::t_GamePath.Text
    /// == ""` in WPF. Mapped by the UI to the "Can't find game"
    /// dialog (WPF L1730-1745).
    #[error("game path is empty")]
    PathEmpty,

    /// Game path was configured but the target file does not exist.
    /// Same UI surface as [`GameError::PathEmpty`] (WPF short-circuits
    /// to the same dialog at L1748).
    #[error("game path does not exist: {}", .path.display())]
    PathNotFound { path: PathBuf },

    /// Game path contains a non-ASCII character; the WPF game loader
    /// refuses paths with any UTF-16 code unit > 128 because the
    /// game binary passes the path through ANSI/CP950 code pages
    /// internally and blows up on wide characters.
    ///
    /// `offending_char` and `position` are diagnostic: the UI can
    /// show "position 3: '遊'" to help the user understand which
    /// character triggered the refusal.
    #[error(
        "game path contains non-ASCII character {offending_char:?} at position {position}: {}",
        .path.display()
    )]
    PathNonAscii {
        path: PathBuf,
        offending_char: char,
        position: usize,
    },

    /// Writing one of the five LocaleRemulator resource files to disk
    /// failed (permission denied, disk full, antivirus lock, …).
    /// `name` is the logical resource name (`"LRProc.exe"`, …) so
    /// the UI message can point at the exact file.
    #[error("LocaleRemulator resource release failed for {name}")]
    LocaleRemulatorRelease {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },

    /// A LocaleRemulator resource file already existed on disk but
    /// its SHA-256 did not match the embedded blob **and** the
    /// delete / overwrite attempt also failed — i.e. we noticed
    /// tampering but couldn't self-heal. In the happy path a
    /// mismatch leads to silent overwrite, not this variant.
    ///
    /// # Security upgrade over WPF
    ///
    /// WPF only compared `FileInfo.Length` (`App.xaml.cs` L140-142),
    /// which a malicious DLL of identical length would bypass.
    /// SHA-256 closes that gap at the cost of a small one-time hash
    /// per startup.
    #[error("LocaleRemulator {name}: SHA-256 mismatch and self-heal failed")]
    LocaleRemulatorSha256Mismatch { name: &'static str },

    /// `ShellExecuteW` (Windows-only) failed to launch `LRProc.exe`
    /// via the `runas` verb — typically the user cancelled the UAC
    /// prompt, or UAC is disabled and the process creation failed.
    ///
    /// `code` is the raw pseudo-HINSTANCE return value from
    /// `ShellExecuteW`; values `<= 32` are documented Win32 error
    /// codes (e.g. `SE_ERR_FNF = 2`, `SE_ERR_ACCESSDENIED = 5`,
    /// `SE_ERR_OOM = 8`, `ERROR_CANCELLED = 1223` for UAC refused).
    /// Preserved verbatim so the UI layer (P10) can branch on
    /// "UAC cancelled" vs "LRProc.exe missing" without re-interpreting
    /// `GetLastError`, whose reliability for `ShellExecuteW` MSDN
    /// does not guarantee. `source` carries whatever `GetLastError`
    /// returned at the call site as a best-effort secondary signal.
    #[cfg(windows)]
    #[error("ShellExecuteW failed to launch LRProc.exe (code={code})")]
    ShellExecute {
        code: i32,
        #[source]
        source: windows::core::Error,
    },

    /// `std::process::Command::spawn` failed for the Normal-mode
    /// launch — permission, missing binary at the exact resolved
    /// path, etc. The underlying [`std::io::Error`] is preserved
    /// via `#[from]` for ergonomic `?`.
    #[error("failed to spawn game process")]
    Spawn(#[from] std::io::Error),
}
