//! Integration tests for [`services::process`][sp] — exercising the
//! chunk 9.1 primitives ([`find`][f] / [`kill`][k]) against a real
//! spawned child process, plus the chunk 9.2 helpers
//! ([`check_and_kill_patcher`][cakp] / [`close_play_window`][cpw]) as
//! smoke tests against the production code path (the DI-friendly
//! per-behaviour unit tests live in each module's `mod tests`).
//!
//! [sp]: beanfun_lib::services::process
//! [f]: beanfun_lib::services::process::find_processes_by_name
//! [k]: beanfun_lib::services::process::kill_process
//! [cakp]: beanfun_lib::services::process::check_and_kill_patcher
//! [cpw]: beanfun_lib::services::process::close_play_window
//!
//! Gated `#[cfg(target_os = "windows")]` because both the `wmi` crate
//! and the Win32 `OpenProcess`/`TerminateProcess`/`FindWindowW` APIs
//! are Windows-only. On other platforms the whole test binary
//! compiles to an empty `main` and the harness reports 0/0 tests.
//!
//! # Harness hygiene
//!
//! Every spawned child is owned by a [`ChildGuard`] whose `Drop`
//! kills + waits the child. Even if a test panics mid-assertion,
//! the child is reaped so subsequent test runs don't accumulate
//! orphan `cmd.exe` timers.

#![cfg(target_os = "windows")]

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use beanfun_lib::services::process::{
    check_and_kill_patcher, close_play_window, find_processes_by_name, kill_process, ProcessError,
};

/// RAII wrapper that guarantees a spawned child is killed on drop.
///
/// Tests interact with it by value (`guard.id()`, `guard.take()`) so
/// the borrow checker enforces "one caller, one child" semantics.
struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn id(&self) -> u32 {
        self.0.as_ref().expect("child already taken").id()
    }

    /// Extract ownership of the child so the caller can `wait()` it
    /// themselves. The guard's `Drop` becomes a no-op after this.
    fn take(&mut self) -> Child {
        self.0.take().expect("child already taken")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Spawn `cmd.exe /c ping -n 30 127.0.0.1 -w 1000` — a ~30-second
/// loop that holds a cmd.exe PID long enough for WMI to see it and
/// for terminate-then-wait to run.
///
/// `timeout` is avoided here: with `Stdio::null()` on stdin, `timeout`
/// bails with "ERROR: Input redirection is not supported" and cmd.exe
/// exits immediately, breaking the test harness. `ping` has no such
/// input dependency.
fn spawn_sleep_cmd() -> ChildGuard {
    let child = Command::new("cmd")
        .args(["/c", "ping", "-n", "30", "127.0.0.1", "-w", "1000"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn cmd.exe");
    ChildGuard(Some(child))
}

#[test]
fn find_processes_by_name_finds_our_spawned_cmd() {
    let guard = spawn_sleep_cmd();
    let our_pid = guard.id();

    // WMI's snapshot is eventually consistent; give the new process a
    // moment to show up. 500 ms is overkill for a fresh CreateProcess
    // but makes the test stable on loaded CI boxes.
    thread::sleep(Duration::from_millis(500));

    let rows = find_processes_by_name("cmd.exe").expect("find_processes_by_name failed");

    let ours = rows.iter().find(|p| p.pid == our_pid).unwrap_or_else(|| {
        panic!(
            "expected pid {our_pid} in WMI result for cmd.exe, got {:?}",
            rows.iter().map(|p| p.pid).collect::<Vec<_>>()
        )
    });

    assert!(
        ours.name.eq_ignore_ascii_case("cmd.exe"),
        "expected Name == cmd.exe (case-insensitive), got {:?}",
        ours.name
    );

    let exe_path = ours
        .executable_path
        .as_ref()
        .expect("spawned cmd.exe should have a non-null ExecutablePath");
    assert!(
        exe_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("cmd.exe"))
            .unwrap_or(false),
        "ExecutablePath should end in cmd.exe, got {}",
        exe_path.display()
    );
}

#[test]
fn kill_process_terminates_spawned_cmd() {
    let mut guard = spawn_sleep_cmd();
    let our_pid = guard.id();

    // Small settle so the child is fully initialized and
    // TerminateProcess has a cleanly running target.
    thread::sleep(Duration::from_millis(100));

    kill_process(our_pid).expect("kill_process should succeed");

    let mut child = guard.take();
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait().expect("try_wait failed") {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("cmd.exe did not exit within 3s after kill_process");
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    };

    // Exit-code parity with .NET Process.Kill() — the Rust
    // [`kill_process`] passes `u32::MAX` (== `0xFFFFFFFF`) to
    // `TerminateProcess`, which surfaces as `-1` when Windows
    // `ExitStatus::code()` reinterprets the DWORD as a signed i32.
    // A future refactor that swaps this magic (e.g. back to `1`)
    // would break observability downstream — this guard makes that
    // regression loud.
    assert_eq!(
        status.code(),
        Some(-1),
        "expected terminate-process exit code matching .NET Process.Kill() (u32::MAX / -1), got {:?}",
        status.code()
    );
}

#[test]
fn find_then_kill_round_trip() {
    // End-to-end: spawn cmd, locate our pid via WMI, kill it, verify
    // it's gone from a subsequent WMI query.
    let mut guard = spawn_sleep_cmd();
    let our_pid = guard.id();

    thread::sleep(Duration::from_millis(500));

    let before = find_processes_by_name("cmd.exe").expect("find before");
    assert!(
        before.iter().any(|p| p.pid == our_pid),
        "expected pid {our_pid} to be alive before kill"
    );

    kill_process(our_pid).expect("kill_process");

    // Wait for the child to actually exit before re-polling WMI —
    // TerminateProcess is async from our perspective.
    let mut child = guard.take();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && child.try_wait().expect("try_wait").is_none() {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        child.try_wait().expect("try_wait").is_some(),
        "child did not exit after kill"
    );

    // WMI's snapshot may still show the zombie for a beat; poll up
    // to 2s for it to disappear. This is the same "eventually
    // consistent" behavior WPF's checkPatcher timer was designed
    // around.
    let disappeared_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let after = find_processes_by_name("cmd.exe").expect("find after");
        if !after.iter().any(|p| p.pid == our_pid) {
            return;
        }
        if Instant::now() >= disappeared_deadline {
            panic!("pid {our_pid} still visible in WMI 2s after kill");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn kill_nonexistent_pid_surfaces_open_process_error() {
    // PID above 0xFFFF_FFF0 is never allocated in practice; serves as
    // a "definitely doesn't exist" input for the negative path.
    let err = kill_process(0xFFFF_FFF0).expect_err("kill_process must error");
    match err {
        ProcessError::OpenProcess { pid, .. } => assert_eq!(pid, 0xFFFF_FFF0),
        other => panic!("expected OpenProcess error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 9.2 smoke tests — production wiring for check_and_kill_patcher +
// close_play_window. The DI-friendly behavioural tests live in each
// module's `mod tests`; these just verify the real-WMI /
// real-FindWindowW path doesn't panic.
// ---------------------------------------------------------------------------

#[test]
fn check_and_kill_patcher_no_patcher_running_returns_empty() {
    // Bogus game path in a directory where no Patcher.exe is running
    // (nor has ever existed) — production code must go through
    // find_processes_by_name -> WMI and come back with an empty kill
    // list. Primary value: we catch a mis-wire where the DI defaults
    // feed the wrong name into WMI, or WMI itself refuses to talk on
    // this machine.
    let fake_game =
        std::path::Path::new(r"C:\this\path\definitely\does\not\exist\beanfun\MapleStory.exe");
    let killed = check_and_kill_patcher(fake_game).expect("check_and_kill_patcher should not Err");
    assert!(
        killed.is_empty(),
        "expected no patcher to match bogus game dir, got pids {killed:?}"
    );
}

#[test]
fn close_play_window_smoke_returns_ok() {
    // WPF parity: when the launcher window isn't present, the call
    // should complete with Ok(false) rather than raising. A common
    // case in CI / dev: no MapleStory session running.
    //
    // We intentionally do NOT strict-assert `== Ok(false)` because a
    // developer who happens to have the launcher open while running
    // tests would get Ok(true) back and — more importantly — we would
    // have posted WM_CLOSE to their live session. The weaker "returns
    // Ok(_)" assertion is enough to catch:
    //   - `FindWindowW` panicking or returning an unhandled variant
    //   - `to_wide_null` producing a malformed buffer
    //   - linkage regressions against user32.dll
    // which are the plausible breakage modes.
    let result = close_play_window();
    assert!(
        result.is_ok(),
        "close_play_window returned Err unexpectedly: {result:?}"
    );
}
