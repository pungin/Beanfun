//! Windows process query / kill layer.
//!
//! Ports the process-lifetime calls that WPF interleaves with `runGame`
//! (`Beanfun/MainWindow.xaml.cs` L1724-1831) and the two timer-driven
//! cleanup tasks (`checkPatcher_Tick` L2455-2614, `checkPlayPage_Tick`
//! L2443-2453, **chunk 9.2**). The auto-paste Win32 wrappers
//! (`API/WindowsAPI.cs` + `getOtpWorker_RunWorkerCompleted` L2131-2238,
//! **chunk 9.3**) also land here.
//!
//! # Chunking (P9)
//!
//! | Chunk | Modules                        | Scope                                          |
//! | ----- | ------------------------------ | ---------------------------------------------- |
//! | 9.1   | [`error`], [`find`], [`kill`]  | WMI query + `OpenProcess` + `TerminateProcess` |
//! | 9.2   | `patcher`, `play_page`         | single-shot helpers; timer driving → P10       |
//! | 9.3   | `post_string`                  | Win32 thin wrappers for auto-paste             |
//!
//! # Timer ownership
//!
//! WPF runs both `checkPatcher` and `checkPlayPage` as 100 ms
//! `DispatcherTimer`s wired into the `MainWindow` life-cycle. That
//! timer **does not** live here — `services/process` exposes single
//! pure functions; the P10 command layer uses `tokio::time::interval`
//! (or Tauri's event loop) to drive them. Same reason the version-check
//! branch of `checkPatcher_Tick` is out of scope here: it belongs next
//! to [`crate::services::updater`] or inside P10 commands, not the
//! kill primitive.
//!
//! # Platform
//!
//! The whole module is gated `#[cfg(target_os = "windows")]` at
//! [`crate::services`] — every Win32 API and the `wmi` crate only
//! compile on Windows. Cross-platform unit tests for P5 / P6 / P7 / P8
//! are unaffected.

pub mod error;
pub mod find;
pub mod kill;

pub use error::ProcessError;
pub use find::{find_processes_by_name, ProcessInfo};
pub use kill::kill_process;
