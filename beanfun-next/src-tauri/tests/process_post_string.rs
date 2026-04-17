//! Integration tests for the chunk 9.3 auto-paste primitives in
//! [`services::process::post_string`][sps].
//!
//! Three baseline tests (no `#[ignore]`) validate the cheap-and-stable
//! primitives against ambient OS state — `Shell_TrayWnd` is present on
//! every interactive desktop session, the cursor is movable on every
//! interactive workstation. These run on every `cargo test` and catch
//! wiring regressions (PCWSTR encoding, `BOOL` interpretation, newtype
//! roundtrips, `Win32Call` source synthesis).
//!
//! One `#[ignore]`-d full smoke (`spawn_notepad_full_paste_smoke`)
//! exercises the entire post sequence — `find_window` →
//! `set_foreground_window` → `post_string` → `post_key(VK_END)` —
//! against a freshly spawned `notepad.exe`. `VK_END` mirrors the WPF
//! auto-paste call site (`MainWindow.xaml.cs` L2222) which uses it to
//! jump the caret to the end of any pre-existing field content before
//! typing. It is deliberately not
//! run by default because (a) it spawns and tears down a UI window,
//! which CI may resent, and (b) Win11 Notepad is a UWP-shelled app
//! whose window class / title shape changes between feature updates;
//! Q7 of the P9.3 pre-flight elected to omit content read-back, so
//! the smoke only verifies that each `Result` returns `Ok`. Run
//! explicitly with `cargo test -- --ignored` on a developer
//! workstation.
//!
//! [sps]: beanfun_next_lib::services::process::post_string
//!
//! # Harness hygiene
//!
//! The notepad smoke owns its spawned child via [`ChildGuard`] (same
//! shape as `tests/process_find_kill.rs`); even on panic mid-assert,
//! `Drop` reaps the child so subsequent runs don't pile up orphan
//! Notepad windows.

#![cfg(target_os = "windows")]

use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use beanfun_next_lib::services::process::{
    client_to_screen, find_window, get_client_area_size, get_cursor_pos, post_key, post_string,
    set_cursor_pos, set_foreground_window, Point, WindowHandle,
};

/// RAII wrapper that guarantees a spawned child is killed on drop.
/// Mirrors `tests/process_find_kill.rs::ChildGuard` for harness
/// symmetry across the two integration files.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Locate the system tray window (`Shell_TrayWnd` — owned by
/// `explorer.exe`). Always present on a logged-in interactive Windows
/// session; missing only on Server Core or session-0 contexts where
/// these tests aren't expected to run anyway. Centralising the lookup
/// keeps the three baseline tests honest about their shared
/// precondition.
fn shell_tray_handle() -> WindowHandle {
    find_window(Some("Shell_TrayWnd"), None)
        .expect("Shell_TrayWnd not found — explorer.exe must be running for these baseline tests")
}

#[test]
fn find_window_locates_shell_tray() {
    let handle = shell_tray_handle();
    // `WindowHandle` is `NonZeroUsize` by construction so this is
    // structurally guaranteed; the assertion makes the invariant
    // visible at the test boundary in case the newtype is ever
    // weakened to a plain `usize` in a future refactor.
    assert!(handle.as_raw() > 0);
}

#[test]
fn get_client_area_size_returns_positive_dimensions_for_shell_tray() {
    let handle = shell_tray_handle();

    let size = get_client_area_size(handle).expect("GetClientRect on Shell_TrayWnd");
    assert!(
        size.width > 0,
        "expected positive width, got {}",
        size.width
    );
    assert!(
        size.height > 0,
        "expected positive height, got {}",
        size.height
    );

    // `client_to_screen((0,0))` on the same handle exercises the
    // BOOL→Result adapter path (Q5/D5) — failure here means the
    // `Win32Call` synthesis from `GetLastError` is mis-wired.
    let _ =
        client_to_screen(handle, Point { x: 0, y: 0 }).expect("ClientToScreen on Shell_TrayWnd");
}

#[test]
fn cursor_round_trips_within_a_pixel() {
    // Restore the original position BEFORE asserting so a panicking
    // test still leaves the cursor where the user left it.
    let original = get_cursor_pos().expect("GetCursorPos must succeed on interactive desktop");
    let target = Point {
        x: original.x + 1,
        y: original.y + 1,
    };

    assert!(set_cursor_pos(target), "SetCursorPos returned false");

    // Brief settle: SetCursorPos is synchronous from our perspective
    // but downstream input handlers may need a tick to update the
    // cached cursor position GetCursorPos reads back.
    thread::sleep(Duration::from_millis(10));

    let observed = get_cursor_pos().expect("GetCursorPos after set");

    let _ = set_cursor_pos(original);

    // ±2 pixel tolerance: high-DPI displays and Windows cursor-snap
    // accessibility settings can legitimately quantise SetCursorPos
    // to even coordinates.
    let dx = (observed.x - target.x).abs();
    let dy = (observed.y - target.y).abs();
    assert!(
        dx <= 2 && dy <= 2,
        "cursor at {observed:?}, expected near {target:?} (dx={dx}, dy={dy})"
    );
}

// ---------------------------------------------------------------------------
// Full smoke — spawn notepad, full auto-paste sequence. `#[ignore]`-d
// because of UI side effects and Win11 Notepad UWP unreliability;
// Q7 pre-flight: presence of `Ok` returns is the contract, content
// read-back is intentionally out of scope.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "spawns notepad.exe; run explicitly with `cargo test -- --ignored`"]
fn spawn_notepad_full_paste_smoke() {
    // `WM_KEYDOWN = 0x0100`, `VK_END = 0x23` per the Win32 spec.
    // Hard-coded rather than re-exported from `windows` to keep the
    // integration test self-contained; the production call site in
    // P10 will use named constants from the `windows` crate. `VK_END`
    // matches the WPF auto-paste flow (`MainWindow.xaml.cs` L2222)
    // which uses it as a cursor-to-end primer before each field.
    const WM_KEYDOWN: u32 = 0x0100;
    const VK_END: u8 = 0x23;

    let _guard = ChildGuard(Some(
        Command::new("notepad.exe")
            .spawn()
            .expect("failed to spawn notepad.exe"),
    ));

    // Poll for the legacy `"Notepad"` class. Win11's UWP-shelled
    // Notepad uses different class names that vary between feature
    // updates — this lookup will time out there, which is the
    // documented Q7 limitation. Up to 5s for cold startup on a
    // loaded box.
    let deadline = Instant::now() + Duration::from_secs(5);
    let handle = loop {
        if let Some(h) = find_window(Some("Notepad"), None) {
            break h;
        }
        if Instant::now() >= deadline {
            panic!(
                "Notepad window did not appear within 5s. \
                 On Win11 the legacy class lookup may not match the \
                 UWP-shelled Notepad — see test docs (Q7 caveat)."
            );
        }
        thread::sleep(Duration::from_millis(100));
    };

    // Best-effort focus shift. Windows may refuse depending on the
    // foreground policy of the calling session; the production flow
    // tolerates this so the smoke does too.
    let _ = set_foreground_window(handle);

    // Q7 contract: each call returns `Ok` — content verification is
    // out of scope. `post_string` targets the top-level Notepad
    // window, not its child `Edit` control, so characters may not
    // appear in the editor; that's expected.
    post_string(handle, "abc").expect("post_string should succeed");
    post_key(handle, WM_KEYDOWN, VK_END).expect("post_key VK_END should succeed");

    // ChildGuard's Drop kills the spawned notepad; no explicit
    // `kill_process` call here because that primitive has its own
    // coverage in `tests/process_find_kill.rs`.
}
