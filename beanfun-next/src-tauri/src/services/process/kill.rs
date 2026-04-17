//! Kill a Windows process by PID via Win32 `OpenProcess` +
//! `TerminateProcess`.
//!
//! # WPF parity
//!
//! WPF reaches for `Process.GetProcessById(pid).Kill()`
//! (`MainWindow.xaml.cs` L1823-1831 and `checkPatcher_Tick` L2464-2475).
//! Under the hood `.NET` calls `OpenProcess(PROCESS_TERMINATE, ...)` +
//! `TerminateProcess(handle, -1)` and then `CloseHandle`. The Rust
//! `std` does not expose a kill-by-external-PID shortcut, so we call
//! the same three Win32 primitives directly via the `windows` crate.
//!
//! # Exit code semantics
//!
//! .NET `Process.Kill()` passes `-1` (i.e. `0xFFFFFFFF` cast to
//! DWORD) as the terminate-process exit code. The deliberate choice
//! of "largest unsigned DWORD" is observability: downstream waitors
//! (`WaitForSingleObject` → `GetExitCodeProcess`, `.NET
//! Process.ExitCode`, shell `%ERRORLEVEL%`, …) can tell
//! "terminated externally" apart from "exited normally with code
//! 1" by reading the exit value. We pass the bit-equivalent
//! [`u32::MAX`] to preserve that parity so a P10 command that reads
//! a zombie's exit code gets the same answer it would have got
//! under WPF.
//!
//! # TOCTOU note
//!
//! Between `OpenProcess` returning a handle and `TerminateProcess`
//! running, the kernel has already pinned the process object in memory
//! (the handle is a refcount). So even if the target process exits
//! naturally in that window, the handle stays valid and
//! `TerminateProcess` no-ops against the zombie — it does not return
//! an error for "already dead". We close the handle in all paths to
//! avoid leaking the refcount.

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

use super::error::ProcessError;

/// Terminate the process identified by `pid`.
///
/// # Errors
///
/// [`ProcessError::OpenProcess`] if the PID no longer exists, the
/// caller lacks permission, or `pid` names a protected process
/// (`System`, `csrss.exe`, etc.).
///
/// [`ProcessError::TerminateProcess`] if `OpenProcess` succeeded but
/// `TerminateProcess` was refused — uncommon; the most likely cause
/// is a critical-process mark
/// ([`RtlSetProcessIsCritical`](https://learn.microsoft.com/en-us/previous-versions/windows/embedded/ms891580\(v=msdn.10\))).
///
/// # Guarantees
///
/// On `Ok(())` the kernel has accepted the terminate request. The
/// target process may still be running for a few milliseconds while
/// the OS tears down its threads — callers that need a hard "it is
/// gone" guarantee should poll for exit (e.g. [`std::process::Child::try_wait`]
/// or a fresh [`super::find::find_processes_by_name`] call).
///
/// # Async runtime guidance
///
/// Callers running on a Tokio (or any async) runtime should dispatch
/// this via [`tokio::task::spawn_blocking`][spawn_blocking] rather
/// than calling it directly from an `async fn`. `OpenProcess` and
/// `TerminateProcess` are sync Win32 calls that can block for a
/// handful of milliseconds on a contended system, which is enough
/// to starve neighbouring tasks on Tokio's single-threaded
/// scheduler (and is **not allowed** at all from the
/// `current_thread` runtime flavor).
///
/// [spawn_blocking]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn kill_process(pid: u32) -> Result<(), ProcessError> {
    // Safety: OpenProcess is a Win32 FFI call; the parameters we pass
    // (PROCESS_TERMINATE access mask, no handle inheritance, plain u32
    // pid) are all copied by value. The returned HANDLE has ownership
    // semantics and must be closed exactly once in every path — see
    // the matched `close` block below.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }
        .map_err(|source| ProcessError::OpenProcess { pid, source })?;

    // Safety: `handle` was just produced by OpenProcess and has not
    // been touched. Exit code [`u32::MAX`] mirrors .NET
    // `Process.Kill()` — see module-level "Exit code semantics".
    let terminate_result = unsafe { TerminateProcess(handle, u32::MAX) };

    // Safety: the handle came from OpenProcess and is still live (the
    // kernel doesn't invalidate it on TerminateProcess). We close it
    // exactly once regardless of whether TerminateProcess succeeded,
    // which is the contract in MSDN's sample code.
    let _ = unsafe { CloseHandle(handle) };

    terminate_result.map_err(|source| ProcessError::TerminateProcess { pid, source })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// OpenProcess on PID 0 (System Idle Process) is documented to
    /// fail with ERROR_INVALID_PARAMETER. Asserts the error mapping
    /// stays on the OpenProcess variant (not TerminateProcess).
    #[test]
    fn kill_pid_zero_errors_on_open_not_terminate() {
        let err = kill_process(0).expect_err("kill_process(0) should error");
        match err {
            ProcessError::OpenProcess { pid, .. } => assert_eq!(pid, 0),
            other => panic!("expected OpenProcess error, got {other:?}"),
        }
    }

    /// A PID above 0xFFFF_FFF0 has never been seen on Windows in
    /// practice (the kernel hands out PIDs in small increments). Used
    /// as a "definitely doesn't exist" probe.
    #[test]
    fn kill_implausible_pid_errors_on_open() {
        let err = kill_process(0xFFFF_FFF0).expect_err("must error");
        match err {
            ProcessError::OpenProcess { pid, .. } => assert_eq!(pid, 0xFFFF_FFF0),
            other => panic!("expected OpenProcess error, got {other:?}"),
        }
    }
}
