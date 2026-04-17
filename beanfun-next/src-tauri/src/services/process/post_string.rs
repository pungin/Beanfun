//! Win32 thin wrappers driving Beanfun's auto-paste flow.
//!
//! Ports the subset of `Beanfun/API/WindowsAPI.cs` (L11-86) that
//! `MainWindow::getOtpWorker_RunWorkerCompleted`
//! (`Beanfun/MainWindow.xaml.cs` L2131-2238) actually drives — the
//! credential synthesizer that types the account / password into the
//! MapleStory launcher's login dialog after Beanfun returns the OTP.
//!
//! # WPF mapping
//!
//! | `WindowsAPI.cs`            | This module                                                            |
//! | -------------------------- | ---------------------------------------------------------------------- |
//! | L11 `FindWindow`           | [`find_window`]                                                        |
//! | L17 `SetForegroundWindow`  | [`set_foreground_window`]                                              |
//! | L20 `MapVirtualKey`        | internal — used by `compute_post_key_lparam`                           |
//! | L22-30 `PostString`        | [`post_string`] — *diverges*: surfaces non-ASCII as `Err` (Q3)         |
//! | L32-35 `PostKey`           | [`post_key`] — *diverges*: lParam bit layout fixed to Win32 spec (Q2)  |
//! | L38 `PostMessage`          | [`post_message_raw`] for non-`WM_CHAR` / non-`WM_KEYDOWN` call sites   |
//! | L41 `ClientToScreen`       | [`client_to_screen`]                                                   |
//! | L44 `GetCursorPos`         | [`get_cursor_pos`]                                                     |
//! | L47 `SetCursorPos`         | [`set_cursor_pos`]                                                     |
//! | L73-86 `GetClientAreaSize` | [`get_client_area_size`] (RECT distilled to [`Size`] internally)       |
//!
//! # Out of scope
//!
//! `WindowsAPI.cs` defines several Win32 wrappers that are **not**
//! part of the auto-paste flow and intentionally do not live here.
//! Each one is either subsumed by Tauri's higher-level surface or
//! belongs to a future, unrelated module:
//!
//! | `WindowsAPI.cs`                                          | Why excluded                                            |
//! | -------------------------------------------------------- | ------------------------------------------------------- |
//! | L14 `GetWindowThreadProcessId`                           | unused by P9 scope; future `services/process/info.rs`   |
//! | L50 / 53 `GetWindowLong` / `SetWindowLong`               | sysmenu / window-style UI chrome, Tauri owns it         |
//! | L55-119 `SetWindowCompositionAttribute` + `AccentPolicy` | acrylic blur — CSS / `tauri-plugin-window-vibrancy`     |
//! | L122-138 `GetSystemDefaultLocaleName`                    | already in P8.1 `services::game::launcher`              |
//! | L141 `GetCurrentProcess`                                 | Tauri / `std::process` handles                          |
//! | L144-150 `GetModuleHandle` / `GetProcAddress`            | dynamic loading not needed by current scope             |
//! | L154 `IsWow64Process`                                    | bitness detection not in P9 scope                       |
//! | L157 `AttachConsole`                                     | Tauri owns stdio                                        |
//! | L171-174 `GetBinaryType`                                 | unused                                                  |
//! | L176-205 `dwMapFlags` + `LCMapStringW`                   | already in P8.1 `services::game::launcher`              |
//!
//! # Design decisions (chunk 9.3 pre-flight)
//!
//! - **Q1 — Scope**: paste-only. The call sites driven by
//!   `MainWindow.xaml.cs` L2131-2238 are the entire surface.
//! - **Q2 — `PostKey` lParam**: corrected to Win32 spec
//!   (`(scan_code << 16) | 1`). WPF's `<< 16 + 1` is a C# operator-
//!   precedence accident — see `compute_post_key_lparam` (private).
//! - **Q3 — Non-ASCII in `post_string`**: surfaces
//!   [`super::ProcessError::NonAscii`] instead of WPF's silent `'?'`
//!   replacement — credential corruption deserves a loud failure.
//! - **Q4 — `WindowHandle`**: type-safe non-null newtype around
//!   `NonZeroUsize`. Construction is gated to this crate so external
//!   callers cannot pass `NULL` HWND into any `pub` function — the
//!   `if (hWnd != IntPtr.Zero)` guard WPF writes by hand
//!   (`MainWindow.xaml.cs` L2158, L2449) is enforced at the type
//!   level.
//! - **Q5 — `Point` / `Size`**: domain newtypes (not Win32 `POINT`
//!   re-exports). The `windows` crate types stay confined to this
//!   module so the Tauri command layer (P10) and the rest of the
//!   crate see only domain shapes.
//! - **Q6 — Chunking**: a single P9.3 commit (this file).
//! - **Q7 — Tests**: medium baseline (unit tests in this module +
//!   integration smoke against `Shell_TrayWnd`); a `#[ignore]`-d
//!   notepad spawn smoke verifies wiring end-to-end without
//!   requiring read-back of synthesised input.
//!
//! # Error surface — must-succeed vs best-effort
//!
//! The module deliberately uses two error shapes for Win32 calls:
//!
//! - **Must-succeed** → `Result<T, ProcessError>`:
//!   [`get_client_area_size`], [`client_to_screen`], [`post_string`],
//!   [`post_key`], [`post_message_raw`]. These either describe
//!   credential transmission (data-loss consequences) or position the
//!   synthetic click (geometry-loss consequences). Failures are
//!   surfaced so the P10 caller can re-find the window, warn the
//!   user, or back off.
//! - **Best-effort** → `Option` / `bool`: [`find_window`] (no
//!   distinguishable error from "not found"), [`get_cursor_pos`],
//!   [`set_cursor_pos`], [`set_foreground_window`]. Failures are
//!   either ambiguous (find_window's `NULL`) or cosmetic (the cursor
//!   doesn't restore, the window doesn't pop to front) and recovery
//!   is "do nothing different". Mirrors the WPF call sites that
//!   ignore the return value.
//!
//! # Async runtime guidance
//!
//! Every `pub` function in this module performs synchronous Win32
//! FFI. Each per-function doc repeats the rule, but the gist:
//! callers on a Tokio runtime should dispatch via
//! [`tokio::task::spawn_blocking`][sb] — the `current_thread`
//! flavor disallows sync FFI inside `async fn` regardless of
//! per-call cost (which is microseconds). Cumulative latency for
//! the full auto-paste sequence (~10 PostMessage roundtrips +
//! cursor move) is under a millisecond on commodity hardware.
//!
//! [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetClientRect, GetCursorPos, PostMessageW, SetCursorPos, SetForegroundWindow,
    WM_CHAR,
};

use super::error::ProcessError;
use super::to_wide_null;

/// Type-safe non-null Win32 window handle.
///
/// Construction is gated to this crate: external callers obtain a
/// `WindowHandle` only by way of [`find_window`] (which returns
/// `Option<WindowHandle>` — `None` for "no such window"). This makes
/// it structurally impossible to pass a `NULL` HWND into any of the
/// `pub` functions in this module that demand one — the entire family
/// of "PostMessage to NULL silently does nothing" bugs that WPF guards
/// against with hand-rolled `if (hWnd != IntPtr.Zero)` checks
/// (`MainWindow.xaml.cs` L2158, L2449) becomes a compile-time
/// invariant here.
///
/// The internal representation is [`NonZeroUsize`]: HWND is
/// pointer-sized and opaque, semantically never zero (NULL signals
/// failure, never a valid handle), and `usize` matches the
/// `ProcessError::PostMessage::hwnd` shape decided in P9.2 R9.2-2.
//
// Hash is added because Point/Size derive it for symmetry; tests
// occasionally want to assert on a `HashSet<WindowHandle>`. There's
// no per-handle ordering that makes sense, so `PartialOrd`/`Ord`
// is intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(NonZeroUsize);

impl WindowHandle {
    /// Wrap a raw Win32 [`HWND`] in a non-null `WindowHandle`.
    /// Returns `None` if `hwnd` is `NULL` (the value Win32 reserves
    /// for "no such window").
    ///
    /// `pub(crate)` so the type-safety invariant — "you can only get
    /// one of these from a successful `find_window`" — is enforced at
    /// the module boundary. The crate-internal escape hatch lets
    /// other `services/process` submodules construct handles in the
    /// rare cases they need to (e.g. wrapping handles obtained from
    /// other Win32 APIs in future chunks).
    pub(crate) fn from_raw(hwnd: HWND) -> Option<Self> {
        NonZeroUsize::new(hwnd.0 as usize).map(Self)
    }

    /// Reconstruct the raw [`HWND`] for handing back to a Win32
    /// function. `pub(crate)` for the same reason as
    /// [`Self::from_raw`].
    pub(crate) fn as_hwnd(self) -> HWND {
        HWND(self.0.get() as *mut _)
    }

    /// The underlying handle value as a `usize`, suitable for logging
    /// (typically formatted `{:#x}`) or serializing across the Tauri
    /// IPC boundary. One-way: a caller that obtains a `usize` cannot
    /// reconstruct a `WindowHandle` from it, so the invariant
    /// guarded by `Self::from_raw` (private) is not weakened.
    pub fn as_raw(self) -> usize {
        self.0.get()
    }
}

/// 2-D screen / client point, mirroring Win32 `POINT` but kept
/// independent of the `windows` crate's type so callers — including
/// the eventual P10 Tauri command layer — see only the domain shape.
///
/// `i32` matches Win32's `LONG`; values can be negative on
/// multi-monitor setups where a secondary display sits to the left
/// or above the primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Window or client-area dimensions.
///
/// Mirrors WPF's choice in `WindowsAPI.cs` L73-86 (`GetClientAreaSize`
/// distills `RECT` into `System.Drawing.Size`): callers never need
/// the four corners of `RECT`, only `width` × `height`.
///
/// `i32` matches Win32's `LONG`. Values are always non-negative in
/// practice (a window with negative width is a contradiction in
/// terms), but `i32` keeps round-trip arithmetic with `Point`
/// straightforward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

// ---------------------------------------------------------------------------
// Window discovery (D4)
// ---------------------------------------------------------------------------

/// Locate a top-level window by class name, window title, or both.
///
/// Wraps [`FindWindowW`][fw]. `class` and `title` are independently
/// optional: passing `None` for either yields a `NULL` `lpClassName` /
/// `lpWindowName` to Win32, instructing it to ignore that criterion.
/// At least one should normally be supplied; passing `None` for both
/// returns the first top-level window in the system, which is rarely
/// useful (matches the underlying Win32 behavior).
///
/// # Returns
///
/// `Some(WindowHandle)` if a matching top-level window is found;
/// `None` otherwise. `FindWindowW` exposes no distinguishable error
/// state — both "no window" and "internal failure" surface as the
/// same `NULL` HWND, and the `windows` crate folds the latter into
/// an `Err`. We collapse both to `None` for symmetry with WPF, where
/// the call site simply tests `hWnd == IntPtr.Zero`
/// (`MainWindow.xaml.cs` L2158, L2449). Callers who need richer
/// diagnostics should reach for `GetLastError` directly.
///
/// # Async runtime guidance
///
/// Synchronous Win32 call. Cheap (microseconds), but callers on a
/// Tokio runtime should still dispatch via
/// [`tokio::task::spawn_blocking`][sb] — the `current_thread` flavor
/// disallows sync FFI inside an `async fn` regardless of cost.
///
/// [fw]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-findwindoww
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn find_window(class: Option<&str>, title: Option<&str>) -> Option<WindowHandle> {
    let class_wide = class.map(to_wide_null);
    let title_wide = title.map(to_wide_null);

    let class_pcwstr = class_wide
        .as_deref()
        .map_or(PCWSTR::null(), |w| PCWSTR(w.as_ptr()));
    let title_pcwstr = title_wide
        .as_deref()
        .map_or(PCWSTR::null(), |w| PCWSTR(w.as_ptr()));

    // Safety: `FindWindowW` reads the two PCWSTRs by value during the
    // call; the backing `Vec<u16>`s are bound by `let` and outlive the
    // unsafe block. `windows`-0.58 returns `Ok(NULL HWND)` or `Err(_)`
    // when no window matches; both collapse to `None` (see fn doc).
    match unsafe { FindWindowW(class_pcwstr, title_pcwstr) } {
        Ok(hwnd) => WindowHandle::from_raw(hwnd),
        Err(_) => None,
    }
}

/// Bring `handle`'s window to the foreground.
///
/// Wraps [`SetForegroundWindow`][sfw]. Returns the underlying Win32
/// `BOOL` as `bool` — `true` if the foreground was actually changed,
/// `false` if Windows refused (most often because the calling thread
/// is not the foreground thread, the target process did not call
/// `AllowSetForegroundWindow`, or focus stealing is disabled).
/// **This `false` return is not an error** in the `Result` sense —
/// it's a routine outcome that callers may decide to ignore, retry,
/// or surface to the user; matches WPF L17's plain `bool` signature
/// and the L2193 call site that swallows the result.
///
/// # Async runtime guidance
///
/// Synchronous Win32 call. See [`find_window`] notes — wrap in
/// [`tokio::task::spawn_blocking`][sb] when called from `async fn`.
///
/// [sfw]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn set_foreground_window(handle: WindowHandle) -> bool {
    // Safety: `handle.as_hwnd()` produces a non-null HWND (the
    // `WindowHandle` type guarantees `NonZeroUsize`). Win32 may still
    // reject the call (returning FALSE), but the input is well-formed.
    unsafe { SetForegroundWindow(handle.as_hwnd()) }.as_bool()
}

// ---------------------------------------------------------------------------
// Geometry (D5)
// ---------------------------------------------------------------------------

/// Width × height of `handle`'s client area (i.e. the window minus its
/// frame and decorations).
///
/// Wraps [`GetClientRect`][gcr] and distills the resulting `RECT`
/// into a [`Size`], mirroring WPF's `WindowsAPI.GetClientAreaSize`
/// (`WindowsAPI.cs` L73-86) but without WPF's silent `Size.Empty`
/// fallback. `RECT` is intentionally not exposed to callers — the
/// auto-paste flow only ever needs `width × height` for click
/// positioning (`MainWindow.xaml.cs` L2181 → L2204), never the four
/// corners.
///
/// # Errors
///
/// Returns [`ProcessError::Win32Call`] (with `name = "GetClientRect"`)
/// when Win32 reports failure — typically because `handle` was
/// destroyed between the [`find_window`] call and now.
///
/// # Async runtime guidance
///
/// Synchronous Win32 call; wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [gcr]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn get_client_area_size(handle: WindowHandle) -> Result<Size, ProcessError> {
    let mut rect = RECT::default();
    // Safety: `&mut rect` is a valid pointer to a stack-local `RECT`
    // for the duration of the call; `handle.as_hwnd()` is non-null.
    unsafe { GetClientRect(handle.as_hwnd(), &mut rect) }.map_err(|source| {
        ProcessError::Win32Call {
            name: "GetClientRect",
            source,
        }
    })?;
    Ok(Size {
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}

/// Convert a client-area point to screen coordinates relative to
/// `handle`.
///
/// Wraps [`ClientToScreen`][cts]. `point` is treated as an
/// (x, y) offset from `handle`'s upper-left client corner;
/// the returned `Point` is the equivalent screen-space position.
/// Used by the auto-paste flow (`MainWindow.xaml.cs` L2204) to
/// translate a click target into the absolute screen coordinates
/// `SetCursorPos` expects.
///
/// # Errors
///
/// Returns [`ProcessError::Win32Call`] (with
/// `name = "ClientToScreen"`) on Win32 failure (typically `handle`
/// destroyed between [`find_window`] and now).
///
/// # Async runtime guidance
///
/// Synchronous Win32 call; wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [cts]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-clienttoscreen
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn client_to_screen(handle: WindowHandle, point: Point) -> Result<Point, ProcessError> {
    let mut win_point = POINT {
        x: point.x,
        y: point.y,
    };
    // Safety: `&mut win_point` is a valid pointer to a stack-local
    // `POINT` for the duration of the call; `handle.as_hwnd()` is
    // non-null. `ClientToScreen` returns `BOOL` (not `Result`) — we
    // synthesise the `windows::core::Error` from `GetLastError` when
    // the call returns FALSE.
    let ok = unsafe { ClientToScreen(handle.as_hwnd(), &mut win_point) }.as_bool();
    if !ok {
        return Err(ProcessError::Win32Call {
            name: "ClientToScreen",
            source: windows::core::Error::from_win32(),
        });
    }
    Ok(Point {
        x: win_point.x,
        y: win_point.y,
    })
}

// ---------------------------------------------------------------------------
// Cursor (D6)
// ---------------------------------------------------------------------------

/// Current cursor position in screen coordinates.
///
/// Wraps [`GetCursorPos`][gcp]. Returns `None` if Win32 reports
/// failure — best-effort by design (`MainWindow.xaml.cs` L2202 saves
/// the cursor before the synthetic click and L2216 restores it; if
/// the save fails, the restore simply doesn't happen, which is the
/// least-surprising outcome). This is the deliberate asymmetry with
/// [`get_client_area_size`] / [`client_to_screen`] — those failures
/// would mis-position the click and warrant surfacing; cursor
/// save/restore failures are cosmetic.
///
/// # Async runtime guidance
///
/// Synchronous Win32 call; wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [gcp]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getcursorpos
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn get_cursor_pos() -> Option<Point> {
    let mut point = POINT::default();
    // Safety: `&mut point` is a valid pointer to a stack-local `POINT`
    // for the duration of the call. `GetCursorPos` may legitimately
    // fail (e.g. session-locked desktop, restricted secure desktop) —
    // we collapse that to `None` for the WPF-mirroring best-effort
    // semantics.
    unsafe { GetCursorPos(&mut point) }.ok().map(|()| Point {
        x: point.x,
        y: point.y,
    })
}

/// Move the cursor to `point` (screen coordinates).
///
/// Wraps [`SetCursorPos`][scp]. Returns `true` if Win32 accepted the
/// move, `false` otherwise (mirrors [`set_foreground_window`]'s
/// signature for the same reason: cursor placement is a routine
/// best-effort operation, not an error condition deserving of
/// `Result`).
///
/// # Async runtime guidance
///
/// Synchronous Win32 call; wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [scp]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcursorpos
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn set_cursor_pos(point: Point) -> bool {
    // Safety: scalar args; no pointer aliasing concerns.
    unsafe { SetCursorPos(point.x, point.y) }.is_ok()
}

// ---------------------------------------------------------------------------
// Auto-paste: text emission (D7)
// ---------------------------------------------------------------------------

/// Type `s` into `handle`'s focused control as a sequence of `WM_CHAR`
/// messages, one per ASCII byte.
///
/// Mirrors WPF's [`PostString`][ps] (`WindowsAPI.cs` L22-30) — the
/// auto-paste credential entry path
/// (`MainWindow.xaml.cs` L2225 / L2235).
///
/// # Pre-validation
///
/// `s` is fully validated for ASCII before any `PostMessageW` call.
/// On the first non-ASCII codepoint (in iteration order over
/// [`str::char_indices`]), the function returns
/// [`ProcessError::NonAscii`] and **no `WM_CHAR` is sent**. This
/// matches WPF's flow shape — `ASCIIEncoding.GetBytes(input)` does
/// the full string conversion before the transmission loop begins —
/// while diverging in the *content* policy: WPF silently rewrites
/// non-ASCII to `'?'` (so the message proceeds with garbage); this
/// crate refuses (Q3=C1, P9.3 pre-flight). Half-typed credentials
/// would force the user to manually backspace and retry.
///
/// `PostMessageW` failures mid-transmission are *not* rolled back —
/// once a byte has been queued for the target's message pump, it
/// cannot be unsent. Such failures are systemic (target window
/// destroyed mid-paste, kernel resource exhaustion) and roll-back
/// would require synthesising backspaces for every prior byte, which
/// has its own corruption modes.
///
/// # Errors
///
/// - [`ProcessError::NonAscii`] if any character is outside
///   `0x00..=0x7F`. No bytes are sent.
/// - [`ProcessError::PostMessage`] on the first `PostMessageW`
///   failure. Bytes preceding the failure may already have been
///   queued (see Pre-validation above).
///
/// # Async runtime guidance
///
/// Synchronous Win32 calls (one per byte). The total cost is on the
/// order of microseconds for typical credential-length strings, but
/// callers on a Tokio runtime should still wrap in
/// [`tokio::task::spawn_blocking`][sb] — the `current_thread` flavor
/// disallows sync FFI inside an `async fn` regardless of cost.
///
/// [ps]: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-char
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn post_string(handle: WindowHandle, s: &str) -> Result<(), ProcessError> {
    if let Some((offset, ch)) = s.char_indices().find(|(_, c)| !c.is_ascii()) {
        return Err(ProcessError::NonAscii { offset, ch });
    }

    for byte in s.bytes() {
        // Safety: `handle.as_hwnd()` is non-null (newtype invariant).
        // `PostMessageW` is asynchronous — it enqueues the message
        // and returns; we propagate any synchronous queueing failure.
        unsafe { PostMessageW(handle.as_hwnd(), WM_CHAR, WPARAM(byte as usize), LPARAM(0)) }
            .map_err(|source| ProcessError::PostMessage {
                hwnd: handle.as_raw(),
                source,
            })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Auto-paste: key emission (D8)
// ---------------------------------------------------------------------------

/// Compose the `lParam` for a single `WM_KEYDOWN` / `WM_KEYUP` press.
///
/// Per the [Win32 keyboard-message spec][spec], `lParam` packs:
///
/// - bits 0..16 — repeat count (we always emit `1`)
/// - bits 16..24 — scan code from `MapVirtualKeyW(vk, MAPVK_VK_TO_VSC)`
/// - bits 24..32 — extended-key / context / previous-state /
///   transition flags (all `0` for a single fresh keydown)
///
/// # Divergence from WPF
///
/// WPF's `WindowsAPI.cs:34` writes
/// `MapVirtualKey(wParam, 0) << 16 + 1`. C#'s operator precedence
/// puts `+` above `<<`, so the expression actually evaluates as
/// `MapVirtualKey(...) << 17`, with a repeat count of `0`. This is a
/// genuine bug in the WPF source, not a deliberate design — Q2 of
/// the P9.3 pre-flight elected to fix it. MapleStory dispatches on
/// `wParam` (the VK) and ignores `lParam` scan-code bits in
/// standard input controls, so the WPF bug is invisible at runtime;
/// emitting the spec-correct shape avoids propagating a bit-twiddling
/// trap into the Rust port.
///
/// Extracted as a private helper so D10 unit tests can verify the bit
/// layout against known scan codes without touching real Win32 state.
///
/// [spec]: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-keydown
fn compute_post_key_lparam(vk: u8) -> isize {
    // Safety: `MapVirtualKeyW` is a pure function of its scalar args;
    // no pointer dereferences, no thread-local state mutations.
    let scan_code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as isize;
    (scan_code << 16) | 1
}

/// Post a single `WM_KEYDOWN` / `WM_KEYUP` for virtual-key `vk` to
/// `handle`.
///
/// Mirrors WPF's [`PostKey`][wpf] (`WindowsAPI.cs` L32-35) used
/// throughout the auto-paste flow — `WM_KEYDOWN` for `VK_ESCAPE` /
/// `VK_END` / `VK_BACK` / `VK_TAB` / `VK_RETURN`
/// (`MainWindow.xaml.cs` L2198, L2219, L2222, L2227, L2229, L2232,
/// L2237). The `msg` parameter is left open even though WPF only
/// uses `WM_KEYDOWN`; future call sites that need `WM_KEYUP` (or
/// `WM_SYSKEYDOWN`) can supply it without touching the wrapper.
///
/// `lParam` is computed by `compute_post_key_lparam` (private) and
/// intentionally diverges from WPF — see that function's docs for the
/// operator-precedence bug being corrected.
///
/// # Errors
///
/// [`ProcessError::PostMessage`] when `PostMessageW` rejects the
/// message (typically the target window destroyed mid-call).
///
/// # Async runtime guidance
///
/// Synchronous Win32 call. Wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [wpf]: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-keydown
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn post_key(handle: WindowHandle, msg: u32, vk: u8) -> Result<(), ProcessError> {
    let lparam = compute_post_key_lparam(vk);
    // Safety: `handle.as_hwnd()` is non-null (newtype invariant);
    // `lparam` is a scalar derived above. `PostMessageW` is
    // asynchronous and propagates any synchronous queueing error.
    unsafe { PostMessageW(handle.as_hwnd(), msg, WPARAM(vk as usize), LPARAM(lparam)) }.map_err(
        |source| ProcessError::PostMessage {
            hwnd: handle.as_raw(),
            source,
        },
    )
}

/// Post an arbitrary `PostMessageW` to `handle` with caller-supplied
/// `wparam` / `lparam`.
///
/// Escape hatch for messages whose payload doesn't fit
/// [`post_string`] (per-character `WM_CHAR`) or [`post_key`]
/// (single-key `WM_KEYDOWN`-shaped lParam). The auto-paste flow uses
/// it for `WM_LBUTTONDOWN` (`MainWindow.xaml.cs` L2214 —
/// `PostMessage(hWnd, WM_LBUTTONDOWN, 1, pos)` where `pos` packs an
/// `(x, y)` point into the lParam).
///
/// # Errors
///
/// [`ProcessError::PostMessage`] on Win32 failure (same surface as
/// [`post_key`] / [`post_string`] / P9.2 `close_play_window`).
///
/// # Async runtime guidance
///
/// Synchronous Win32 call. Wrap in
/// [`tokio::task::spawn_blocking`][sb] from `async fn` callers.
///
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn post_message_raw(
    handle: WindowHandle,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> Result<(), ProcessError> {
    // Safety: `handle.as_hwnd()` is non-null (newtype invariant);
    // `wparam` and `lparam` are scalars interpreted by the message
    // contract. `PostMessageW` is asynchronous and propagates any
    // synchronous queueing error.
    unsafe { PostMessageW(handle.as_hwnd(), msg, WPARAM(wparam), LPARAM(lparam)) }.map_err(
        |source| ProcessError::PostMessage {
            hwnd: handle.as_raw(),
            source,
        },
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_handle_from_raw_null_is_none() {
        let null = HWND(std::ptr::null_mut());
        assert!(WindowHandle::from_raw(null).is_none());
    }

    #[test]
    fn window_handle_from_raw_nonzero_round_trips() {
        let raw = HWND(0x1234_5678 as *mut _);
        let wrapped = WindowHandle::from_raw(raw).expect("non-null HWND wraps");
        assert_eq!(wrapped.as_raw(), 0x1234_5678);
        assert_eq!(wrapped.as_hwnd().0 as usize, 0x1234_5678);
    }

    #[test]
    fn point_serializes_as_object() {
        let json = serde_json::to_string(&Point { x: 100, y: -50 }).unwrap();
        assert_eq!(json, r#"{"x":100,"y":-50}"#);
    }

    #[test]
    fn size_serializes_as_object() {
        let json = serde_json::to_string(&Size {
            width: 1920,
            height: 1080,
        })
        .unwrap();
        assert_eq!(json, r#"{"width":1920,"height":1080}"#);
    }

    #[test]
    fn point_round_trips_through_json() {
        // P10 will move `Point` across the Tauri IPC boundary as JSON;
        // assert deserialize agrees with serialize at the type level
        // (not just the on-the-wire shape, which `point_serializes_as_object`
        // already pins).
        let original = Point { x: -7, y: 42 };
        let wire = serde_json::to_string(&original).unwrap();
        let back: Point = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn size_round_trips_through_json() {
        // Same rationale as `point_round_trips_through_json`.
        let original = Size {
            width: 800,
            height: 600,
        };
        let wire = serde_json::to_string(&original).unwrap();
        let back: Size = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn compute_post_key_lparam_repeat_count_is_one() {
        // Win32 `WM_KEYDOWN` lParam packs the repeat count in bits 0..16.
        // Single-press emission must always advertise a count of 1 — a
        // count of 0 (the WPF bug, see next test) tells the receiver the
        // press is malformed and is the reason Q2 elected to fix the
        // upstream bug.
        let vk_return: u8 = 0x0D;
        let lparam = compute_post_key_lparam(vk_return);
        assert_eq!(lparam & 0xFFFF, 1);
    }

    #[test]
    fn compute_post_key_lparam_diverges_from_wpf_bug() {
        // Encode the WPF C# bug — `MapVirtualKey(vk, 0) << 16 + 1`
        // evaluates as `MapVirtualKey(...) << 17` (operator precedence
        // puts `+` above `<<`). Asserting inequality keeps Q2's
        // intentional divergence from regressing back to the buggy
        // shape under future refactors.
        let vk_return: u8 = 0x0D;
        let scan_code = unsafe { MapVirtualKeyW(vk_return as u32, MAPVK_VK_TO_VSC) } as isize;
        let wpf_buggy_lparam = scan_code << 17;
        assert_ne!(compute_post_key_lparam(vk_return), wpf_buggy_lparam);
    }

    #[test]
    fn process_error_non_ascii_display_includes_offset_and_char() {
        // The Q3 surface promises an actionable error: a P10 caller
        // should be able to surface "non-ASCII character X at byte Y"
        // verbatim to the user. Pin both pieces of evidence on the
        // Display impl so the wording stays self-explanatory.
        let err = ProcessError::NonAscii {
            offset: 3,
            ch: '中',
        };
        let msg = format!("{err}");
        assert!(msg.contains('中'), "expected '中' in {msg:?}");
        assert!(msg.contains('3'), "expected offset '3' in {msg:?}");
    }
}
