//! Check for and terminate the MapleStory `Patcher.exe` process.
//!
//! # WPF parity
//!
//! Ports the `kill` half of `MainWindow::checkPatcher_Tick`
//! (`Beanfun/MainWindow.xaml.cs` L2455-2477). The WPF timer body is:
//!
//! ```csharp
//! string patherPath = Path.GetDirectoryName(settingPage.t_GamePath.Text)
//!                     + "\\Patcher.exe";
//! foreach (Process process in Process.GetProcessesByName("Patcher"))
//! {
//!     try
//!     {
//!         if (process.MainModule.FileName == patherPath)
//!         {
//!             process.Kill();
//!             found = true;
//!         }
//!     }
//!     catch { }  // per-process swallow
//! }
//! ```
//!
//! The Rust port keeps the **kill semantics** (enumerate, filter by
//! exact path, best-effort kill each) and drops everything after the
//! `if (found)` branch (L2478-2613) — the server-version RPC, the
//! `MessageBox`, and the download URL belong to
//! [`crate::services::updater`] + the P10 command layer, not to the
//! single-shot kill primitive. See [`super`]'s "Timer ownership"
//! section for the split rationale.
//!
//! # Best-effort kill (Q2=B2)
//!
//! A per-pid [`kill_process`] failure is **silently skipped** so one
//! unkillable instance (e.g. protected-mode, or the process exiting
//! between `find` and `kill`) doesn't block the other instances from
//! being cleaned up. This mirrors WPF's nested `try / catch {}`. The
//! [`find_processes_by_name`] call itself is **not** swallowed — a
//! WMI failure there means the whole operation can't run and is
//! propagated as [`ProcessError`].
//!
//! # Async runtime guidance
//!
//! [`check_and_kill_patcher`] composes two blocking Win32 / WMI
//! calls — callers running on Tokio should dispatch it via
//! [`tokio::task::spawn_blocking`][sb]. Same COM-apartment caveat
//! as [`super::find::find_processes_by_name`] applies, because we
//! invoke that underneath.
//!
//! [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use std::path::Path;

use super::error::ProcessError;
use super::find::{find_processes_by_name, ProcessInfo};
use super::kill::kill_process;

/// The executable name WMI matches against. WPF uses the
/// extension-less `"Patcher"` (via `Process.GetProcessesByName`); our
/// [`find_processes_by_name`] takes the WMI `Name` field which
/// **includes** the extension.
pub const PATCHER_EXE_NAME: &str = "Patcher.exe";

/// Locate every `Patcher.exe` running from the directory that hosts
/// `game_path` and terminate them. Returns the PIDs that were
/// successfully killed — empty `Vec` means "nothing to do".
///
/// # Parity
///
/// WPF's `found: bool` result maps to `!killed.is_empty()`; we return
/// the PID list instead of a bare `bool` so the P10 command layer can
/// log which process it reaped without re-querying WMI. See module
/// docs for the semantics match.
///
/// # Arguments
///
/// * `game_path` — the configured `MapleStory.exe` path (or any file
///   inside the game directory). The Patcher lookup uses
///   `game_path.parent().join("Patcher.exe")` to compute the expected
///   full path, matching WPF's
///   `Path.GetDirectoryName(gamePath) + "\\Patcher.exe"` exactly.
///
///   The early `Ok(Vec::new())` short-circuit fires only when
///   [`Path::parent`] returns `None` — i.e. an empty path or a pure
///   root (`"C:\\"` on Windows, `"/"` on Unix-shaped paths). A **bare
///   filename** like `"foo.exe"` has `Path::parent() == Some("")`, so
///   it still hits WMI; the result is naturally empty because no
///   running Patcher reports `ExecutablePath = "Patcher.exe"`
///   (absolute paths always come back from `Win32_Process`).
///
/// # Errors
///
/// Propagates any [`ProcessError`] from [`find_processes_by_name`].
/// Per-PID [`kill_process`] failures are *intentionally* swallowed
/// (see module "Best-effort kill" section).
///
/// # Async runtime guidance
///
/// Composes [`find_processes_by_name`] (WMI round-trip) and
/// [`kill_process`] (Win32 `OpenProcess`/`TerminateProcess`). Callers
/// on a Tokio runtime should dispatch via
/// [`tokio::task::spawn_blocking`][sb]; the module-level guidance
/// covers the COM-apartment caveat.
///
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn check_and_kill_patcher(game_path: &Path) -> Result<Vec<u32>, ProcessError> {
    check_and_kill_patcher_with(game_path, find_processes_by_name, kill_process)
}

/// Dependency-injected variant of [`check_and_kill_patcher`] used by
/// unit tests. The production call wires [`find_processes_by_name`]
/// and [`kill_process`] in; tests substitute pure closures so they can
/// exercise path-match / best-effort logic without WMI.
///
/// This is the same DI pattern P7 [`check_update_at`][cua] uses for
/// the GitHub releases fetcher.
///
/// [cua]: crate::services::updater::checker::check_update_at
pub fn check_and_kill_patcher_with<F, K>(
    game_path: &Path,
    mut find: F,
    mut kill: K,
) -> Result<Vec<u32>, ProcessError>
where
    F: FnMut(&str) -> Result<Vec<ProcessInfo>, ProcessError>,
    K: FnMut(u32) -> Result<(), ProcessError>,
{
    let Some(parent) = game_path.parent() else {
        return Ok(Vec::new());
    };
    let expected_path = parent.join(PATCHER_EXE_NAME);

    let processes = find(PATCHER_EXE_NAME)?;

    let mut killed = Vec::new();
    for info in processes {
        if matches_expected_path(&info, &expected_path) && kill(info.pid).is_ok() {
            killed.push(info.pid);
        }
    }
    Ok(killed)
}

/// Path comparison used to decide whether a `Patcher.exe` running on
/// the system belongs to *this* game install.
///
/// The comparison is **case-sensitive byte equality**, matching WPF's
/// `process.MainModule.FileName == patherPath`. That's fine in
/// practice because both path strings ultimately come from the same
/// on-disk file registration (the game directory), so drift-in-case is
/// extremely unlikely. Should it happen, the Patcher survives — same
/// failure mode as the WPF original.
fn matches_expected_path(info: &ProcessInfo, expected: &Path) -> bool {
    info.executable_path
        .as_deref()
        .map(|p| p == expected)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    fn make_info(pid: u32, path: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: "Patcher.exe".into(),
            executable_path: path.map(PathBuf::from),
        }
    }

    fn terminate_err(pid: u32) -> ProcessError {
        ProcessError::TerminateProcess {
            pid,
            source: windows::core::Error::from_win32(),
        }
    }

    #[test]
    fn matches_expected_path_exact_match() {
        let info = make_info(1, Some(r"C:\MapleStory\Patcher.exe"));
        let expected = PathBuf::from(r"C:\MapleStory\Patcher.exe");
        assert!(matches_expected_path(&info, &expected));
    }

    #[test]
    fn matches_expected_path_different_directory() {
        let info = make_info(1, Some(r"D:\Other\Patcher.exe"));
        let expected = PathBuf::from(r"C:\MapleStory\Patcher.exe");
        assert!(!matches_expected_path(&info, &expected));
    }

    #[test]
    fn matches_expected_path_none_executable_path_is_false() {
        let info = make_info(1, None);
        let expected = PathBuf::from(r"C:\MapleStory\Patcher.exe");
        assert!(!matches_expected_path(&info, &expected));
    }

    #[test]
    fn game_path_without_parent_returns_empty() {
        let killed = RefCell::new(Vec::<u32>::new());
        let find_called = RefCell::new(false);

        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            *find_called.borrow_mut() = true;
            Ok(vec![])
        };
        let kill = |pid: u32| -> Result<(), ProcessError> {
            killed.borrow_mut().push(pid);
            Ok(())
        };

        let result = check_and_kill_patcher_with(Path::new("/"), find, kill).expect("ok");
        assert!(result.is_empty());
        assert!(
            !*find_called.borrow(),
            "short-circuit: find must NOT be called when game_path has no parent"
        );
    }

    #[test]
    fn kills_only_matching_processes() {
        let expected_game = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Ok(vec![
                make_info(100, Some(r"C:\MapleStory\Patcher.exe")), // match
                make_info(200, Some(r"D:\Other\Patcher.exe")),      // not match
                make_info(300, None),                               // no path
            ])
        };
        let killed = RefCell::new(Vec::<u32>::new());
        let kill = |pid: u32| -> Result<(), ProcessError> {
            killed.borrow_mut().push(pid);
            Ok(())
        };

        let result = check_and_kill_patcher_with(&expected_game, find, kill).expect("ok");
        assert_eq!(result, vec![100]);
        assert_eq!(*killed.borrow(), vec![100]);
    }

    #[test]
    fn best_effort_skips_kill_failures() {
        // pid 100 kill fails; pid 101 kill succeeds. We still return
        // Ok and only list the successful kill.
        let expected_game = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Ok(vec![
                make_info(100, Some(r"C:\MapleStory\Patcher.exe")),
                make_info(101, Some(r"C:\MapleStory\Patcher.exe")),
            ])
        };
        let kill = |pid: u32| -> Result<(), ProcessError> {
            if pid == 100 {
                Err(terminate_err(pid))
            } else {
                Ok(())
            }
        };

        let result = check_and_kill_patcher_with(&expected_game, find, kill).expect("ok");
        assert_eq!(result, vec![101]);
    }

    #[test]
    fn find_failure_propagates() {
        // We want to prove "any Err from find is propagated as-is".
        // Production `find_processes_by_name` would only ever return
        // WMI-flavoured errors (`WmiInit`/`WmiConnect`/`WmiQuery`),
        // but `wmi::WMIError` has no public builder usable from a
        // unit test. We therefore borrow the easiest-to-construct
        // variant (`OpenProcess`, which carries just
        // `windows::core::Error::from_win32()`) purely as a transport
        // — the assertion below is on propagation, not on semantic
        // correctness of the variant for a find-side failure.
        let expected_game = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Err(ProcessError::OpenProcess {
                pid: 42,
                source: windows::core::Error::from_win32(),
            })
        };
        let kill = |_: u32| -> Result<(), ProcessError> { Ok(()) };

        let err = check_and_kill_patcher_with(&expected_game, find, kill)
            .expect_err("must propagate find failure");
        match err {
            ProcessError::OpenProcess { pid, .. } => assert_eq!(pid, 42),
            other => panic!("expected OpenProcess propagation, got {other:?}"),
        }
    }

    #[test]
    fn empty_process_list_returns_empty_kill_list() {
        let expected_game = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> { Ok(vec![]) };
        let kill = |_: u32| -> Result<(), ProcessError> {
            panic!("kill should not be called when no Patcher is running");
        };
        let result = check_and_kill_patcher_with(&expected_game, find, kill).expect("ok");
        assert!(result.is_empty());
    }
}
