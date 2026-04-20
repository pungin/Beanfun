//! Close the MapleStory launcher dialog (`StartUpDlgClass` +
//! `MapleStory`).
//!
//! # WPF parity
//!
//! Ports `MainWindow::checkPlayPage_Tick`
//! (`Beanfun/MainWindow.xaml.cs` L2443-2453):
//!
//! ```csharp
//! const uint WM_CLOSE = 0x10;
//! IntPtr hWnd;
//! if ((hWnd = WindowsAPI.FindWindow("StartUpDlgClass", "MapleStory"))
//!     != IntPtr.Zero)
//!     WindowsAPI.PostMessage(hWnd, WM_CLOSE, 0, 0);
//! ```
//!
//! WPF wraps the whole body in `try / catch { }`. The Rust version
//! distinguishes three cases instead:
//!
//! - `Ok(false)` — no matching window (common: the launcher dialog
//!   isn't open). This is the WPF `hWnd == IntPtr.Zero` branch.
//! - `Ok(true)` — window found and `WM_CLOSE` posted. Note:
//!   `PostMessageW` is asynchronous — the target window will receive
//!   `WM_CLOSE` on its next message pump cycle, not synchronously.
//!   Callers that need "window actually gone" must poll (same caveat
//!   as P9.1 [`kill_process`][kp]).
//! - `Err(ProcessError::PostMessage)` — window found but posting the
//!   close message failed (the window was destroyed between
//!   `FindWindowW` and `PostMessageW`, or a system-level refusal).
//!   This case is genuinely rare but surfaced — silently swallowing
//!   it here would cost downstream telemetry.
//!
//! [kp]: super::kill::kill_process
//!
//! # Window identity
//!
//! The MapleStory launcher registers itself with:
//!
//! - **class name**: `"StartUpDlgClass"` (fixed by Nexon's launcher
//!   executable)
//! - **window title**: `"MapleStory"` (localized variants like the
//!   Traditional-Chinese `"新楓之谷"` are **not** what this targets —
//!   WPF also looks for the English literal; the launcher's internal
//!   class/title match English regardless of UI language)
//!
//! These are locked in as public consts ([`PLAY_WINDOW_CLASS`] /
//! [`PLAY_WINDOW_TITLE`]) so a future behaviour change (e.g. Nexon
//! renames the class) only needs one patch point, and so tests can
//! assert against the exact literal.
//!
//! # Async runtime guidance
//!
//! [`close_play_window`] calls two sync Win32 entry points; callers on
//! a Tokio runtime should dispatch via
//! [`tokio::task::spawn_blocking`][sb]. Both calls are very cheap
//! (microseconds), but the `current_thread` runtime flavor still
//! disallows sync FFI from within an `async fn`.
//!
//! [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};

use super::error::ProcessError;
use super::to_wide_null;

/// Window class name the MapleStory launcher registers with. Used as
/// the first argument to `FindWindowW`.
pub const PLAY_WINDOW_CLASS: &str = "StartUpDlgClass";

/// Window title the MapleStory launcher displays. Matches WPF's
/// literal; the launcher's native title ignores the UI language
/// selection.
pub const PLAY_WINDOW_TITLE: &str = "MapleStory";

/// If the MapleStory launcher window is open, post `WM_CLOSE` to it.
///
/// # Returns
///
/// - `Ok(true)` — window was present and `WM_CLOSE` was posted
///   successfully (the close is asynchronous — see module docs).
/// - `Ok(false)` — no matching window; nothing to do.
/// - `Err(ProcessError::PostMessage)` — window was present but the
///   `WM_CLOSE` post failed.
///
/// # Errors
///
/// Only [`ProcessError::PostMessage`] after a successful
/// [`FindWindowW`][fw]. `FindWindowW` returning `NULL` (window not
/// found) is treated as `Ok(false)`, matching WPF's
/// `hWnd == IntPtr.Zero` branch.
///
/// [fw]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-findwindoww
///
/// # Async runtime guidance
///
/// Calls two sync Win32 entry points (`FindWindowW` + `PostMessageW`).
/// Callers on a Tokio runtime should dispatch this via
/// [`tokio::task::spawn_blocking`][sb] — both calls are cheap but the
/// `current_thread` runtime flavor disallows sync FFI inside an
/// `async fn` regardless.
///
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn close_play_window() -> Result<bool, ProcessError> {
    let class_wide = to_wide_null(PLAY_WINDOW_CLASS);
    let title_wide = to_wide_null(PLAY_WINDOW_TITLE);

    // Safety: `FindWindowW` copies both PCWSTRs by value; the `Vec<u16>`s
    // backing them stay alive for the whole call. `windows`-0.58 returns
    // `Ok(HWND)` when the window exists, and `Err(_)` when NULL is
    // returned (the crate conflates "not found" with other failures by
    // reading GetLastError). We treat any error as "not found" for WPF
    // parity — `FindWindowW` with literal class + title has effectively
    // no other failure mode on a healthy system.
    let hwnd =
        match unsafe { FindWindowW(PCWSTR(class_wide.as_ptr()), PCWSTR(title_wide.as_ptr())) } {
            Ok(hwnd) if !hwnd.is_invalid() => hwnd,
            Ok(_) | Err(_) => return Ok(false),
        };

    // Safety: `hwnd` is a live window handle we just received from
    // FindWindowW. `PostMessageW` is asynchronous — it enqueues
    // WM_CLOSE and returns without waiting for the window procedure.
    unsafe { PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) }.map_err(|source| {
        ProcessError::PostMessage {
            hwnd: hwnd.0 as usize,
            source,
        }
    })?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_class_literal_matches_wpf() {
        // Guard against accidental rename — WPF's
        // `checkPlayPage_Tick` L2449 hard-codes "StartUpDlgClass",
        // which is what Nexon's launcher EXE uses to register its
        // dialog class. Losing this literal breaks the kill path.
        assert_eq!(PLAY_WINDOW_CLASS, "StartUpDlgClass");
    }

    #[test]
    fn window_title_literal_matches_wpf() {
        assert_eq!(PLAY_WINDOW_TITLE, "MapleStory");
    }
}
