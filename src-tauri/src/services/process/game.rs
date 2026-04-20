//! Game main-process enumerate + terminate helpers.
//!
//! # WPF parity
//!
//! Ports the "is the game already running?" preflight block of
//! `MainWindow::btn_Run_Game_Click` (`Beanfun/MainWindow.xaml.cs`
//! L1765-1833). The WPF source does:
//!
//! ```csharp
//! string gameProcessName = Regex("(.*).exe").Match(game_exe).Groups[1].Value;
//! foreach (Process process in Process.GetProcessesByName(gameProcessName))
//! {
//!     try // 1st attempt: WMI ExecutablePath lookup
//!     {
//!         using var searcher = new ManagementObjectSearcher(
//!             "select * from Win32_Process where ProcessId = " + process.Id);
//!         if (gamePath == objects…["executablepath"]…) { processIds.Add(process.Id); continue; }
//!     } catch { }
//!     try // 2nd attempt: .NET MainModule fallback
//!     {
//!         if (process.MainModule.FileName == gamePath) { processIds.Add(process.Id); continue; }
//!     } catch { }
//! }
//! if (processIds.Count > 0 && MessageBox.Show(MsgGameAlreadyRun) == Yes)
//! {
//!     foreach (int pid in processIds)
//!         try { Process.GetProcessById(pid).Kill(); } catch { }
//! }
//! ```
//!
//! The Rust port replicates the **match semantics** (same exe name,
//! exact path equality) and **best-effort kill** semantics (per-pid
//! swallow) while folding WPF's two `try` blocks into a single WMI
//! query that already exposes `ExecutablePath`. `find_processes_by_name`
//! does the name filter + path lookup in **one** round-trip instead of
//! WPF's N+1 pattern (list by name → per-pid WMI lookup). The
//! `MainModule.FileName` fallback is therefore redundant — it exists in
//! WPF only because the first WMI call can throw on protected-process
//! races, and the single-query path here either succeeds or fails the
//! whole operation deterministically. Callers that need the partial-
//! recovery semantics can retry.
//!
//! # Sibling of [`super::patcher`]
//!
//! This module is the "game main process" analogue of [`super::patcher`]
//! (which kills stray `Patcher.exe`). Both follow the same shape:
//!
//! | Aspect            | [`super::patcher`]                      | this module                                     |
//! | ----------------- | --------------------------------------- | ----------------------------------------------- |
//! | exe name source   | hard-coded `"Patcher.exe"` constant     | derived from `game_path.file_name()`            |
//! | match strategy    | `<game_dir>/Patcher.exe` byte-equal     | `game_path` byte-equal (same rule applied to    |
//! |                   |                                         | the game binary itself)                         |
//! | kill semantics    | best-effort silent-skip                 | best-effort silent-skip                         |
//! | DI for tests      | `_with` suffix pattern                  | same `_with` suffix pattern                     |
//! | WPF source lines  | L2455-2477 (`checkPatcher_Tick`)        | L1765-1833 (preflight in `btn_Run_Game_Click`)  |
//!
//! Keeping them in sibling modules (`patcher` and `game`) rather than
//! merging into a generic `find_matching_processes(exe, path)` keeps
//! the WPF-parity documentation readable — each WPF call site lands in
//! one Rust file with its own module docs, unit tests, and doc-linked
//! `MainWindow.xaml.cs` line numbers.
//!
//! # Best-effort kill
//!
//! [`kill_game_processes`] swallows per-pid [`kill_process`] failures
//! so one unkillable instance (protected mode, race with natural exit,
//! permission denied) doesn't block cleanup of the rest. This mirrors
//! WPF's nested `try { process.Kill(); } catch { }`. The caller
//! receives the list of pids that **did** terminate; absent pids were
//! either already dead or couldn't be killed — the frontend treats
//! both the same way (re-`list_game_processes` and surface any
//! leftover to the user).
//!
//! # Async runtime guidance
//!
//! Both [`find_game_processes`] and [`kill_game_processes`] compose
//! blocking Win32 / WMI primitives — callers on a Tokio runtime
//! should dispatch them via [`tokio::task::spawn_blocking`][sb]. The
//! COM-apartment caveat in [`super::find::find_processes_by_name`]
//! applies here too because it is our only WMI call site.
//!
//! [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use std::path::Path;

use super::error::ProcessError;
use super::find::{find_processes_by_name, ProcessInfo};
use super::kill::kill_process;

/// Locate every running process whose executable matches `game_path`
/// exactly (same exe name + same full path), returning each match as
/// a [`ProcessInfo`].
///
/// # Match rule
///
/// 1. Pull the file-name component off `game_path` (e.g. `"MapleStory.exe"`).
///    If `Path::file_name` returns `None` — i.e. `game_path` is empty or a
///    pure root (`"/"`, `"C:\\"`) — short-circuit to `Ok(Vec::new())`
///    without hitting WMI. This matches the shape of
///    [`super::patcher::check_and_kill_patcher`]'s `parent()` guard: we
///    decline to run the query rather than letting WMI interpret a
///    degenerate name.
/// 2. Call [`find_processes_by_name`] with that exe name.
/// 3. Filter the result by **byte-equal** comparison of
///    [`ProcessInfo::executable_path`] against `game_path`. A `None`
///    executable_path (WMI returned `NULL` — protected process or a
///    race with natural exit) is treated as **no match** and filtered
///    out. This mirrors WPF's per-process `try { ... } catch { }`
///    silently skipping processes that fail the path lookup.
///
/// Byte-equal comparison is deliberately case-sensitive (see
/// [`super::patcher::check_and_kill_patcher`]'s matching helper for the
/// same contract). Both the caller's `game_path` and WMI's
/// `ExecutablePath` come from the same on-disk registration, so drift-
/// in-case is a pathological edge case; if it does happen, the game
/// survives — identical failure mode to WPF.
///
/// # Returns
///
/// A `Vec<ProcessInfo>` — empty if nothing matches. Callers that only
/// need the pid list can `.into_iter().map(|p| p.pid).collect()`; this
/// function returns the full [`ProcessInfo`] so command-layer DTOs can
/// surface the executable path back to the frontend for display
/// ("already running at `C:\\...`").
///
/// # Errors
///
/// Propagates [`ProcessError`] from [`find_processes_by_name`]:
/// [`ProcessError::WmiInit`] / [`ProcessError::WmiConnect`] /
/// [`ProcessError::WmiQuery`]. The early `Ok(Vec::new())` for a
/// file-name-less path does **not** call WMI and therefore cannot
/// produce those errors.
pub fn find_game_processes(game_path: &Path) -> Result<Vec<ProcessInfo>, ProcessError> {
    find_game_processes_with(game_path, find_processes_by_name)
}

/// Dependency-injected variant of [`find_game_processes`] used by
/// unit tests. The production path wires [`find_processes_by_name`];
/// tests substitute a pure closure so they can exercise the file-name
/// extraction + path-equal filter without WMI.
///
/// Follows the same DI pattern as
/// [`super::patcher::check_and_kill_patcher_with`] (chunk 9.2) and
/// [`crate::services::updater::checker::check_update_at`] (chunk 7.2).
pub fn find_game_processes_with<F>(
    game_path: &Path,
    mut find: F,
) -> Result<Vec<ProcessInfo>, ProcessError>
where
    F: FnMut(&str) -> Result<Vec<ProcessInfo>, ProcessError>,
{
    let Some(exe_name) = game_path.file_name().and_then(|n| n.to_str()) else {
        return Ok(Vec::new());
    };

    let processes = find(exe_name)?;

    Ok(processes
        .into_iter()
        .filter(|info| matches_game_path(info, game_path))
        .collect())
}

/// Best-effort terminate every pid in `pids`, returning the subset
/// that was actually killed.
///
/// A per-pid [`kill_process`] failure is silently skipped — see the
/// module-level "Best-effort kill" section for the rationale and the
/// WPF parity note.
///
/// # Returns
///
/// `Vec<u32>` containing only the pids that [`kill_process`]
/// returned `Ok` for. Order matches the input slice (the filter
/// preserves iteration order). An empty input produces an empty
/// output without any kill calls.
pub fn kill_game_processes(pids: &[u32]) -> Vec<u32> {
    kill_game_processes_with(pids, kill_process)
}

/// Dependency-injected variant of [`kill_game_processes`] used by
/// unit tests. The production path wires [`kill_process`]; tests
/// substitute a pure closure so they can exercise the per-pid
/// best-effort logic without needing real OS processes.
pub fn kill_game_processes_with<K>(pids: &[u32], mut kill: K) -> Vec<u32>
where
    K: FnMut(u32) -> Result<(), ProcessError>,
{
    pids.iter()
        .copied()
        .filter(|&pid| kill(pid).is_ok())
        .collect()
}

/// Match predicate used by [`find_game_processes_with`].
///
/// Pure, no IO — lifted out of the closure so it's independently
/// unit-testable and so the equality rule (byte-equal path, `None`
/// filtered out) lives in one place. Mirrors
/// [`super::patcher`]'s private `matches_expected_path` helper.
fn matches_game_path(info: &ProcessInfo, game_path: &Path) -> bool {
    info.executable_path
        .as_deref()
        .map(|p| p == game_path)
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
            name: "MapleStory.exe".into(),
            executable_path: path.map(PathBuf::from),
        }
    }

    fn terminate_err(pid: u32) -> ProcessError {
        ProcessError::TerminateProcess {
            pid,
            source: windows::core::Error::from_win32(),
        }
    }

    // ---- matches_game_path ------------------------------------------------

    #[test]
    fn matches_game_path_exact_match() {
        let info = make_info(1, Some(r"C:\MapleStory\MapleStory.exe"));
        let expected = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        assert!(matches_game_path(&info, &expected));
    }

    #[test]
    fn matches_game_path_different_directory_rejected() {
        let info = make_info(1, Some(r"D:\Other\MapleStory.exe"));
        let expected = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        assert!(!matches_game_path(&info, &expected));
    }

    #[test]
    fn matches_game_path_none_executable_path_is_false() {
        // WMI `ExecutablePath` comes back `NULL` for protected
        // processes or mid-exit races. WPF swallows these via its
        // per-process `try { ... } catch { }`; we mirror the skip by
        // filtering them out here.
        let info = make_info(1, None);
        let expected = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        assert!(!matches_game_path(&info, &expected));
    }

    // ---- find_game_processes_with -----------------------------------------

    #[test]
    fn find_game_processes_with_empty_file_name_short_circuits() {
        // A pure-root path (`/` or `C:\`) has `file_name() == None`
        // — we must skip the WMI query entirely. The sentinel below
        // would otherwise be returned.
        let find_called = RefCell::new(false);
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            *find_called.borrow_mut() = true;
            Ok(vec![make_info(1, Some("does not matter"))])
        };

        let result = find_game_processes_with(Path::new("/"), find).expect("ok");
        assert!(result.is_empty());
        assert!(
            !*find_called.borrow(),
            "find must NOT be called when game_path has no file_name"
        );
    }

    #[test]
    fn find_game_processes_with_all_match_returns_all() {
        let game_path = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Ok(vec![
                make_info(100, Some(r"C:\MapleStory\MapleStory.exe")),
                make_info(200, Some(r"C:\MapleStory\MapleStory.exe")),
            ])
        };

        let result = find_game_processes_with(&game_path, find).expect("ok");
        let pids: Vec<u32> = result.iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![100, 200]);
    }

    #[test]
    fn find_game_processes_with_only_matching_paths_are_kept() {
        // Two process entries share the exe name but live in
        // different directories — only the one whose
        // ExecutablePath byte-equals `game_path` survives the
        // filter. This locks the "same filename in a different
        // install is NOT the same game" rule.
        let game_path = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Ok(vec![
                make_info(100, Some(r"C:\MapleStory\MapleStory.exe")), // match
                make_info(200, Some(r"D:\Other\MapleStory.exe")),      // mismatch
                make_info(300, None),                                  // NULL executable_path
            ])
        };

        let result = find_game_processes_with(&game_path, find).expect("ok");
        let pids: Vec<u32> = result.iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![100]);
    }

    #[test]
    fn find_game_processes_with_passes_exe_name_with_extension() {
        // WMI's `Win32_Process.Name` carries the `.exe` extension;
        // our `find_processes_by_name` takes the WMI name shape.
        // Assert we extract `game_path.file_name()` verbatim (with
        // extension) rather than stripping it like WPF's
        // `Process.GetProcessesByName` expects.
        let seen_name = RefCell::new(String::new());
        let find = |name: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            *seen_name.borrow_mut() = name.to_string();
            Ok(vec![])
        };

        let game_path = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let _ = find_game_processes_with(&game_path, find).expect("ok");
        assert_eq!(*seen_name.borrow(), "MapleStory.exe");
    }

    #[test]
    fn find_game_processes_with_find_error_propagates() {
        // Any `Err` from the injected finder must bubble up
        // unchanged — the command layer relies on the
        // `ProcessError → CommandError` mapping for WMI failures, so
        // we must not swallow them here.
        let game_path = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> {
            Err(ProcessError::OpenProcess {
                pid: 42,
                source: windows::core::Error::from_win32(),
            })
        };

        let err = find_game_processes_with(&game_path, find).expect_err("must propagate");
        match err {
            ProcessError::OpenProcess { pid, .. } => assert_eq!(pid, 42),
            other => panic!("expected OpenProcess propagation, got {other:?}"),
        }
    }

    #[test]
    fn find_game_processes_with_empty_result_is_empty_vec() {
        let game_path = PathBuf::from(r"C:\MapleStory\MapleStory.exe");
        let find = |_: &str| -> Result<Vec<ProcessInfo>, ProcessError> { Ok(vec![]) };

        let result = find_game_processes_with(&game_path, find).expect("ok");
        assert!(result.is_empty());
    }

    // ---- kill_game_processes_with -----------------------------------------

    #[test]
    fn kill_game_processes_with_empty_pids_skips_kill_call() {
        let kill_called = RefCell::new(Vec::<u32>::new());
        let kill = |pid: u32| -> Result<(), ProcessError> {
            kill_called.borrow_mut().push(pid);
            Ok(())
        };

        let result = kill_game_processes_with(&[], kill);
        assert!(result.is_empty());
        assert!(
            kill_called.borrow().is_empty(),
            "kill must not be called for an empty pid list"
        );
    }

    #[test]
    fn kill_game_processes_with_all_success_returns_all_pids() {
        let seen = RefCell::new(Vec::<u32>::new());
        let kill = |pid: u32| -> Result<(), ProcessError> {
            seen.borrow_mut().push(pid);
            Ok(())
        };

        let result = kill_game_processes_with(&[100, 200, 300], kill);
        assert_eq!(result, vec![100, 200, 300]);
        assert_eq!(*seen.borrow(), vec![100, 200, 300]);
    }

    #[test]
    fn kill_game_processes_with_partial_failure_returns_only_successful_pids() {
        // pid 200 fails; the other two succeed. WPF's nested
        // `try { ... } catch { }` swallows the failure and
        // continues — we mirror that by returning `[100, 300]`
        // without surfacing an error.
        let kill = |pid: u32| -> Result<(), ProcessError> {
            if pid == 200 {
                Err(terminate_err(pid))
            } else {
                Ok(())
            }
        };

        let result = kill_game_processes_with(&[100, 200, 300], kill);
        assert_eq!(result, vec![100, 300]);
    }

    #[test]
    fn kill_game_processes_with_all_failures_returns_empty_vec() {
        // Every pid fails — the whole `Vec` is filtered away. This
        // is not an error: the caller learns about it by receiving
        // an empty `Vec` instead of the list they passed in, then
        // re-`list_game_processes` to surface the leftovers.
        let kill = |pid: u32| -> Result<(), ProcessError> { Err(terminate_err(pid)) };

        let result = kill_game_processes_with(&[100, 200], kill);
        assert!(result.is_empty());
    }

    #[test]
    fn kill_game_processes_with_preserves_input_order() {
        // Regression guard: a future refactor might swap the
        // iterator for a `HashSet` or parallel iterator and
        // accidentally reorder pids. Preserve input order so
        // frontend UI lists can be matched up by index if needed.
        let kill = |_: u32| -> Result<(), ProcessError> { Ok(()) };
        let result = kill_game_processes_with(&[300, 100, 200], kill);
        assert_eq!(result, vec![300, 100, 200]);
    }
}
