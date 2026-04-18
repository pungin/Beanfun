//! Auto-paste orchestration for Beanfun's OTP credential hand-off.
//!
//! Ports `getOtpWorker_RunWorkerCompleted` (`Beanfun/MainWindow.xaml.cs`
//! L2131-2238) — the sequence that types the account name plus
//! freshly-issued OTP into the MapleStory launcher's login dialog
//! right after `services/beanfun` returns the OTP.
//!
//! # WPF parity
//!
//! | WPF step                                | This module                                                     |
//! | --------------------------------------- | --------------------------------------------------------------- |
//! | L2158 `FindWindow(win_class_name, ..)`  | private `find_target_window` (primary + MapleStoryClass fallback)|
//! | L2159-2162 `MapleStoryClassTW` fallback | same — hardcoded here, not caller-supplied                      |
//! | L2164 `hWnd == IntPtr.Zero` short-circuit | **surfaced** as [`ProcessError::WindowNotFound`]              |
//! | L2181 `GetClientAreaSize`               | driver `get_client_area_size`                                   |
//! | L2193-2194 `SetForegroundWindow` + 100ms | driver `set_foreground_window` + private `FOREGROUND_SETTLE`   |
//! | L2195 `"610074".Equals(service_code) && "T9".Equals(service_region)` | `PasteRequest::special_click` (caller decides)    |
//! | L2198-2216 ESC + special click pipeline | private `do_special_click`                                      |
//! | L2219-2223 clear account field (END + 64×BACK) | private `clear_field` with `ACCOUNT_CLEAR_BACKSPACES`    |
//! | L2225 `PostString(hWnd, acc)`           | driver `post_string`                                            |
//! | L2227 VK_TAB to password field          | driver `post_key`                                               |
//! | L2229-2233 clear password (END + 20×BACK)| private `clear_field` with `PASSWORD_CLEAR_BACKSPACES`         |
//! | L2235 `PostString(hWnd, password)`      | driver `post_string`                                            |
//! | L2237 VK_RETURN to submit               | driver `post_key`                                               |
//!
//! # Design decisions (chunk 10.3 D5d pre-flight)
//!
//! - **Q1 — Service- vs command-layer orchestration**: orchestration
//!   lives here. `commands/launcher.rs` stays a thin IPC wrapper
//!   (mirrors the D5a–D5c pattern); the Win32 sequence is framework-
//!   agnostic and testable without Tauri.
//! - **Q2 — Fallback class**: hardcoded (`MAPLESTORY_PRIMARY_CLASS` →
//!   `MAPLESTORY_FALLBACK_CLASS`). WPF hardcodes `"MapleStoryClassTW"`
//!   at L2161; exposing it as a caller parameter would just re-derive
//!   WPF's literal at the command layer with no upside.
//! - **Q3 — `special_click` dispatch**: caller-supplied `bool`, not
//!   `(service_code, service_region)` pair. The command layer (or
//!   eventually the frontend) computes `code == "610074" && region == "T9"`
//!   once and hands the decision down; the service module stays
//!   agnostic about MapleStory SEA / TW business rules.
//! - **Q4 — Sleep mechanism**: [`std::thread::sleep`] inside the
//!   default driver. Every Win32 wrapper we call is already sync and
//!   must run under `spawn_blocking` from the Tokio side; adding an
//!   `await` point here would force a second `spawn_blocking`
//!   boundary per sleep with no benefit — `thread::sleep` blocks the
//!   same OS thread that's already sync-FFI-bound.
//! - **Q5 — Error surface**: [`ProcessError`] only. Window discovery
//!   failure surfaces as [`ProcessError::WindowNotFound`] (P10.3 D5d
//!   new variant); every other wrapped Win32 call reuses the existing
//!   [`ProcessError::PostMessage`] / [`ProcessError::Win32Call`] /
//!   [`ProcessError::NonAscii`] shape. No new variants beyond
//!   `WindowNotFound`.
//! - **Q6 — Dependency injection**: [`PasteDriver`] trait with
//!   [`DefaultPasteDriver`] for production and `tests::RecordingDriver`
//!   for unit tests. Mirrors the `FnMut` DI pattern in
//!   [`super::game::kill_game_processes_with`], scaled up to ten
//!   distinct Win32 call shapes that do not collapse cleanly into a
//!   single closure.
//!
//! # Async runtime guidance
//!
//! [`paste_credentials`] is synchronous end-to-end (Win32 FFI + three
//! [`std::thread::sleep`] calls totalling 400 ms). Callers on a Tokio
//! runtime **must** dispatch via [`tokio::task::spawn_blocking`][sb] —
//! same reason the raw wrappers in [`mod@super::post_string`] require it.
//!
//! [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use std::thread;
use std::time::Duration;

use super::error::ProcessError;
use super::post_string::{
    client_to_screen, find_window, get_client_area_size, get_cursor_pos, post_key,
    post_message_raw, post_string, set_cursor_pos, set_foreground_window, Point, Size,
    WindowHandle,
};

// ---------------------------------------------------------------------------
// Win32 message constants (mirror `MainWindow.xaml.cs` L2186-2192)
// ---------------------------------------------------------------------------

/// `WM_KEYDOWN` message id — every `post_key` call in this flow uses
/// it (WPF L2198, L2219, L2222, L2227, L2229, L2232, L2237).
const WM_KEYDOWN: u32 = 0x0100;

/// `WM_LBUTTONDOWN` message id — synthetic click at the login input
/// (WPF L2214, only on the special-click branch).
const WM_LBUTTONDOWN: u32 = 0x0201;

/// `VK_BACK` (backspace) — used to clear account / password fields
/// (WPF L2222, L2232).
const VK_BACK: u8 = 0x08;

/// `VK_TAB` — moves focus from account to password (WPF L2227).
const VK_TAB: u8 = 0x09;

/// `VK_RETURN` (Enter) — submits the login form (WPF L2237).
const VK_RETURN: u8 = 0x0D;

/// `VK_ESCAPE` — dismisses the MapleStory SEA pre-login prompt
/// (WPF L2198, special-click branch only).
const VK_ESCAPE: u8 = 0x1B;

/// `VK_END` — moves the caret to end-of-field before the backspace
/// spray (WPF L2219, L2229).
const VK_END: u8 = 0x23;

// ---------------------------------------------------------------------------
// Timing + click-layout constants (mirror WPF exactly)
// ---------------------------------------------------------------------------

/// How many backspaces WPF sends to clear the account field
/// (L2220 `for (int i = 0; i < 64; i++)`). The fixed count mirrors
/// the longest plausible account plus margin; the WPF author
/// evidently preferred an over-estimate to a computed length
/// (which would race with the password field's pre-existing content).
const ACCOUNT_CLEAR_BACKSPACES: u32 = 64;

/// How many backspaces WPF sends to clear the password field
/// (L2230 `for (int i = 0; i < 20; i++)`). Same rationale as
/// [`ACCOUNT_CLEAR_BACKSPACES`], smaller because MapleStory
/// passwords are capped at 16 characters by the server.
const PASSWORD_CLEAR_BACKSPACES: u32 = 20;

/// Settling delay after `SetForegroundWindow` before we start
/// posting messages (WPF L2194 `Thread.Sleep(100)`).
const FOREGROUND_SETTLE: Duration = Duration::from_millis(100);

/// Settling delay after ESC on the special-click branch (WPF L2199
/// `Thread.Sleep(100)`) — lets the SEA pre-login prompt fully dismiss
/// before we move the cursor.
const ESCAPE_SETTLE: Duration = Duration::from_millis(100);

/// Settling delay after the synthetic click (WPF L2215
/// `Thread.Sleep(200)`) — lets the MapleStory client process the
/// click before we restore the cursor and start typing.
const CLICK_SETTLE: Duration = Duration::from_millis(200);

/// Horizontal fraction of the client area to click at on the special-
/// click branch (WPF L2206 `wndSize.Width * 0.5`).
const CLICK_X_RATIO: f64 = 0.5;

/// Vertical fraction of the client area to click at on the special-
/// click branch (WPF L2207 `wndSize.Height * 0.4`).
const CLICK_Y_RATIO: f64 = 0.4;

/// Primary MapleStory launcher window class (WPF L76 / L2158).
pub const MAPLESTORY_PRIMARY_CLASS: &str = "MapleStoryClass";

/// Fallback class name WPF tries when the primary query fails on
/// the TW region (WPF L2161).
pub const MAPLESTORY_FALLBACK_CLASS: &str = "MapleStoryClassTW";

// ---------------------------------------------------------------------------
// PasteRequest
// ---------------------------------------------------------------------------

/// Service-layer orchestration input.
///
/// Every field mirrors a WPF input at the `getOtpWorker_RunWorkerCompleted`
/// call site — see the WPF-parity table in the module docs for the
/// per-field mapping. Fields are `&str` (not owned `String`) because
/// the command layer already owns the data; copying once at the IPC
/// boundary and borrowing end-to-end here avoids an unnecessary
/// allocation per paste.
#[derive(Debug, Clone, Copy)]
pub struct PasteRequest<'a> {
    /// Top-level window class to find; the fallback class
    /// ([`MAPLESTORY_FALLBACK_CLASS`]) is applied automatically when
    /// `class_name == MAPLESTORY_PRIMARY_CLASS`.
    pub class_name: &'a str,

    /// Account name to type into the login dialog after clearing.
    /// Must be ASCII (WPF's constraint, preserved in
    /// [`super::post_string::post_string`] via
    /// [`ProcessError::NonAscii`]).
    pub account: &'a str,

    /// Password (or OTP) to type into the password field. Same ASCII
    /// constraint as [`Self::account`].
    pub password: &'a str,

    /// When `true`, execute the MapleStory-SEA pre-click sequence
    /// (ESC dismiss + synthetic click at ~(50%, 40%) of the client
    /// area). WPF gates this on `service_code == "610074" &&
    /// service_region == "T9"` (L2195); the decision is computed by
    /// the caller (command layer or frontend) and handed down as a
    /// `bool` to keep this module free of business rules.
    pub special_click: bool,
}

// ---------------------------------------------------------------------------
// PasteDriver trait — DI seam for all Win32 touch points
// ---------------------------------------------------------------------------

/// Behavioural abstraction over every Win32 / timing call the
/// auto-paste sequence makes.
///
/// Exists so [`paste_credentials_with`] is fully unit-testable without
/// a live MapleStory window — tests implement this trait with an
/// in-memory recorder that captures every call in order, and asserts
/// the sequence matches the WPF-parity table. The production
/// implementation [`DefaultPasteDriver`] delegates each method to its
/// [`mod@super::post_string`] / [`std::thread::sleep`] counterpart.
///
/// Method signatures take `&mut self` so non-trivial mock drivers can
/// record state (or simulate transient failures) without interior
/// mutability. Production [`DefaultPasteDriver`] is stateless and
/// simply ignores the `&mut`.
pub trait PasteDriver {
    /// Locate a top-level window by class name. Returns `None` if no
    /// such window exists (same semantics as
    /// [`super::post_string::find_window`] with `title = None`).
    fn find_window(&mut self, class: &str) -> Option<WindowHandle>;

    /// Bring `handle` to the foreground. Returns `false` when Windows
    /// refuses (routine, not an error); see
    /// [`super::post_string::set_foreground_window`].
    fn set_foreground_window(&mut self, handle: WindowHandle) -> bool;

    /// Width × height of `handle`'s client area.
    fn get_client_area_size(&mut self, handle: WindowHandle) -> Result<Size, ProcessError>;

    /// Current cursor position (best-effort; returns `None` when
    /// Win32 reports failure — typically a locked desktop).
    fn get_cursor_pos(&mut self) -> Option<Point>;

    /// Convert `point` from `handle`'s client area to screen
    /// coordinates.
    fn client_to_screen(
        &mut self,
        handle: WindowHandle,
        point: Point,
    ) -> Result<Point, ProcessError>;

    /// Move the cursor to `point` (screen coordinates); returns
    /// `false` on failure (best-effort, mirrors
    /// [`super::post_string::set_cursor_pos`]).
    fn set_cursor_pos(&mut self, point: Point) -> bool;

    /// Post a single `WM_KEYDOWN` / `WM_KEYUP` for `vk`.
    fn post_key(&mut self, handle: WindowHandle, msg: u32, vk: u8) -> Result<(), ProcessError>;

    /// Post `s` as a sequence of `WM_CHAR` messages; fails fast on
    /// non-ASCII input ([`ProcessError::NonAscii`]).
    fn post_string(&mut self, handle: WindowHandle, s: &str) -> Result<(), ProcessError>;

    /// Post an arbitrary `PostMessageW` — used only for
    /// `WM_LBUTTONDOWN` on the special-click branch.
    fn post_message_raw(
        &mut self,
        handle: WindowHandle,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Result<(), ProcessError>;

    /// Sleep for `duration`. Production implementation calls
    /// [`std::thread::sleep`]; tests typically record the duration
    /// and return immediately.
    fn sleep(&mut self, duration: Duration);
}

/// Production driver — delegates every call to [`mod@super::post_string`]
/// and [`std::thread::sleep`]. Stateless; constructable from any
/// context because all underlying functions are free functions.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPasteDriver;

impl PasteDriver for DefaultPasteDriver {
    fn find_window(&mut self, class: &str) -> Option<WindowHandle> {
        find_window(Some(class), None)
    }

    fn set_foreground_window(&mut self, handle: WindowHandle) -> bool {
        set_foreground_window(handle)
    }

    fn get_client_area_size(&mut self, handle: WindowHandle) -> Result<Size, ProcessError> {
        get_client_area_size(handle)
    }

    fn get_cursor_pos(&mut self) -> Option<Point> {
        get_cursor_pos()
    }

    fn client_to_screen(
        &mut self,
        handle: WindowHandle,
        point: Point,
    ) -> Result<Point, ProcessError> {
        client_to_screen(handle, point)
    }

    fn set_cursor_pos(&mut self, point: Point) -> bool {
        set_cursor_pos(point)
    }

    fn post_key(&mut self, handle: WindowHandle, msg: u32, vk: u8) -> Result<(), ProcessError> {
        post_key(handle, msg, vk)
    }

    fn post_string(&mut self, handle: WindowHandle, s: &str) -> Result<(), ProcessError> {
        post_string(handle, s)
    }

    fn post_message_raw(
        &mut self,
        handle: WindowHandle,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Result<(), ProcessError> {
        post_message_raw(handle, msg, wparam, lparam)
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Run the auto-paste sequence against a live MapleStory launcher.
///
/// Convenience wrapper that uses [`DefaultPasteDriver`]. See
/// [`paste_credentials_with`] for the DI-friendly variant and the
/// WPF-parity breakdown in the module docs.
///
/// # Errors
///
/// Returns any [`ProcessError`] surfaced by the orchestration — see
/// [`paste_credentials_with`] for the full list.
///
/// # Async runtime guidance
///
/// Synchronous end-to-end (Win32 FFI + 400 ms of
/// [`std::thread::sleep`]). Tokio callers must dispatch via
/// [`tokio::task::spawn_blocking`][sb].
///
/// [sb]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub fn paste_credentials(request: PasteRequest<'_>) -> Result<(), ProcessError> {
    paste_credentials_with(request, &mut DefaultPasteDriver)
}

/// DI-friendly variant of [`paste_credentials`].
///
/// The production driver is [`DefaultPasteDriver`]; tests substitute
/// a `RecordingDriver` to assert the call sequence without touching
/// real Win32 state.
///
/// # Errors
///
/// - [`ProcessError::WindowNotFound`] when no matching top-level
///   window exists (both primary and fallback classes fail).
/// - [`ProcessError::Win32Call`] from `GetClientRect` /
///   `ClientToScreen` when the window is destroyed mid-sequence.
/// - [`ProcessError::PostMessage`] from any synthetic-input call
///   (`WM_KEYDOWN`, `WM_CHAR`, `WM_LBUTTONDOWN`) when the message
///   queue rejects the post.
/// - [`ProcessError::NonAscii`] if `request.account` or
///   `request.password` contains a non-ASCII codepoint (preserved
///   from [`super::post_string::post_string`]).
pub fn paste_credentials_with<D: PasteDriver>(
    request: PasteRequest<'_>,
    driver: &mut D,
) -> Result<(), ProcessError> {
    let handle = find_target_window(driver, request.class_name)?;
    let size = driver.get_client_area_size(handle)?;

    driver.set_foreground_window(handle);
    driver.sleep(FOREGROUND_SETTLE);

    if request.special_click {
        do_special_click(driver, handle, size)?;
    }

    clear_field(driver, handle, ACCOUNT_CLEAR_BACKSPACES)?;
    driver.post_string(handle, request.account)?;
    driver.post_key(handle, WM_KEYDOWN, VK_TAB)?;
    clear_field(driver, handle, PASSWORD_CLEAR_BACKSPACES)?;
    driver.post_string(handle, request.password)?;
    driver.post_key(handle, WM_KEYDOWN, VK_RETURN)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Locate the target window with primary-then-fallback semantics.
///
/// WPF L2158-2162: tries `class_name`; if the name is exactly
/// [`MAPLESTORY_PRIMARY_CLASS`] and the first call returned no
/// window, retries with [`MAPLESTORY_FALLBACK_CLASS`]. Surfaces
/// [`ProcessError::WindowNotFound`] when both probes fail (WPF
/// silently copied the OTP to clipboard instead — our command
/// layer handles that fallback, see `commands/launcher.rs` D5d).
fn find_target_window<D: PasteDriver>(
    driver: &mut D,
    class_name: &str,
) -> Result<WindowHandle, ProcessError> {
    if let Some(handle) = driver.find_window(class_name) {
        return Ok(handle);
    }

    let fallback = if class_name == MAPLESTORY_PRIMARY_CLASS {
        Some(MAPLESTORY_FALLBACK_CLASS)
    } else {
        None
    };

    if let Some(fb_class) = fallback {
        if let Some(handle) = driver.find_window(fb_class) {
            return Ok(handle);
        }
    }

    Err(ProcessError::WindowNotFound {
        primary_class: class_name.to_owned(),
        fallback_class: fallback.map(str::to_owned),
    })
}

/// Compute the `(x, y)` pixel offset inside the client area where the
/// special-click branch synthesises a click.
///
/// WPF L2205-2208:
/// ```csharp
/// new System.Drawing.Point(
///     (int)(wndSize.Width * 0.5),
///     (int)(wndSize.Height * 0.4)
/// )
/// ```
///
/// Extracted so unit tests can pin the ratio contract (`0.5`, `0.4`)
/// without standing up a full driver.
fn compute_click_point(size: Size) -> Point {
    Point {
        x: (size.width as f64 * CLICK_X_RATIO) as i32,
        y: (size.height as f64 * CLICK_Y_RATIO) as i32,
    }
}

/// Pack a client-area [`Point`] into the `lParam` shape
/// `WM_LBUTTONDOWN` expects: low 16 bits = `x`, high 16 bits = `y`.
///
/// WPF L2213: `(textBoxPoint.X & 0xFFFF) | (textBoxPoint.Y << 16)`.
///
/// Extracted so unit tests can pin the bit layout without needing a
/// live message pump.
fn pack_lbutton_pos(point: Point) -> isize {
    ((point.x & 0xFFFF) | (point.y << 16)) as isize
}

/// Execute the MapleStory-SEA pre-login click sequence (WPF
/// L2198-2216).
///
/// Errors short-circuit the paste — if the `WM_LBUTTONDOWN` fails
/// (target destroyed) or `ClientToScreen` fails mid-way, we refuse
/// to type credentials into an uncertain-focus window.
/// `get_cursor_pos` / `set_cursor_pos` / `set_foreground_window`
/// stay best-effort (their failures are cosmetic).
fn do_special_click<D: PasteDriver>(
    driver: &mut D,
    handle: WindowHandle,
    size: Size,
) -> Result<(), ProcessError> {
    driver.post_key(handle, WM_KEYDOWN, VK_ESCAPE)?;
    driver.sleep(ESCAPE_SETTLE);

    let saved_cursor = driver.get_cursor_pos();
    let screen_origin = driver.client_to_screen(handle, Point { x: 0, y: 0 })?;
    let click_point = compute_click_point(size);

    driver.set_cursor_pos(Point {
        x: screen_origin.x + click_point.x,
        y: screen_origin.y + click_point.y,
    });

    driver.post_message_raw(handle, WM_LBUTTONDOWN, 1, pack_lbutton_pos(click_point))?;
    driver.sleep(CLICK_SETTLE);

    if let Some(old) = saved_cursor {
        driver.set_cursor_pos(old);
    }

    Ok(())
}

/// Clear the currently-focused text field by moving the caret to the
/// end (`VK_END`) and spraying `backspaces` × `VK_BACK`.
///
/// Mirrors WPF L2219-2223 (account clear) and L2229-2233 (password
/// clear). Extracted so the two call sites share one implementation —
/// the only variable is the backspace count.
fn clear_field<D: PasteDriver>(
    driver: &mut D,
    handle: WindowHandle,
    backspaces: u32,
) -> Result<(), ProcessError> {
    driver.post_key(handle, WM_KEYDOWN, VK_END)?;
    for _ in 0..backspaces {
        driver.post_key(handle, WM_KEYDOWN, VK_BACK)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::HWND;

    // Stable non-null HWND for every test that needs one.
    fn test_handle() -> WindowHandle {
        let raw = HWND(0x1000 as *mut _);
        WindowHandle::from_raw(raw).expect("non-null HWND wraps")
    }

    /// Every call [`paste_credentials_with`] can make against the
    /// driver, in the order it was issued. Comparing a `Vec<Call>`
    /// against a literal expected sequence is the most direct way
    /// to assert WPF parity.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        FindWindow(String),
        SetForegroundWindow,
        GetClientAreaSize,
        GetCursorPos,
        ClientToScreen(Point),
        SetCursorPos(Point),
        PostKey(u32, u8),
        PostString(String),
        PostMessageRaw(u32, usize, isize),
        Sleep(Duration),
    }

    /// Driver that records every call + lets tests plant canned
    /// responses for the methods that return values.
    struct RecordingDriver {
        calls: Vec<Call>,
        find_window_responses: Vec<Option<WindowHandle>>,
        client_area_size: Size,
        cursor_pos: Option<Point>,
        client_to_screen_result: Point,
    }

    impl RecordingDriver {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                find_window_responses: vec![Some(test_handle())],
                client_area_size: Size {
                    width: 800,
                    height: 600,
                },
                cursor_pos: Some(Point { x: 42, y: 84 }),
                client_to_screen_result: Point { x: 100, y: 200 },
            }
        }
    }

    impl PasteDriver for RecordingDriver {
        fn find_window(&mut self, class: &str) -> Option<WindowHandle> {
            self.calls.push(Call::FindWindow(class.to_owned()));
            if self.find_window_responses.is_empty() {
                None
            } else {
                self.find_window_responses.remove(0)
            }
        }

        fn set_foreground_window(&mut self, _handle: WindowHandle) -> bool {
            self.calls.push(Call::SetForegroundWindow);
            true
        }

        fn get_client_area_size(&mut self, _handle: WindowHandle) -> Result<Size, ProcessError> {
            self.calls.push(Call::GetClientAreaSize);
            Ok(self.client_area_size)
        }

        fn get_cursor_pos(&mut self) -> Option<Point> {
            self.calls.push(Call::GetCursorPos);
            self.cursor_pos
        }

        fn client_to_screen(
            &mut self,
            _handle: WindowHandle,
            point: Point,
        ) -> Result<Point, ProcessError> {
            self.calls.push(Call::ClientToScreen(point));
            Ok(self.client_to_screen_result)
        }

        fn set_cursor_pos(&mut self, point: Point) -> bool {
            self.calls.push(Call::SetCursorPos(point));
            true
        }

        fn post_key(
            &mut self,
            _handle: WindowHandle,
            msg: u32,
            vk: u8,
        ) -> Result<(), ProcessError> {
            self.calls.push(Call::PostKey(msg, vk));
            Ok(())
        }

        fn post_string(&mut self, _handle: WindowHandle, s: &str) -> Result<(), ProcessError> {
            self.calls.push(Call::PostString(s.to_owned()));
            Ok(())
        }

        fn post_message_raw(
            &mut self,
            _handle: WindowHandle,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> Result<(), ProcessError> {
            self.calls.push(Call::PostMessageRaw(msg, wparam, lparam));
            Ok(())
        }

        fn sleep(&mut self, duration: Duration) {
            self.calls.push(Call::Sleep(duration));
        }
    }

    // ----- pure helpers ---------------------------------------------

    #[test]
    fn pack_lbutton_pos_matches_wpf_bit_layout() {
        // Matches WPF L2213 verbatim: `(x & 0xFFFF) | (y << 16)` for
        // positive 16-bit-wide coords. Using 400 × 240 makes the bit
        // pattern human-readable (0x00F0_0190).
        let packed = pack_lbutton_pos(Point { x: 400, y: 240 });
        assert_eq!(packed, (400 & 0xFFFF) | (240 << 16));
        assert_eq!(packed, 0x00F0_0190);
    }

    #[test]
    fn pack_lbutton_pos_keeps_x_in_low_word_under_overflow() {
        // WPF masks `x & 0xFFFF` before the OR — we preserve that so
        // client-area widths that exceed 65535 (pathological, but
        // possible on 4K+ multi-monitor setups) still produce a
        // valid lParam.
        let packed = pack_lbutton_pos(Point { x: 0x1_ABCD, y: 5 });
        assert_eq!(packed & 0xFFFF, 0xABCD);
    }

    #[test]
    fn compute_click_point_applies_wpf_ratios() {
        // Pins the 0.5 / 0.4 ratios so any future refactor that
        // "rationalises" them (e.g. 0.5 / 0.5) has to explain why
        // WPF's L2206-2207 was wrong.
        let p = compute_click_point(Size {
            width: 1000,
            height: 500,
        });
        assert_eq!(p, Point { x: 500, y: 200 });
    }

    #[test]
    fn compute_click_point_truncates_toward_zero_like_csharp_int_cast() {
        // C# `(int)(wndSize.Width * 0.5)` truncates. `Size {33,33}`
        // yields 16.5 / 13.2, both of which should truncate to 16 / 13.
        let p = compute_click_point(Size {
            width: 33,
            height: 33,
        });
        assert_eq!(p, Point { x: 16, y: 13 });
    }

    // ----- find_target_window ---------------------------------------

    #[test]
    fn find_target_window_returns_primary_when_match() {
        let mut driver = RecordingDriver::new();
        let handle = find_target_window(&mut driver, MAPLESTORY_PRIMARY_CLASS)
            .expect("primary window found");
        assert_eq!(handle, test_handle());
        // Exactly one find_window call, for the primary class.
        let find_calls: Vec<_> = driver
            .calls
            .iter()
            .filter_map(|c| match c {
                Call::FindWindow(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(find_calls, vec![MAPLESTORY_PRIMARY_CLASS.to_owned()]);
    }

    #[test]
    fn find_target_window_falls_back_when_primary_is_maplestory() {
        let mut driver = RecordingDriver::new();
        driver.find_window_responses = vec![None, Some(test_handle())];
        let handle = find_target_window(&mut driver, MAPLESTORY_PRIMARY_CLASS)
            .expect("fallback window found");
        assert_eq!(handle, test_handle());
        // Two find_window calls: primary then fallback, in order.
        let find_calls: Vec<_> = driver
            .calls
            .iter()
            .filter_map(|c| match c {
                Call::FindWindow(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            find_calls,
            vec![
                MAPLESTORY_PRIMARY_CLASS.to_owned(),
                MAPLESTORY_FALLBACK_CLASS.to_owned(),
            ]
        );
    }

    #[test]
    fn find_target_window_does_not_fall_back_for_non_maplestory_class() {
        let mut driver = RecordingDriver::new();
        driver.find_window_responses = vec![None];
        let err = find_target_window(&mut driver, "NexonGameClass")
            .expect_err("no window should surface WindowNotFound");
        match err {
            ProcessError::WindowNotFound {
                primary_class,
                fallback_class,
            } => {
                assert_eq!(primary_class, "NexonGameClass");
                assert!(fallback_class.is_none());
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
        // Only one find_window call (no fallback attempted).
        let find_calls = driver
            .calls
            .iter()
            .filter(|c| matches!(c, Call::FindWindow(_)))
            .count();
        assert_eq!(find_calls, 1);
    }

    #[test]
    fn find_target_window_surfaces_window_not_found_when_both_classes_miss() {
        let mut driver = RecordingDriver::new();
        driver.find_window_responses = vec![None, None];
        let err = find_target_window(&mut driver, MAPLESTORY_PRIMARY_CLASS)
            .expect_err("both misses should surface WindowNotFound");
        match err {
            ProcessError::WindowNotFound {
                primary_class,
                fallback_class,
            } => {
                assert_eq!(primary_class, MAPLESTORY_PRIMARY_CLASS);
                assert_eq!(fallback_class.as_deref(), Some(MAPLESTORY_FALLBACK_CLASS));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    // ----- paste_credentials_with: non-special-click path ------------

    #[test]
    fn paste_credentials_without_special_click_matches_wpf_sequence() {
        let mut driver = RecordingDriver::new();
        let request = PasteRequest {
            class_name: "NexonGameClass",
            account: "user1",
            password: "pw42",
            special_click: false,
        };
        paste_credentials_with(request, &mut driver).expect("paste succeeds");

        let mut expected = vec![
            Call::FindWindow("NexonGameClass".into()),
            Call::GetClientAreaSize,
            Call::SetForegroundWindow,
            Call::Sleep(FOREGROUND_SETTLE),
            Call::PostKey(WM_KEYDOWN, VK_END),
        ];
        expected.extend(
            std::iter::repeat(Call::PostKey(WM_KEYDOWN, VK_BACK))
                .take(ACCOUNT_CLEAR_BACKSPACES as usize),
        );
        expected.push(Call::PostString("user1".into()));
        expected.push(Call::PostKey(WM_KEYDOWN, VK_TAB));
        expected.push(Call::PostKey(WM_KEYDOWN, VK_END));
        expected.extend(
            std::iter::repeat(Call::PostKey(WM_KEYDOWN, VK_BACK))
                .take(PASSWORD_CLEAR_BACKSPACES as usize),
        );
        expected.push(Call::PostString("pw42".into()));
        expected.push(Call::PostKey(WM_KEYDOWN, VK_RETURN));

        assert_eq!(driver.calls, expected);
    }

    // ----- paste_credentials_with: special-click path ----------------

    #[test]
    fn paste_credentials_with_special_click_injects_esc_and_click() {
        let mut driver = RecordingDriver::new();
        let request = PasteRequest {
            class_name: MAPLESTORY_PRIMARY_CLASS,
            account: "acc",
            password: "otp",
            special_click: true,
        };
        paste_credentials_with(request, &mut driver).expect("paste succeeds");

        // Pin only the prefix of the sequence — the body
        // (clear + type + submit) is covered by the non-special
        // test; here we verify the ESC + click detour.
        let prefix_len = 10;
        let click_point = compute_click_point(Size {
            width: 800,
            height: 600,
        });
        let screen_origin = Point { x: 100, y: 200 };

        let expected_prefix = vec![
            Call::FindWindow(MAPLESTORY_PRIMARY_CLASS.into()),
            Call::GetClientAreaSize,
            Call::SetForegroundWindow,
            Call::Sleep(FOREGROUND_SETTLE),
            Call::PostKey(WM_KEYDOWN, VK_ESCAPE),
            Call::Sleep(ESCAPE_SETTLE),
            Call::GetCursorPos,
            Call::ClientToScreen(Point { x: 0, y: 0 }),
            Call::SetCursorPos(Point {
                x: screen_origin.x + click_point.x,
                y: screen_origin.y + click_point.y,
            }),
            Call::PostMessageRaw(WM_LBUTTONDOWN, 1, pack_lbutton_pos(click_point)),
        ];

        assert_eq!(&driver.calls[..prefix_len], expected_prefix.as_slice());

        // Click-settle sleep + cursor restore happen directly after.
        assert_eq!(driver.calls[prefix_len], Call::Sleep(CLICK_SETTLE));
        assert_eq!(
            driver.calls[prefix_len + 1],
            Call::SetCursorPos(Point { x: 42, y: 84 })
        );
    }

    #[test]
    fn paste_credentials_with_special_click_skips_cursor_restore_when_save_failed() {
        // If `GetCursorPos` returns `None` (locked desktop, RDP
        // quirk, …), WPF L2202-2216 leaves the cursor wherever our
        // synthetic click last placed it — no restore call. The Rust
        // port preserves that by only restoring `if let Some(old)`.
        let mut driver = RecordingDriver::new();
        driver.cursor_pos = None;

        let request = PasteRequest {
            class_name: MAPLESTORY_PRIMARY_CLASS,
            account: "a",
            password: "b",
            special_click: true,
        };
        paste_credentials_with(request, &mut driver).expect("paste succeeds");

        // Exactly one SetCursorPos call (the click target), not two.
        let set_cursor_calls = driver
            .calls
            .iter()
            .filter(|c| matches!(c, Call::SetCursorPos(_)))
            .count();
        assert_eq!(set_cursor_calls, 1);
    }

    // ----- error propagation ----------------------------------------

    #[test]
    fn paste_credentials_surfaces_window_not_found() {
        let mut driver = RecordingDriver::new();
        driver.find_window_responses = vec![None, None];
        let request = PasteRequest {
            class_name: MAPLESTORY_PRIMARY_CLASS,
            account: "a",
            password: "b",
            special_click: false,
        };
        let err = paste_credentials_with(request, &mut driver)
            .expect_err("no window should short-circuit");
        assert!(matches!(err, ProcessError::WindowNotFound { .. }));
    }

    #[test]
    fn paste_credentials_short_circuits_on_client_area_size_failure() {
        // A GetClientRect failure mid-sequence means the handle is
        // gone; WPF L2184 falls back to clipboard-copy because
        // `wndSize == Size.Empty`. Our Rust port surfaces the error
        // so the command layer can do the same fallback higher up.
        struct FailingSizeDriver {
            inner: RecordingDriver,
        }

        impl PasteDriver for FailingSizeDriver {
            fn find_window(&mut self, class: &str) -> Option<WindowHandle> {
                self.inner.find_window(class)
            }
            fn set_foreground_window(&mut self, handle: WindowHandle) -> bool {
                self.inner.set_foreground_window(handle)
            }
            fn get_client_area_size(
                &mut self,
                _handle: WindowHandle,
            ) -> Result<Size, ProcessError> {
                Err(ProcessError::Win32Call {
                    name: "GetClientRect",
                    source: windows::core::Error::from_win32(),
                })
            }
            fn get_cursor_pos(&mut self) -> Option<Point> {
                self.inner.get_cursor_pos()
            }
            fn client_to_screen(
                &mut self,
                handle: WindowHandle,
                point: Point,
            ) -> Result<Point, ProcessError> {
                self.inner.client_to_screen(handle, point)
            }
            fn set_cursor_pos(&mut self, point: Point) -> bool {
                self.inner.set_cursor_pos(point)
            }
            fn post_key(
                &mut self,
                handle: WindowHandle,
                msg: u32,
                vk: u8,
            ) -> Result<(), ProcessError> {
                self.inner.post_key(handle, msg, vk)
            }
            fn post_string(&mut self, handle: WindowHandle, s: &str) -> Result<(), ProcessError> {
                self.inner.post_string(handle, s)
            }
            fn post_message_raw(
                &mut self,
                handle: WindowHandle,
                msg: u32,
                wparam: usize,
                lparam: isize,
            ) -> Result<(), ProcessError> {
                self.inner.post_message_raw(handle, msg, wparam, lparam)
            }
            fn sleep(&mut self, duration: Duration) {
                self.inner.sleep(duration);
            }
        }

        let mut driver = FailingSizeDriver {
            inner: RecordingDriver::new(),
        };
        let request = PasteRequest {
            class_name: MAPLESTORY_PRIMARY_CLASS,
            account: "a",
            password: "b",
            special_click: false,
        };
        let err = paste_credentials_with(request, &mut driver)
            .expect_err("GetClientRect failure should short-circuit");
        assert!(matches!(
            err,
            ProcessError::Win32Call {
                name: "GetClientRect",
                ..
            }
        ));
        // Critical property: no synthetic input reaches the window
        // after a size-query failure — the short-circuit is what
        // prevents typing into a defocused / destroyed window.
        assert!(!driver
            .inner
            .calls
            .iter()
            .any(|c| matches!(c, Call::PostString(_) | Call::PostKey(..))));
    }

    #[test]
    fn paste_credentials_propagates_non_ascii_account() {
        // Planting a non-ASCII account surfaces through the
        // `post_string` hop and must short-circuit before the password
        // is typed. Uses a custom driver because the default recorder
        // always returns Ok from `post_string`.
        struct NonAsciiAccountDriver {
            inner: RecordingDriver,
        }

        impl PasteDriver for NonAsciiAccountDriver {
            fn find_window(&mut self, class: &str) -> Option<WindowHandle> {
                self.inner.find_window(class)
            }
            fn set_foreground_window(&mut self, handle: WindowHandle) -> bool {
                self.inner.set_foreground_window(handle)
            }
            fn get_client_area_size(&mut self, handle: WindowHandle) -> Result<Size, ProcessError> {
                self.inner.get_client_area_size(handle)
            }
            fn get_cursor_pos(&mut self) -> Option<Point> {
                self.inner.get_cursor_pos()
            }
            fn client_to_screen(
                &mut self,
                handle: WindowHandle,
                point: Point,
            ) -> Result<Point, ProcessError> {
                self.inner.client_to_screen(handle, point)
            }
            fn set_cursor_pos(&mut self, point: Point) -> bool {
                self.inner.set_cursor_pos(point)
            }
            fn post_key(
                &mut self,
                handle: WindowHandle,
                msg: u32,
                vk: u8,
            ) -> Result<(), ProcessError> {
                self.inner.post_key(handle, msg, vk)
            }
            fn post_string(&mut self, _handle: WindowHandle, s: &str) -> Result<(), ProcessError> {
                if let Some((offset, ch)) = s.char_indices().find(|(_, c)| !c.is_ascii()) {
                    return Err(ProcessError::NonAscii { offset, ch });
                }
                self.inner.calls.push(Call::PostString(s.to_owned()));
                Ok(())
            }
            fn post_message_raw(
                &mut self,
                handle: WindowHandle,
                msg: u32,
                wparam: usize,
                lparam: isize,
            ) -> Result<(), ProcessError> {
                self.inner.post_message_raw(handle, msg, wparam, lparam)
            }
            fn sleep(&mut self, duration: Duration) {
                self.inner.sleep(duration);
            }
        }

        let mut driver = NonAsciiAccountDriver {
            inner: RecordingDriver::new(),
        };
        let request = PasteRequest {
            class_name: MAPLESTORY_PRIMARY_CLASS,
            account: "中文",
            password: "ascii-pw",
            special_click: false,
        };
        let err = paste_credentials_with(request, &mut driver)
            .expect_err("non-ASCII account short-circuits");
        assert!(matches!(err, ProcessError::NonAscii { .. }));
        // Password PostString must not have fired.
        assert!(!driver
            .inner
            .calls
            .iter()
            .any(|c| matches!(c, Call::PostString(s) if s == "ascii-pw")));
    }
}
