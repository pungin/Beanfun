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
//! | Chunk | Modules                                | Scope                                          |
//! | ----- | -------------------------------------- | ---------------------------------------------- |
//! | 9.1   | [`error`], [`find`], [`kill`]          | WMI query + `OpenProcess` + `TerminateProcess` |
//! | 9.2   | [`patcher`], [`play_page`]             | single-shot helpers; timer driving → P10       |
//! | 9.3   | `post_string`                          | Win32 thin wrappers for auto-paste             |
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
pub mod patcher;
pub mod play_page;

pub use error::ProcessError;
pub use find::{find_processes_by_name, ProcessInfo};
pub use kill::kill_process;
pub use patcher::{check_and_kill_patcher, PATCHER_EXE_NAME};
pub use play_page::{close_play_window, PLAY_WINDOW_CLASS, PLAY_WINDOW_TITLE};

/// UTF-16 encode `s` with a trailing NUL, the shape
/// [`windows::core::PCWSTR`][PCWSTR] expects.
///
/// Private helper shared by the Win32 call sites in this module
/// ([`play_page`], P9.3 `post_string`). Default-private visibility —
/// descendant modules of `services/process` can still reach it via
/// `super::to_wide_null`, but the rest of the crate cannot, which
/// matches the stated "internal to process/" scope. A byte-identical
/// copy already lives in `services/game/locale_remulator.rs`; if a
/// third caller lands we promote both to `services/util/wide.rs` —
/// not before (YAGNI, and avoids the drive-by edit to the P8 module
/// that this chunk's scope does not justify).
///
/// [PCWSTR]: https://microsoft.github.io/windows-docs-rs/doc/windows/core/struct.PCWSTR.html
fn to_wide_null(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod wide_tests {
    use super::to_wide_null;

    #[test]
    fn to_wide_null_terminates_with_zero() {
        let wide = to_wide_null("abc");
        assert_eq!(wide, vec![b'a' as u16, b'b' as u16, b'c' as u16, 0]);
    }

    #[test]
    fn to_wide_null_empty_string_is_just_nul() {
        assert_eq!(to_wide_null(""), vec![0u16]);
    }
}
