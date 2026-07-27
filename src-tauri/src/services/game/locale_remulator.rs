//! LocaleRemulator resource release + elevated launch.
//!
//! Covers three slices of the WPF launcher:
//!
//! | Helper                             | WPF origin                                           |
//! | ---------------------------------- | ---------------------------------------------------- |
//! | [`LR_ASSETS`] / `LR_SHA256`        | `Beanfun.csproj` embedded resources + `App.xaml.cs`  |
//! | [`verify_file`]                    | `App.xaml.cs` L140-142 `FileInfo.Length == ...`      |
//! | [`release_file`] / [`release_all`] | `App.xaml.cs` L131-167 + `MainWindow` L1904-1914     |
//! | [`build_lr_arguments`]             | `MainWindow.xaml.cs` L1917-1918 + L1930-1931         |
//! | [`launch_via_lr`] (Windows-only)   | `MainWindow.xaml.cs` L1923-1944                      |
//!
//! (`LR_SHA256` and `expected_sha256` are `pub(crate)` — the
//! build-time hash table that [`release_file`] consults. External
//! code should go through [`verify_file`] / [`release_file`]
//! instead, or compute SHA-256 directly from [`LR_ASSETS`].)
//!
//! # SHA-256 upgrade over WPF
//!
//! WPF's [`App.ReleaseResource`](https://github.com/pungin/Beanfun/blob/main/Beanfun/App.xaml.cs)
//! compares `FileInfo.Length == stream.Length` to decide whether to
//! skip the rewrite. Any DLL of identical length would bypass the check,
//! including a malicious replacement. Chunk 8.2 upgrades that guard to
//! a byte-exact SHA-256 computed at build time by
//! [`build.rs`](../../../build.rs.html) and embedded via
//! `include!(concat!(env!("OUT_DIR"), "/lr_sha256.rs"))`. The cost is a
//! single ~240 KB hash at cold launch (well under 1 ms on commodity
//! hardware); the benefit is rejection of any on-disk tampering.
//!
//! # TOCTOU stance
//!
//! Verification happens at release time. Between [`release_all`]
//! returning `Ok(_)` and [`launch_via_lr`] invoking `LRProc.exe`,
//! another process could in principle mutate the DLLs — but UAC-
//! elevated `runas` raises the attacker-capability bar considerably,
//! and WPF made no attempt at closing that window either. Re-verifying
//! after `ShellExecuteW` returns would race anyway (`ShellExecuteW`
//! has already dispatched the new process by the time control returns).
//! Documented deliberate non-handling; see D-step decisions in
//! `Todo.md` "shared design decisions C".
//!
//! # Cross-platform layout
//!
//! The WPF LR binaries are Windows-only, but all pre-launch helpers
//! — hashing, file release, argument formatting — are pure Rust and
//! build / test on macOS / Linux so developer laptops can run unit
//! tests without a VM. Only [`launch_via_lr`] (which calls
//! `ShellExecuteW`) is gated behind `#[cfg(windows)]`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::error::GameError;

include!(concat!(env!("OUT_DIR"), "/lr_sha256.rs"));

// ---------------------------------------------------------------------------
// Embedded assets
// ---------------------------------------------------------------------------

/// LocaleRemulator profile GUID used by `LRProc.exe` to select the
/// Traditional-Chinese locale profile (see
/// `Beanfun/LocaleRemulator/LRConfig.xml` → `<Profile Guid=...>`).
///
/// Passed as the first whitespace-separated token to `LRProc.exe`'s
/// command line; WPF hard-codes the same literal at
/// `MainWindow.xaml.cs` L1931.
pub const LR_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762";

/// Embedded name + byte slice for each LocaleRemulator asset.
///
/// The order matches WPF's `startByLR` short-circuit chain (L1904-1914)
/// so [`release_all`]'s `[ReleaseOutcome; 5]` output is position-stable
/// for diagnostics — index 0 is always `LRConfig.xml`, index 3 is
/// always `LRProc.exe`, etc.
///
/// Bytes come from `include_bytes!` at compile time — the Rust target
/// binary carries the 5 files inline, matching WPF's embedded-resource
/// approach and keeping Beanfun self-contained.
pub const LR_ASSETS: [(&str, &[u8]); 5] = [
    (
        "LRConfig.xml",
        include_bytes!("../../../LocaleRemulator/LRConfig.xml"),
    ),
    (
        "LRHookx32.dll",
        include_bytes!("../../../LocaleRemulator/LRHookx32.dll"),
    ),
    (
        "LRHookx64.dll",
        include_bytes!("../../../LocaleRemulator/LRHookx64.dll"),
    ),
    (
        "LRProc.exe",
        include_bytes!("../../../LocaleRemulator/LRProc.exe"),
    ),
    (
        "LRSubMenus.dll",
        include_bytes!("../../../LocaleRemulator/LRSubMenus.dll"),
    ),
];

/// Look up the expected SHA-256 for `name` from the build-generated
/// [`LR_SHA256`] table.
///
/// The name list comes from the same constant array in `build.rs` so
/// ordering / spelling mismatches would fail the build, not the runtime.
pub(crate) fn expected_sha256(name: &str) -> Option<&'static [u8; 32]> {
    LR_SHA256.iter().find(|(n, _)| *n == name).map(|(_, h)| h)
}

// ---------------------------------------------------------------------------
// verify_file
// ---------------------------------------------------------------------------

/// Hash `path` and compare the result to `expected`.
///
/// Returns:
/// - `Ok(true)` if the file exists and its SHA-256 matches `expected`.
/// - `Ok(false)` if the file is missing (so callers can treat "absent"
///   as "needs release" without branching on `ErrorKind::NotFound`).
/// - `Ok(false)` if the file exists but hashes differ.
/// - `Err(io::Error)` for any other read failure (permission denied,
///   I/O error, pointing at a directory, …) so release logic can decide
///   whether to surface the error or self-heal.
pub fn verify_file(path: &Path, expected: &[u8; 32]) -> io::Result<bool> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    Ok(digest == *expected)
}

// ---------------------------------------------------------------------------
// release_file / release_all
// ---------------------------------------------------------------------------

/// Outcome of [`release_file`] / one slot of [`release_all`].
///
/// Carried back for diagnostics (logs / test assertions) so we can tell
/// the difference between "skipped because already good" and "replaced
/// because tampered or missing".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseOutcome {
    /// Target file already existed with a matching SHA-256; nothing
    /// written.
    Skipped,
    /// Target file was written for the first time (did not exist).
    Created,
    /// Target file existed but the SHA-256 did not match the embedded
    /// blob; we deleted and rewrote it.
    Rewritten,
}

/// Release one LocaleRemulator asset into `target_dir`.
///
/// # Strategy: always write, rename over, compare only when refused
///
/// The bytes are written to `<name>.tmp` and **renamed over** the
/// target — unconditionally, without comparing first. So every launch
/// leaves a file that is byte-for-byte what this build embeds, instead
/// of a file that merely *hashed* like it at some point (a copy
/// hand-restored from another version, a half-applied update, or
/// anything an external tool "fixed" is replaced outright). The rename
/// is also atomic: a crash mid-release can't leave a truncated DLL
/// where a complete one used to be.
///
/// The only case that needs a comparison is a rename Windows refuses,
/// which in practice means the target is mapped by an LRProc / game
/// from an earlier launch:
///
/// - contents already match `expected_sha256` → nothing to do →
///   [`ReleaseOutcome::Skipped`]
/// - contents differ → the locked copy is moved aside by [`sideline`]
///   (Windows allows renaming a mapped file even when it cannot be
///   deleted) and the new bytes take the freed name →
///   [`ReleaseOutcome::Rewritten`]
/// - it cannot even be moved aside → the original rename error surfaces
///   as [`GameError::LocaleRemulatorRelease`], tagged with the
///   `'static` asset name so the UI can say which asset failed
///
/// `Created` vs `Rewritten` is a pre-write `exists()` snapshot: purely a
/// label for logging, so the TOCTOU window it carries is harmless (the
/// write itself no longer depends on it).
pub fn release_file(
    target_dir: &Path,
    name: &'static str,
    bytes: &[u8],
    expected_sha256: &[u8; 32],
) -> Result<ReleaseOutcome, GameError> {
    let target = target_dir.join(name);

    // Ensure the parent directory exists. `create_dir_all` is a no-op
    // when the directory is already there, so we don't need a separate
    // `!parent.exists()` pre-check (that check would be racey anyway).
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| GameError::LocaleRemulatorRelease { name, source: e })?;
        }
    }

    let existed = target.exists();
    let staged = target_dir.join(format!("{name}.tmp"));
    let _ = fs::remove_file(&staged); // leftover from an interrupted run
    fs::write(&staged, bytes).map_err(|e| GameError::LocaleRemulatorRelease { name, source: e })?;

    match fs::rename(&staged, &target) {
        Ok(()) => Ok(if existed {
            ReleaseOutcome::Rewritten
        } else {
            ReleaseOutcome::Created
        }),
        Err(rename_err) => {
            // Refused — the target is in use. Now (and only now) is a
            // comparison worth its I/O.
            if verify_file(&target, expected_sha256).unwrap_or(false) {
                let _ = fs::remove_file(&staged);
                return Ok(ReleaseOutcome::Skipped);
            }
            match sideline(&target) {
                Ok(()) => {
                    fs::rename(&staged, &target).map_err(|e| {
                        let _ = fs::remove_file(&staged);
                        GameError::LocaleRemulatorRelease { name, source: e }
                    })?;
                    Ok(ReleaseOutcome::Rewritten)
                }
                Err(_) => {
                    let _ = fs::remove_file(&staged);
                    Err(GameError::LocaleRemulatorRelease {
                        name,
                        source: rename_err,
                    })
                }
            }
        }
    }
}

/// Suffix marking a copy that was moved aside because it could not be
/// deleted (still in use). Swept on later releases.
pub(crate) const SIDELINED_SUFFIX: &str = ".old";

/// Move `target` aside so its name is free to be written again.
///
/// Tries `<name>.old` first, then `<name>.old.1`, `.2`, … so an earlier
/// sidelined copy that is *also* still locked doesn't block the rename.
fn sideline(target: &Path) -> io::Result<()> {
    let base = target.as_os_str().to_string_lossy().into_owned();
    for attempt in 0..8 {
        let candidate = if attempt == 0 {
            format!("{base}{SIDELINED_SUFFIX}")
        } else {
            format!("{base}{SIDELINED_SUFFIX}.{attempt}")
        };
        let candidate = PathBuf::from(candidate);
        // A leftover from a previous sideline may itself be deletable
        // now; clearing it keeps the names from creeping upward.
        if candidate.exists() && fs::remove_file(&candidate).is_err() {
            continue;
        }
        if fs::rename(target, &candidate).is_ok() {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "could not move the in-use file aside",
    ))
}

/// Best-effort removal of copies left behind by [`sideline`], now that
/// whatever held them open has probably exited. Failures are ignored —
/// a still-locked leftover is simply retried on the next release.
fn sweep_sidelined(target_dir: &Path) {
    for (name, _) in LR_ASSETS.iter() {
        let base = target_dir
            .join(name)
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        for attempt in 0..8 {
            let candidate = if attempt == 0 {
                format!("{base}{SIDELINED_SUFFIX}")
            } else {
                format!("{base}{SIDELINED_SUFFIX}.{attempt}")
            };
            let _ = fs::remove_file(candidate);
        }
    }
}

/// Release every asset in [`LR_ASSETS`] into `target_dir`, short-circuiting
/// on the first failure (mirrors WPF's `|| chain` at `MainWindow.xaml.cs`
/// L1904-1914).
///
/// Every call rewrites all five assets (see [`release_file`]); a
/// `Skipped` outcome means only that a locked target already held the
/// right bytes.
///
/// On success returns a position-stable `[ReleaseOutcome; 5]` — index
/// matches [`LR_ASSETS`] — so the caller can log per-asset actions.
pub fn release_all(target_dir: &Path) -> Result<[ReleaseOutcome; 5], GameError> {
    // Reclaim copies an earlier release had to move aside because they
    // were still in use; harmless when there are none.
    sweep_sidelined(target_dir);

    let mut outcomes = [ReleaseOutcome::Skipped; 5];
    for (idx, (name, bytes)) in LR_ASSETS.iter().enumerate() {
        let expected = expected_sha256(name).unwrap_or_else(|| {
            // Unreachable under normal builds: `build.rs` writes
            // hashes for the same list LR_ASSETS reads from.
            // Defensive against a future rename that forgets to
            // update both sides — blow up loudly in tests.
            panic!("LR asset `{name}` missing from build-time SHA-256 table");
        });
        outcomes[idx] = release_file(target_dir, name, bytes, expected)?;
    }
    Ok(outcomes)
}

// ---------------------------------------------------------------------------
// build_lr_arguments
// ---------------------------------------------------------------------------

/// Assemble the `LRProc.exe` `Arguments` string the way WPF does.
///
/// WPF (`MainWindow.xaml.cs` L1917-1918 + L1930-1931):
///
/// ```csharp
/// var commandLine = path.StartsWith("\"") ? $"{path} " : $"\"{path}\" ";
/// commandLine += command;
/// // ...
/// proc.StartInfo.Arguments =
///     "ef3e7b42-a87c-4c07-ae3e-eeebeef12762 " + commandLine;
/// ```
///
/// The guard protects already-quoted config values (where the user
/// wrote `"C:\\Games\\MapleStory\\MapleStory.exe"` with the quotes).
/// The output shape is:
///
/// ```text
/// {GUID} "{game_path}" {command_line}
/// ```
///
/// with a single space after the closing quote / unquoted path, so the
/// token boundary between path and command-line options is always
/// unambiguous even when `command_line` is empty.
pub fn build_lr_arguments(game_path: &Path, command_line: &str) -> String {
    let path_str = game_path.to_string_lossy();
    let path_part = if path_str.starts_with('"') {
        format!("{path_str} ")
    } else {
        format!("\"{path_str}\" ")
    };
    format!("{LR_GUID} {path_part}{command_line}")
}

// ---------------------------------------------------------------------------
// launch_via_lr (Windows-only)
// ---------------------------------------------------------------------------

/// Spawn `LRProc.exe` with `runas` elevation, handing it the game
/// path + credentials so `LRProc` re-launches the game under a
/// Traditional-Chinese locale.
///
/// Mirrors `MainWindow::startByLR` L1923-1944:
///
/// ```csharp
/// var proc = new Process();
/// proc.StartInfo.FileName = Path.Combine(App.AppDir, "LRProc.exe");
/// proc.StartInfo.Arguments = GUID + " " + commandLine;
/// proc.StartInfo.WorkingDirectory = Path.GetDirectoryName(path);
/// proc.StartInfo.UseShellExecute = true;
/// proc.StartInfo.Verb = "runas";
/// proc.Start();
/// ```
///
/// `ShellExecuteW` is the Win32 primitive behind `Process.Start` with
/// `UseShellExecute = true` — calling it directly avoids the dance of
/// pulling in a .NET-equivalent wrapper. Return values `<= 32` indicate
/// failure per
/// [MSDN](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew)
/// and are mapped to [`GameError::ShellExecute`] carrying both the
/// raw pseudo-HINSTANCE code and the last Win32 error.
///
/// # Async runtime guidance
///
/// `ShellExecuteW` is synchronous and blocks the calling thread while
/// the UAC consent dialog is on screen — seconds to minutes depending
/// on user reaction. When invoked from the Tauri command layer (P10)
/// on a Tokio runtime, wrap this call in [`tokio::task::spawn_blocking`]
/// so runtime worker threads stay free for other commands. WPF does
/// the equivalent by wrapping `proc.Start()` in `new Thread(...)` at
/// `MainWindow.xaml.cs` L1923; the service layer itself stays sync
/// so unit tests don't need an async harness.
///
/// [`tokio::task::spawn_blocking`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
#[cfg(windows)]
pub fn launch_via_lr(
    target_dir: &Path,
    game_path: &Path,
    command_line: &str,
) -> Result<(), GameError> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let lr_proc_path = target_dir.join("LRProc.exe");
    let arguments = build_lr_arguments(game_path, command_line);
    let working_dir: PathBuf = game_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let file_wide = to_wide_null(&lr_proc_path.to_string_lossy());
    let verb_wide = to_wide_null("runas");
    let args_wide = to_wide_null(&arguments);
    let dir_wide = to_wide_null(&working_dir.to_string_lossy());

    // Safety: every PCWSTR points into a Vec<u16> owned by this stack
    // frame for the duration of the call; `ShellExecuteW` is documented
    // to copy the strings before dispatching the child process.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(file_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            PCWSTR(dir_wide.as_ptr()),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a pseudo-HINSTANCE. Values `> 32` mean
    // success; `<= 32` is a documented error code. We use `.0 as isize`
    // to work across the Win32 / Win64 pointer-width split without
    // manually casting the windows crate's `HINSTANCE` wrapper, then
    // narrow to `i32` for the error variant — the valid error range
    // (`0..=32` per MSDN) fits comfortably and a wide HINSTANCE would
    // have landed in the success arm above.
    let raw = result.0 as isize;
    if raw <= 32 {
        return Err(GameError::ShellExecute {
            code: raw as i32,
            source: windows::core::Error::from_win32(),
        });
    }

    Ok(())
}

/// UTF-16 encode `s` with a trailing NUL, the shape
/// [`windows::core::PCWSTR`][PCWSTR] expects.
///
/// [PCWSTR]: https://microsoft.github.io/windows-docs-rs/doc/windows/core/struct.PCWSTR.html
#[cfg(windows)]
fn to_wide_null(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn known_asset() -> (&'static str, &'static [u8], &'static [u8; 32]) {
        let (name, bytes) = LR_ASSETS[0];
        (name, bytes, expected_sha256(name).unwrap())
    }

    // ---- LR_ASSETS / LR_SHA256 table invariants -------------------------

    #[test]
    fn lr_assets_and_sha256_tables_are_parallel() {
        assert_eq!(LR_ASSETS.len(), LR_SHA256.len());
        for (asset, sha) in LR_ASSETS.iter().zip(LR_SHA256.iter()) {
            assert_eq!(asset.0, sha.0, "LR_ASSETS / LR_SHA256 order mismatch");
        }
    }

    #[test]
    fn embedded_bytes_match_build_time_sha256() {
        for (name, bytes) in LR_ASSETS {
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            let expected = expected_sha256(name).unwrap();
            assert_eq!(
                &digest, expected,
                "embedded bytes for {name} do not match build-time SHA-256 — \
                 build.rs read a different file than include_bytes! embedded",
            );
        }
    }

    #[test]
    fn lr_guid_matches_wpf_literal() {
        // Lock-in against config drift — this GUID comes from
        // `Beanfun/LocaleRemulator/LRConfig.xml` Profile tag and is
        // hard-coded in WPF `MainWindow.xaml.cs` L1931. A change would
        // break LRProc.exe's profile lookup silently.
        assert_eq!(LR_GUID, "ef3e7b42-a87c-4c07-ae3e-eeebeef12762");
    }

    // ---- verify_file ----------------------------------------------------

    #[test]
    fn verify_file_returns_false_for_missing_path() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist.bin");
        assert_eq!(verify_file(&missing, &[0u8; 32]).unwrap(), false);
    }

    #[test]
    fn verify_file_returns_true_when_hash_matches() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.bin");
        let payload = b"payload-bytes";
        fs::write(&path, payload).unwrap();
        let digest: [u8; 32] = Sha256::digest(payload).into();
        assert_eq!(verify_file(&path, &digest).unwrap(), true);
    }

    #[test]
    fn verify_file_returns_false_when_hash_differs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.bin");
        fs::write(&path, b"original").unwrap();
        let bogus = [0xABu8; 32];
        assert_eq!(verify_file(&path, &bogus).unwrap(), false);
    }

    #[test]
    fn verify_file_returns_err_when_target_is_directory() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        let err = verify_file(&sub, &[0u8; 32]).unwrap_err();
        // Exact ErrorKind varies by platform (`IsADirectory` on Linux,
        // `PermissionDenied` / `Other` on Windows); we just assert it
        // is not `NotFound` (which would be silently absorbed).
        assert_ne!(err.kind(), io::ErrorKind::NotFound);
    }

    // ---- release_file ---------------------------------------------------

    #[test]
    fn release_file_creates_when_target_is_missing() {
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Created);
        let written = fs::read(dir.path().join(name)).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn release_file_rewrites_even_when_the_hash_already_matches() {
        // Always-write: a matching hash proves the bytes looked right at
        // some point, not that the file is the one this build embeds, so
        // the release replaces it regardless.
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        fs::write(dir.path().join(name), bytes).unwrap();

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Rewritten);
        assert_eq!(fs::read(dir.path().join(name)).unwrap(), bytes);
    }

    #[test]
    fn release_file_leaves_no_staging_file_behind() {
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        release_file(dir.path(), name, bytes, sha).unwrap();
        assert!(
            !dir.path().join(format!("{name}.tmp")).exists(),
            "the staged copy must be renamed into place, not left around"
        );
    }

    #[cfg(windows)]
    #[test]
    fn release_file_skips_a_locked_target_that_already_matches() {
        // A DLL mapped by a running LRProc / game cannot be renamed
        // over. When its bytes are already the ones we ship there is
        // nothing to do — the launch must proceed, not fail.
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;

        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        let target = dir.path().join(name);
        fs::write(&target, bytes).unwrap();

        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&target)
            .unwrap();

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();

        assert_eq!(outcome, ReleaseOutcome::Skipped);
        assert!(!dir.path().join(format!("{name}.tmp")).exists());
        drop(lock);
    }

    #[test]
    fn release_file_rewrites_when_hash_differs() {
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        fs::write(dir.path().join(name), b"tampered bytes").unwrap();

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Rewritten);
        let written = fs::read(dir.path().join(name)).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn release_file_sidelines_a_target_that_cannot_be_removed() {
        // A stale copy still mapped by a running LRProc / game cannot be
        // deleted — precisely when an updated DLL most needs to land. A
        // directory standing in the file's place reproduces the same
        // `remove_file` refusal without needing a real lock: the release
        // must move the obstruction aside and still write the asset.
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        let target = dir.path().join(name);
        fs::create_dir(&target).unwrap();
        fs::write(target.join("marker"), b"in use").unwrap();

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();

        assert_eq!(outcome, ReleaseOutcome::Rewritten);
        // The asset is in place with the right bytes…
        assert_eq!(fs::read(&target).unwrap(), bytes);
        // …and the obstruction was moved aside, not destroyed.
        let sidelined = dir.path().join(format!("{name}{SIDELINED_SUFFIX}"));
        assert!(
            sidelined.join("marker").exists(),
            "obstruction must be preserved"
        );
    }

    #[test]
    fn release_all_sweeps_sidelined_copies() {
        // Once whatever held the old copy open has exited, the next
        // release reclaims the space instead of leaving `.old` files
        // behind forever.
        let dir = TempDir::new().unwrap();
        for (name, bytes) in LR_ASSETS.iter() {
            fs::write(dir.path().join(name), bytes).unwrap();
            fs::write(
                dir.path().join(format!("{name}{SIDELINED_SUFFIX}")),
                b"stale",
            )
            .unwrap();
        }

        let outcomes = release_all(dir.path()).unwrap();

        // Always-write: the assets are refreshed and the leftovers go.
        assert!(outcomes.iter().all(|o| *o == ReleaseOutcome::Rewritten));
        for (name, _) in LR_ASSETS.iter() {
            assert!(
                !dir.path()
                    .join(format!("{name}{SIDELINED_SUFFIX}"))
                    .exists(),
                "{name}{SIDELINED_SUFFIX} must be swept"
            );
        }
    }

    #[test]
    fn release_file_rewrites_when_length_matches_but_hash_differs() {
        // Security-upgrade lock-in: WPF's length-only check would have
        // declared this file "already good" and skipped. SHA-256 must
        // catch it.
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        let mut tampered = bytes.to_vec();
        tampered[0] ^= 0xFF;
        fs::write(dir.path().join(name), &tampered).unwrap();
        assert_eq!(tampered.len(), bytes.len(), "test setup invariant");

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Rewritten);
    }

    #[test]
    fn release_file_auto_creates_missing_parent_directory() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("does").join("not").join("exist");
        let (name, bytes, sha) = known_asset();

        let outcome = release_file(&nested, name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Created);
        assert!(nested.join(name).exists());
    }

    #[test]
    fn release_file_handles_missing_file_as_created_not_error() {
        // TOCTOU lock-in: `verify_file` returning `Ok(false)` covers
        // both "missing" and "present but hash mismatch". The rewritten
        // `release_file` must reach the `Created` arm for the missing
        // case rather than blowing up on `remove_file`'s NotFound.
        let dir = TempDir::new().unwrap();
        let (name, bytes, sha) = known_asset();
        let target = dir.path().join(name);
        assert!(!target.exists(), "test setup invariant");

        let outcome = release_file(dir.path(), name, bytes, sha).unwrap();
        assert_eq!(outcome, ReleaseOutcome::Created);
    }

    // ---- release_all ----------------------------------------------------

    #[test]
    fn release_all_creates_every_asset_on_empty_dir() {
        let dir = TempDir::new().unwrap();
        let outcomes = release_all(dir.path()).unwrap();
        for outcome in outcomes {
            assert_eq!(outcome, ReleaseOutcome::Created);
        }
        for (name, _) in LR_ASSETS {
            assert!(dir.path().join(name).exists(), "{name} should exist");
        }
    }

    #[test]
    fn release_all_rewrites_every_asset_on_a_second_call() {
        // Always-write means a repeat launch refreshes every asset; the
        // point is that what sits on disk is what this build embeds.
        let dir = TempDir::new().unwrap();
        release_all(dir.path()).unwrap();

        let outcomes = release_all(dir.path()).unwrap();

        assert!(
            outcomes.iter().all(|o| *o == ReleaseOutcome::Rewritten),
            "second call must rewrite: {outcomes:?}"
        );
        for (name, bytes) in LR_ASSETS.iter() {
            assert_eq!(&fs::read(dir.path().join(name)).unwrap(), bytes);
        }
    }

    #[test]
    fn release_all_restores_a_tampered_asset() {
        // Under always-write every asset is refreshed, so the guarantee
        // to pin is the one that matters: tampering never survives a
        // launch.
        let dir = TempDir::new().unwrap();
        release_all(dir.path()).unwrap();
        let victim = dir.path().join("LRProc.exe");
        let mut bytes = fs::read(&victim).unwrap();
        bytes[0] ^= 0xFF;
        fs::write(&victim, &bytes).unwrap();

        let outcomes = release_all(dir.path()).unwrap();

        assert!(outcomes.iter().all(|o| *o == ReleaseOutcome::Rewritten));
        let (_, embedded) = LR_ASSETS[3];
        assert_eq!(&fs::read(&victim).unwrap(), embedded);
    }

    // ---- build_lr_arguments ---------------------------------------------

    #[test]
    fn build_lr_arguments_quotes_unquoted_path() {
        let got = build_lr_arguments(Path::new(r"C:\Games\MapleStory.exe"), "/hb /u:a /p:b");
        assert_eq!(
            got,
            "ef3e7b42-a87c-4c07-ae3e-eeebeef12762 \"C:\\Games\\MapleStory.exe\" /hb /u:a /p:b"
        );
    }

    #[test]
    fn build_lr_arguments_keeps_existing_quotes() {
        let got = build_lr_arguments(
            Path::new("\"C:\\Games\\Maple Story\\Maple.exe\""),
            "/u:a /p:b",
        );
        // Starts-with('"') arm — do NOT re-wrap.
        assert_eq!(
            got,
            "ef3e7b42-a87c-4c07-ae3e-eeebeef12762 \"C:\\Games\\Maple Story\\Maple.exe\" /u:a /p:b"
        );
    }

    #[test]
    fn build_lr_arguments_handles_empty_command_line() {
        let got = build_lr_arguments(Path::new("game.exe"), "");
        assert_eq!(got, "ef3e7b42-a87c-4c07-ae3e-eeebeef12762 \"game.exe\" ");
    }

    #[test]
    fn build_lr_arguments_preserves_path_with_spaces() {
        let got = build_lr_arguments(Path::new(r"C:\Program Files\Maple\Maple.exe"), "/hb");
        assert_eq!(
            got,
            "ef3e7b42-a87c-4c07-ae3e-eeebeef12762 \"C:\\Program Files\\Maple\\Maple.exe\" /hb"
        );
    }

    // ---- Error path (GameError) smoke -----------------------------------

    #[test]
    fn release_file_surfaces_io_error_with_static_name() {
        // Feed in a target directory that's actually a file — write
        // should fail with a platform-specific io::Error but we only
        // care the error is tagged with the right `name`.
        let dir = TempDir::new().unwrap();
        let bogus_target_dir = dir.path().join("a-file-not-a-dir");
        fs::write(&bogus_target_dir, b"existing file").unwrap();

        let (name, bytes, sha) = known_asset();
        let err = release_file(&bogus_target_dir, name, bytes, sha).unwrap_err();
        match err {
            GameError::LocaleRemulatorRelease { name: n, .. } => assert_eq!(n, name),
            other => panic!("expected LocaleRemulatorRelease, got {other:?}"),
        }
    }

    // ---- to_wide_null (Windows-only) ------------------------------------

    #[cfg(windows)]
    #[test]
    fn to_wide_null_terminates_with_zero() {
        let wide = to_wide_null("abc");
        assert_eq!(wide, vec![b'a' as u16, b'b' as u16, b'c' as u16, 0u16]);
    }

    #[cfg(windows)]
    #[test]
    fn to_wide_null_empty_string_is_just_nul() {
        assert_eq!(to_wide_null(""), vec![0u16]);
    }
}
