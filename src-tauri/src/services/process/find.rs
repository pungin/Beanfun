//! WMI-backed enumeration of running Windows processes.
//!
//! Ports `MainWindow::runGame` L1724-1812's
//! `ManagementObjectSearcher("select * from Win32_Process where ProcessId = <pid>")`
//! loop, but folds the two-step pattern (list by name via
//! `Process.GetProcessesByName`, then WMI-lookup per PID) into one
//! WMI query that already filters on `Name`. Net: one round-trip to
//! WMI instead of N+1 round-trips.
//!
//! # Name semantics
//!
//! - .NET `Process.GetProcessesByName("Patcher")` matches by the
//!   **module name without extension** (case-insensitive).
//! - WMI `Win32_Process.Name` stores the **executable file name
//!   including extension** (e.g. `Patcher.exe`), and WQL string
//!   comparisons are case-**insensitive**.
//!
//! To stay explicit, the public API on this module takes the process
//! name **with extension** (`"Patcher.exe"`, `"MapleStory.exe"`). The
//! doc-note on [`find_processes_by_name`] calls this out so callers
//! port WPF strings correctly.
//!
//! # `ExecutablePath` caveat
//!
//! `Win32_Process.ExecutablePath` is `nullable` in WMI — on protected
//! system processes or race conditions (process exited between listing
//! and property fetch) it can come back `NULL`. The Rust API exposes
//! this honestly as `Option<PathBuf>`; callers that compare paths
//! (`checkPatcher` parity: match Patcher.exe path to the expected
//! `game_dir\Patcher.exe`) must handle `None` explicitly.

use std::path::PathBuf;

use serde::Deserialize;
use wmi::{COMLibrary, WMIConnection};

use super::error::ProcessError;

/// Summary of a running process as returned by [`find_processes_by_name`].
///
/// Only the fields we currently need from `Win32_Process` are mapped.
/// Add more when a downstream consumer lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    /// `Win32_Process.ProcessId` — the OS-level PID. Stable for the
    /// lifetime of the process.
    pub pid: u32,

    /// `Win32_Process.Name` — executable file name including extension
    /// (e.g. `"cmd.exe"`). Mirrors WMI exactly; do not strip the
    /// extension.
    pub name: String,

    /// `Win32_Process.ExecutablePath` — full path to the executable.
    /// `None` when WMI returns `NULL` (protected process, or the
    /// process exited during the query).
    pub executable_path: Option<PathBuf>,
}

/// Raw WMI shape we deserialize from. Kept private; callers receive
/// [`ProcessInfo`] instead of the WMI-specific naming.
#[derive(Debug, Deserialize)]
#[serde(rename = "Win32_Process")]
#[serde(rename_all = "PascalCase")]
struct Win32Process {
    process_id: u32,
    name: String,
    executable_path: Option<String>,
}

/// List every running process whose `Win32_Process.Name` equals
/// `name` (case-insensitive, WQL string comparison rules).
///
/// # Arguments
///
/// * `name` — executable file name **including** extension, e.g.
///   `"Patcher.exe"`, `"MapleStory.exe"`. Single-quote (`'`) in `name`
///   is rejected early to keep the WQL literal safe; the function
///   returns an empty `Vec` for that input rather than raising an
///   error, because the only realistic caller supplies a
///   compile-time constant.
///
/// # Returns
///
/// A `Vec` of [`ProcessInfo`] — empty if no matching process is
/// running, otherwise one entry per match.
///
/// # Errors
///
/// [`ProcessError::WmiInit`] if `CoInitializeEx` rejects our request
/// (typically another apartment mode is active on this thread — see
/// the next section).
/// [`ProcessError::WmiConnect`] if connecting to `root\cimv2` fails.
/// [`ProcessError::WmiQuery`] if WMI rejects the WQL or the query
/// fails mid-stream.
///
/// # COM apartment mode
///
/// [`COMLibrary::new`] internally calls
/// `CoInitializeEx(COINIT_MULTITHREADED)` on the current thread. If
/// that thread has already initialised COM with a different mode
/// (e.g. `COINIT_APARTMENTTHREADED` — which Tauri's WebView2 main
/// thread and any Win32 UI thread use by default), the call returns
/// `RPC_E_CHANGED_MODE` and is surfaced here as
/// [`ProcessError::WmiInit`]. Therefore always run this function on
/// a fresh worker thread — never on the Tauri command-dispatcher
/// main thread. The `spawn_blocking` guidance below covers both
/// this and the blocking-call concern.
///
/// # Async runtime guidance
///
/// Callers on a Tokio (or any async) runtime should dispatch this
/// via [`tokio::task::spawn_blocking`][spawn_blocking]. The WMI
/// round-trip costs tens of milliseconds on a cold COM init and is
/// a hard-blocking call — running it directly from an `async fn`
/// starves neighbouring tasks on the single-threaded scheduler
/// flavor and is disallowed on `current_thread` runtimes.
///
/// [spawn_blocking]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn find_processes_by_name(name: &str) -> Result<Vec<ProcessInfo>, ProcessError> {
    // Reject WQL injection vectors up front. The only expected input is
    // a hard-coded executable name like `"Patcher.exe"`; anything with
    // a single-quote would have to escape it and is almost certainly
    // caller error. Return empty rather than error — semantically "no
    // process matches this bogus name".
    if name.contains('\'') {
        return Ok(Vec::new());
    }

    let com = COMLibrary::new().map_err(ProcessError::WmiInit)?;
    let conn = WMIConnection::new(com).map_err(ProcessError::WmiConnect)?;

    let query =
        format!("SELECT ProcessId, Name, ExecutablePath FROM Win32_Process WHERE Name = '{name}'");

    let rows: Vec<Win32Process> =
        conn.raw_query(&query)
            .map_err(|source| ProcessError::WmiQuery {
                query: query.clone(),
                source,
            })?;

    Ok(rows
        .into_iter()
        .map(|p| ProcessInfo {
            pid: p.process_id,
            name: p.name,
            executable_path: p.executable_path.map(PathBuf::from),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_in_name_returns_empty() {
        // Defense-in-depth: the only realistic caller passes a
        // constant executable name, but a stray single-quote must not
        // become a WQL injection vector.
        let got = find_processes_by_name("foo'; DROP TABLE bar; --").expect("ok");
        assert!(got.is_empty());
    }

    #[test]
    fn process_info_equality_rejects_path_casing_sloppiness() {
        // Explicit check that PathBuf equality is case-sensitive at
        // the byte level — `ProcessInfo` does NOT normalize, callers
        // compare full paths via case-insensitive logic themselves
        // when they need Windows-path parity (checkPatcher).
        let a = ProcessInfo {
            pid: 1,
            name: "cmd.exe".into(),
            executable_path: Some(PathBuf::from(r"C:\Windows\System32\cmd.exe")),
        };
        let b = ProcessInfo {
            pid: 1,
            name: "cmd.exe".into(),
            executable_path: Some(PathBuf::from(r"c:\windows\system32\cmd.exe")),
        };
        assert_ne!(a, b);
    }
}
