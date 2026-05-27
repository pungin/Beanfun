//! Game-launch Tauri commands — the thin async boundary between
//! the frontend's "Run Game" button and the service layer's
//! `launch_game` orchestrator.
//!
//! Ports the WPF `btn_Run_Game_Click` pipeline
//! (`Beanfun/MainWindow.xaml.cs` L1727-1900) and its neighbouring
//! game-path / process-management helpers to IPC. The split into
//! **six** separate commands follows the P10.3 Q4=A (split
//! list/kill) + Q6=B (detect = read+write one-shot) + Q8=A (D5a
//! first, risk last) decisions in `Todo.md`.
//!
//! # Chunk layout
//!
//! | D-step | Command(s)                                             | Status          |
//! | ------ | ------------------------------------------------------ | --------------- |
//! | D5a    | [`launch_game`]                                        | **this module** |
//! | D5b    | [`set_game_path`] / [`detect_game_path`]               | **this module** |
//! | D5c    | [`list_game_processes`] / [`kill_game_processes`]      | **this module** |
//! | D5d    | [`auto_paste`]                                         | **this module** |
//!
//! # D5a — `launch_game`
//!
//! The command is intentionally **thin**: it takes already-resolved
//! ingredients (`game_path`, `mode`, `command_line_template`,
//! `account`, `password`) from the frontend, assembles a
//! [`LaunchRequest`], and hands it to the pre-existing
//! [`services::game::launch_game`][svc] orchestrator under
//! [`tokio::task::spawn_blocking`]. All business logic —
//! path validation, locale-aware mode resolution, Normal /
//! LocaleRemulator dispatch, SHA-256 integrity checks on LR
//! resources, `ShellExecuteW` with `runas` verb — lives in the
//! service layer and is covered by the chunk 8.1 / 8.2 test suite.
//!
//! Config I/O (reading `Path.<gameCode>`, `startGameMode`, per-game
//! `CommandLine` template) is deliberately **not** done here —
//! those round-trips belong to [`commands::config`][cfg] (D2) and
//! the per-game INI pipeline (P11/P12). Keeping launch + config as
//! separate Tauri invocations preserves SRP and matches the "one
//! user-meaningful action per command" convention the rest of the
//! P10 command layer follows.
//!
//! # Credentials handling (P10.3 Q7=A)
//!
//! `account` and `password` cross IPC as plaintext `String`
//! parameters, matching the legacy WPF flow where
//! `MainWindow.account` / `MainWindow.password` are in-memory
//! strings that the launcher reads directly. The service-layer
//! [`LaunchRequest`] has a bespoke [`Debug`][std::fmt::Debug]
//! impl that redacts [`command_line`][lr::command_line] (post-
//! substitution) to prevent accidental leakage through
//! `tracing::debug!("{req:?}")`; this command inherits that
//! guarantee by using `LaunchRequest` verbatim.
//!
//! The [`build_command_line`] helper short-circuits to `""` when
//! **any** of `template` / `account` / `password` is empty, matching
//! the WPF guard at `MainWindow.xaml.cs` L1867-1879
//! (`account != null && password != null && account != ""
//! && password != "" && game_commandLine != ""`). This means
//! unauthenticated launches (games that don't accept CLI
//! credentials) work by passing empty strings from the frontend.
//!
//! # Blocking isolation
//!
//! [`services::game::launch_game`][svc] is a synchronous function
//! that ultimately calls either [`std::process::Command::spawn`]
//! (Normal mode) or `ShellExecuteW` via the `windows` crate (LR
//! mode on Windows). Both are blocking system calls that would
//! stall the `tokio` async runtime if called inline.
//! [`tokio::task::spawn_blocking`] offloads the whole orchestrator
//! onto the blocking thread pool (P10-Q5 = A). Granularity is the
//! **entire orchestrator**, not individual Win32 call sites — path
//! validation + LR resource release + ShellExecute together run
//! under one task so the async boundary has exactly one await
//! point (easier for future tracing spans, no intermediate
//! `Result` gymnastics).
//!
//! # Command-layer error codes
//!
//! See [`crate::commands::error`] for the full table. D5a / D5b
//! introduce two **command-only** codes (no service-layer
//! counterpart) for failures that happen in this module's
//! orchestration:
//!
//! | Code                                    | Origin                                                                                             |
//! | --------------------------------------- | -------------------------------------------------------------------------------------------------- |
//! | `launcher.spawn_blocking_failed`        | [`tokio::task::JoinError`] from a `spawn_blocking` call (task panicked or was aborted).            |
//! | `launcher.platform_unsupported`         | [`detect_game_path`] called on a non-Windows build (registry/default-path probes are Windows-specific). |
//!
//! Every `services::game::launch_game` result flows through the
//! existing [`From<GameError> for CommandError`][gfrom] in
//! [`crate::commands::error`], so `game.path_empty` /
//! `game.path_not_found` / `game.shellexecute_failed` etc. surface
//! unchanged without a second mapping layer (DRY).
//!
//! # D5b — `set_game_path` / `detect_game_path`
//!
//! Port of the WPF `selectedGameChanged` L574-607 branch that seeds
//! `Config.xml` with a game's executable directory. Two complementary
//! commands cover the user-meaningful halves:
//!
//! - [`set_game_path`] — store the user-chosen path for the given
//!   game code. Thin wrapper over
//!   [`crate::services::config::set_value`] with a standardised
//!   key format (see [`game_path_config_key`]). Cross-platform —
//!   the write side is just `Config.xml` I/O.
//! - [`detect_game_path`] — check `Config.xml` first, then fall
//!   back to registry lookup and common install directories,
//!   writing the discovered value back to `Config.xml` for future
//!   launches. This matches the P10.3 Q6 = B decision (read + write
//!   fused into one IPC call, matching WPF parity).
//!
//! ## Input shape (INI separation)
//!
//! Both commands take `dir_value_name` / `dir_reg` / `game_code` as
//! explicit parameters instead of reading them from a per-game INI.
//! The INI pipeline is a P11 concern (Vue frontend side) — keeping
//! launcher commands INI-agnostic means:
//!
//! 1. **SRP** — one command, one side effect. No hidden "also reads
//!    `MapleStory_TW.ini` to look up registry hive".
//! 2. **Testability** — unit tests can exercise the detect flow
//!    against synthetic `dir_reg` / `dir_value_name` without
//!    provisioning an INI.
//! 3. **Forward compat** — when P11 introduces a `read_game_ini`
//!    command the frontend can compose it with these calls without
//!    this module carrying the dependency.
//!
//! ## Config key format
//!
//! WPF uses `{dir_value_name}.{game_code}` (L575 / L590 / L604) —
//! e.g. `ExecPath.610074_T9`. [`game_path_config_key`] encapsulates
//! that format so neither side of the IPC boundary has to re-derive
//! it.
//!
//! ## `detect_game_path` body flow
//!
//! ```text
//! 1. key = game_path_config_key(dir_value_name, game_code)
//! 2. let cached = Config[key]
//!    if cached != "" → return Some(cached)          (no registry call)
//! 3. build registry candidates from dir_reg's hive prefix
//! 4. spawn_blocking { read candidates until one returns a value }
//! 5. if found → Config[key] = value                 (WPF L589-592)
//! 6. otherwise probe common Program Files install paths
//! 7. if found → Config[key] = value
//! 8. return the discovered value (Some / None)
//! ```
//!
//! Registry access is gated on `target_os = "windows"`; non-Windows
//! builds return [`launcher.platform_unsupported`] via
//! [`PLATFORM_UNSUPPORTED_CODE`]. [`set_game_path`] stays
//! unconditional — Config I/O is portable.
//!
//! ## Blocking isolation (detect_game_path)
//!
//! Unlike [`launch_game`] (whole orchestrator under one
//! `spawn_blocking`), [`detect_game_path`] keeps Config I/O on the
//! tokio runtime (it's natively `async`) and only wraps the
//! `winreg` call — the single synchronous island in the pipeline.
//! This is a finer-grained split than D5a's "one big blocking box"
//! rule because here the non-blocking parts genuinely exist: an
//! `async` Config read that resolves in memory, a synchronous
//! registry hit, and another `async` Config write. Three awaits is
//! clearer than one `spawn_blocking` wrapping all of it.
//!
//! # D5c — `list_game_processes` / `kill_game_processes`
//!
//! Ports the "is the game already running?" preflight block of the
//! WPF `btn_Run_Game_Click` flow (`MainWindow.xaml.cs` L1765-1833)
//! to IPC. WPF does list-then-confirm-then-kill inline; we split
//! that into two commands so the user-facing confirmation dialog
//! stays on the Vue side (P10.3 Q4 = A, stateless pair):
//!
//! - [`list_game_processes`] — enumerate every running process
//!   whose executable path byte-equals `game_path`. Returns a
//!   [`Vec<GameProcessInfo>`][GameProcessInfo] so the UI can render
//!   "2 instances of MapleStory.exe are running" with the
//!   matching exe paths.
//! - [`kill_game_processes`] — best-effort terminate the pids the
//!   frontend passes in. Returns the subset that actually died so
//!   the UI can re-check / re-render leftovers. **Does not**
//!   re-validate the pids against any game path — the design
//!   (P10.3 Q4 = A) puts the trust boundary at the frontend: it
//!   calls [`list_game_processes`] first, shows the confirm dialog,
//!   and only then forwards the resulting pids.
//!
//! ## IPC DTO vs service-layer type
//!
//! Service-layer [`crate::services::process::ProcessInfo`] is
//! Windows-only (the whole `services::process` module is
//! `#[cfg(target_os = "windows")]`). To keep the command signature
//! cross-platform — a hard requirement from the P10 chunk layout
//! so `bindings.ts` stays stable on dev boxes that `cargo check`
//! on macOS / Linux — we surface [`GameProcessInfo`], a
//! cross-platform DTO shaped as:
//!
//! ```text
//! { pid: u32, name: String, executable_path: Option<String> }
//! ```
//!
//! `executable_path` is `Option<String>` (rather than `PathBuf`) to
//! avoid leaking the specta `PathBuf` quirks to the frontend and to
//! let the UI treat missing paths uniformly. The conversion uses
//! [`std::path::Path::to_string_lossy`] — in practice every game
//! install path is ASCII so this is lossless; the docstring on
//! [`GameProcessInfo::executable_path`] spells that out for
//! pathological inputs.
//!
//! ## Blocking isolation (D5c)
//!
//! Both commands wrap their service-layer primitives in
//! [`tokio::task::spawn_blocking`]:
//!
//! - [`list_game_processes`] → `find_game_processes` (WMI query)
//! - [`kill_game_processes`] → `kill_game_processes` service
//!   (per-pid `OpenProcess` + `TerminateProcess`)
//!
//! Both primitives are synchronous Win32 / WMI calls — letting
//! them run inline would block the `current_thread` runtime flavor
//! (forbidden) and starve peers on the multi-threaded flavor.
//!
//! ## No new error codes (D5c)
//!
//! Every failure surfaces through existing mappings:
//!
//! - `process.wmi_init_failed` / `process.wmi_connect_failed` /
//!   `process.wmi_query_failed` / `process.open_process_failed` /
//!   `process.terminate_process_failed` — from the existing
//!   [`From<ProcessError> for CommandError`][pfrom] conversion.
//! - [`SPAWN_BLOCKING_FAILED_CODE`] — reused from D5a/D5b for
//!   Tokio `JoinError`.
//! - [`PLATFORM_UNSUPPORTED_CODE`] — reused from D5b for non-
//!   Windows builds. Both new commands `#[cfg]`-gate their bodies
//!   and fall through to the same error shape.
//!
//! # D5d — `auto_paste`
//!
//! Ports the credential hand-off at the tail of `getOtpWorker_RunWorkerCompleted`
//! (`MainWindow.xaml.cs` L2158-2238) to IPC. WPF fires this after
//! `services/beanfun` resolves the OTP for the selected account —
//! the frontend now owns the OTP string (the `check_otp` / `get_otp`
//! commands return it), so the command layer's responsibility is
//! just the Win32 sequence: find the launcher window, optionally
//! click through the SEA pre-login prompt, clear the account /
//! password fields, type the credentials, and submit.
//!
//! The orchestration itself lives in
//! [`crate::services::process::auto_paste::paste_credentials`]
//! (framework-agnostic, unit-tested against a recording
//! [`PasteDriver`][pd] mock). This command is the thin IPC wrapper.
//!
//! ## IPC DTO shape
//!
//! [`AutoPasteRequest`] groups the four parameters into one struct
//! (rather than four positional args) because:
//!
//! 1. **Readability** — call sites spell each field by name
//!    (`{ className, account, password, specialClick }`), so the
//!    frontend can't silently swap `account` and `password` in
//!    a refactor.
//! 2. **Specta friendliness** — generates a `AutoPasteRequest`
//!    TypeScript interface the Vue side can type against,
//!    instead of a positional tuple.
//! 3. **Future-proofing** — if WPF's hard-coded timings (100 ms /
//!    100 ms / 200 ms) ever need to become runtime-tunable,
//!    adding a `Duration` field to one struct is cheaper than
//!    a breaking-change to the command signature.
//!
//! ## `specialClick` dispatch (P10.3 Q2 decision)
//!
//! The service layer takes a single `bool` rather than the
//! `(service_code, service_region)` pair WPF tests (`== "610074"`
//! and `== "T9"`, L2195). The command layer stays agnostic about
//! "what counts as MapleStory SEA" — the frontend computes the
//! boolean from the selected game and forwards it here. Keeps
//! the Rust side free of MapleStory business rules that might
//! churn with future game additions.
//!
//! ## Blocking isolation (D5d)
//!
//! `paste_credentials` is synchronous end-to-end (Win32 FFI +
//! ~400 ms of `std::thread::sleep`). The command wraps the whole
//! orchestration in one [`tokio::task::spawn_blocking`] — same
//! granularity as D5a's "whole orchestrator" rule. The sleeps are
//! deliberately `thread::sleep` (not `tokio::time::sleep`) inside
//! the service layer because every step around them is already
//! sync FFI; crossing back into async just to sleep would force a
//! second `spawn_blocking` per step (see
//! [`auto_paste` module docs][am] Q4 for the full reasoning).
//!
//! ## Credentials handling (D5d inherits P10.3 Q7=A)
//!
//! `account` and `password` cross IPC as plaintext, identical to
//! [`launch_game`]. The D5d-specific risk: the password field is
//! typically the freshly-issued OTP (rotates every ~30 s),
//! narrowing the plaintext exposure window compared to launch_game's
//! long-lived account password. No extra redaction is added — the
//! frontend is expected to clear its OTP display state after the
//! paste completes.
//!
//! ## No new error codes (D5d)
//!
//! Every failure routes through existing mappings:
//!
//! - `process.window_not_found` — **new in D5d** at the service layer
//!   (`ProcessError::WindowNotFound`), surfaced through the existing
//!   [`From<ProcessError> for CommandError`][pfrom] conversion.
//!   Frontend branches on this code to fall back to clipboard-copy
//!   (mirrors WPF L2169-2174).
//! - `process.post_message_failed` / `process.win32_call_failed` /
//!   `process.non_ascii` — existing conversions from other
//!   `services/process` modules.
//! - [`SPAWN_BLOCKING_FAILED_CODE`] — reused for Tokio `JoinError`.
//! - [`PLATFORM_UNSUPPORTED_CODE`] — reused for non-Windows builds.
//!
//! [pfrom]: crate::commands::error#processerror--commanderror-servicesprocess
//! [lr::command_line]: crate::services::game::LaunchRequest::command_line
//! [svc]: crate::services::game::launch_game
//! [cfg]: crate::commands::config
//! [gfrom]: crate::commands::error#gameerror--commanderror-servicesgame
//! [`launcher.platform_unsupported`]: PLATFORM_UNSUPPORTED_CODE
//! [pd]: crate::services::process::auto_paste::PasteDriver
//! [am]: crate::services::process::auto_paste

use std::path::{Path, PathBuf};

use serde_json::json;
use tauri::State;

use crate::commands::config::config_xml_path;
use crate::commands::error::CommandError;
use crate::commands::state::AppState;
use crate::services::config as svc_config;
use crate::services::game::{self, substitute_credentials, GameStartMode, LaunchRequest};

/// IPC-shaped summary of a running game process, returned by
/// [`list_game_processes`].
///
/// # Cross-platform availability
///
/// This type is defined at the command layer (not re-exported from
/// [`crate::services::process`]) because the service-layer
/// [`ProcessInfo`][svc_pi] lives inside a
/// `#[cfg(target_os = "windows")]`-gated module. Surfacing the
/// DTO here lets [`list_game_processes`] keep a cross-platform
/// signature (the body errors out at runtime on non-Windows via
/// [`PLATFORM_UNSUPPORTED_CODE`]) so `cargo check` on macOS /
/// Linux dev boxes still produces a stable `bindings.ts`.
///
/// # Field semantics
///
/// | Field             | Matches                                                    |
/// | ----------------- | ---------------------------------------------------------- |
/// | `pid`             | `Win32_Process.ProcessId` (OS-level pid, stable for life)  |
/// | `name`            | `Win32_Process.Name` (executable file name **with** ext)   |
/// | `executable_path` | `Win32_Process.ExecutablePath` — see **path encoding** below |
///
/// ## Path encoding
///
/// `executable_path: Option<String>` is the UTF-8 form of the
/// service-layer `Option<PathBuf>`, produced via
/// [`std::path::Path::to_string_lossy`]. Windows paths that land
/// in `Win32_Process.ExecutablePath` are effectively always valid
/// Unicode (the filesystem stores them as UTF-16 and WMI hands us
/// the `String` form directly), so the `to_string_lossy` bridge
/// is lossless in practice. `None` when WMI returned `NULL` (the
/// process is protected or was mid-exit during enumeration).
///
/// [svc_pi]: crate::services::process::ProcessInfo
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameProcessInfo {
    /// OS-level process id, stable for the process's lifetime.
    pub pid: u32,

    /// Executable file name **including** the `.exe` extension
    /// (e.g. `"MapleStory.exe"`).
    pub name: String,

    /// UTF-8 path to the executable on disk, or `None` when WMI
    /// couldn't read it (protected process or mid-exit). See the
    /// struct-level "Path encoding" section for the conversion
    /// rationale.
    pub executable_path: Option<String>,
}

/// IPC-shaped input for [`auto_paste`].
///
/// Groups the four per-call parameters (window class, account,
/// password, SEA pre-click toggle) into one struct so the frontend
/// spells each field by name — see the D5d section in the module
/// docs for the rationale.
///
/// # Field semantics
///
/// | Field          | WPF origin                                                    |
/// | -------------- | ------------------------------------------------------------- |
/// | `class_name`   | `MainWindow.win_class_name` (L76, per-game INI column)        |
/// | `account`      | `bfClient.accountList[index].sid` (L2149)                     |
/// | `password`     | `MainWindow.otp` (fresh OTP from `services/beanfun`, L2150)   |
/// | `special_click`| `"610074".Equals(service_code) && "T9".Equals(service_region)` (L2195) |
///
/// The fallback to `MapleStoryClassTW` (WPF L2161) is **hardcoded**
/// inside [`crate::services::process::auto_paste`] — frontends
/// that pass `className = "MapleStoryClass"` get the fallback for
/// free; other class names go through without fallback (matches
/// WPF's `"MapleStoryClass".Equals(win_class_name)` guard).
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AutoPasteRequest {
    /// Top-level window class name of the launcher dialog
    /// (e.g. `"MapleStoryClass"`, `"NexonGameClass"`). Sourced
    /// from the per-game INI on the frontend side.
    pub class_name: String,

    /// Game account name to type into the login dialog. Must be
    /// ASCII — non-ASCII bytes surface as `process.non_ascii`
    /// (the existing Q3 contract from
    /// [`crate::services::process::post_string::post_string`]).
    pub account: String,

    /// Password (or OTP) to type into the password field. Same
    /// ASCII constraint as [`Self::account`].
    pub password: String,

    /// When `true`, inject the MapleStory-SEA pre-click sequence
    /// (ESC + synthetic click at ~50% / 40% of the client area)
    /// before typing credentials. WPF gates this on
    /// `service_code == "610074" && service_region == "T9"` —
    /// the command layer delegates the decision to the frontend
    /// (see module docs).
    pub special_click: bool,
}

/// Command-layer code minted when [`tokio::task::spawn_blocking`]
/// returns a [`tokio::task::JoinError`] (task panicked or was
/// aborted). Kept distinct from the [`crate::services::system::error::SystemError::SpawnBlockingFailed`]
/// code so UI telemetry can tell "launcher path panicked" apart
/// from "open_url path panicked" (P10.1 Q8.D4 fine-grained codes).
pub(crate) const SPAWN_BLOCKING_FAILED_CODE: &str = "launcher.spawn_blocking_failed";

/// Command-layer code returned by [`detect_game_path`] on
/// non-Windows build targets. Registry and Program Files probes are
/// implemented via Windows-specific APIs/conventions; dev boxes
/// (macOS / Linux) can still `cargo check` the command signature —
/// the body simply errors out at runtime.
///
/// Kept at module scope (rather than inlined into the non-Windows
/// fallback helper) so the `platform_unsupported_code_is_stable`
/// unit test can pin the exact string against rename drift — the
/// frontend contract depends on this specific value. Mirrors the
/// pattern established by [`crate::commands::storage`]'s
/// `storage.platform_unsupported` code.
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) const PLATFORM_UNSUPPORTED_CODE: &str = "launcher.platform_unsupported";

#[cfg(not(target_os = "windows"))]
fn platform_unsupported_error() -> CommandError {
    CommandError::new(
        PLATFORM_UNSUPPORTED_CODE,
        "detect_game_path requires Windows (registry/default-path lookup for game install path)",
    )
}

/// Format the `Config.xml` key for a game's executable directory.
///
/// WPF uses `{dir_value_name}.{game_code}` literally (see
/// `MainWindow.xaml.cs` L575 / L590 / L604) — e.g.
/// `ExecPath.610074_T9` for MapleStory TW. This helper is the
/// **single point of truth** for the format so a refactor that
/// accidentally flips the segment order (`"{game_code}.{dir_value_name}"`)
/// or changes the separator is caught by the
/// `game_path_config_key_format_is_dir_then_game` unit test rather
/// than silently losing every user's saved paths on upgrade.
///
/// Both [`set_game_path`] and [`detect_game_path`] route through
/// this helper (DRY) — the frontend never computes the key on its
/// own, so neither WPF → Rust nor renderer → Rust boundaries can
/// disagree on the format.
pub(crate) fn game_path_config_key(dir_value_name: &str, game_code: &str) -> String {
    format!("{dir_value_name}.{game_code}")
}

/// Build the `CreateProcess` / `ShellExecuteW` command-line string
/// from a WPF-style template with `%s` placeholders.
///
/// Mirrors the WPF guard at `MainWindow.xaml.cs` L1867-1879: when
/// any one of `template` / `account` / `password` is empty, the
/// command line is entirely skipped (the game is launched without
/// arguments). Otherwise, the first two `%s` placeholders are
/// replaced with `account` and `password` via
/// [`substitute_credentials`] — third-or-later `%s` are left
/// literal, matching the two-pass `Regex.Replace(..., 1)` quirk in
/// WPF.
///
/// Pulled out as a separate `pub(crate)` helper so the
/// empty-string short-circuit logic is independently unit-testable
/// (no `spawn_blocking` / `current_exe` dependencies) and kept
/// DRY: future launcher commands that might want to echo the
/// substituted command-line back to the UI (they shouldn't, due to
/// the plaintext-password concern — see module docs) would reuse
/// the same helper rather than re-deriving the guard.
pub(crate) fn build_command_line(template: &str, account: &str, password: &str) -> String {
    if template.is_empty() || account.is_empty() || password.is_empty() {
        String::new()
    } else {
        substitute_credentials(template, account, password)
    }
}

pub(crate) fn build_launch_request(
    storage_root: &Path,
    game_path: String,
    mode: GameStartMode,
    command_line_template: String,
    account: String,
    password: String,
) -> LaunchRequest {
    // WPF splits the INI `exe` field into `game_exe` (filename) and
    // `game_commandLine` (args with %s placeholders) via:
    //   game_exe = Regex("(.*).exe").Match(exe) + ".exe"
    //   game_commandLine = Regex(".exe (.*)").Match(exe)
    // Only `game_commandLine` is used for credential substitution.
    // We replicate the split here so the command line passed to
    // CreateProcess matches WPF byte-for-byte.
    let args_template = if let Some(pos) = command_line_template.to_ascii_lowercase().find(".exe ")
    {
        &command_line_template[pos + 5..]
    } else {
        ""
    };
    let command_line = build_command_line(args_template, &account, &password);

    LaunchRequest {
        game_path: PathBuf::from(game_path),
        command_line,
        mode,
        target_dir: storage_root.to_path_buf(),
    }
}

/// Launch the configured game binary with the current account
/// credentials.
///
/// Thin wrapper over [`crate::services::game::launch_game`] — see the
/// module-level docs for the full rationale, credential-handling
/// policy, and blocking-isolation contract. The command performs
/// three orchestration steps before delegating:
///
/// 1. Resolve the LocaleRemulator staging directory from
///    [`AppState::storage_root`], the same `%APPDATA%\Beanfun`
///    directory that holds `Config.xml`.
/// 2. Assemble the command-line string via [`build_command_line`]
///    (see that helper's docs for the empty-string short-circuit
///    semantics).
/// 3. Hand the [`LaunchRequest`] to the service orchestrator under
///    [`tokio::task::spawn_blocking`]. A [`tokio::task::JoinError`]
///    surfaces as `launcher.spawn_blocking_failed`; any
///    [`crate::services::game::GameError`] from the orchestrator
///    itself (path validation / LR resource release / ShellExecute
///    / Command::spawn) flows through the existing
///    [`From<GameError> for CommandError`][gfrom] conversion.
///
/// # Parameters
///
/// - `game_path` — absolute path to the game executable (e.g.
///   `C:\\Games\\MapleStory\\MapleStory.exe`). Frontend typically
///   reads this from `Config.xml` via `get_config_value` — this
///   command does not read Config itself (SRP).
/// - `mode` — user's requested launch mode. `Auto` will resolve
///   against the Windows system locale inside the service layer;
///   see [`crate::services::game::resolve_mode`]. Maps to the
///   legacy `startGameMode` integer config value on the frontend
///   side.
/// - `command_line_template` — per-game command-line template with
///   `%s` placeholders. Empty string disables credential
///   substitution entirely (the game is launched with no
///   arguments). Typically sourced from the per-game INI pipeline
///   that P11/P12 will implement.
/// - `account` / `password` — the logged-in game account
///   credentials. Both empty → no substitution (see
///   [`build_command_line`]). Plaintext over IPC by P10.3 Q7=A
///   decision; treat this command as sensitive at the callsite.
///
/// # Fire-and-forget
///
/// The spawned game process is detached — the service layer drops
/// the `std::process::Child` immediately on Normal mode, and
/// `ShellExecuteW` takes care of its own child on LR mode. There
/// is no `pid` returned, no lifecycle tracking: matches the legacy
/// WPF behaviour (P10.3 Q5 = A "stateless process commands").
///
/// [gfrom]: crate::commands::error#gameerror--commanderror-servicesgame
#[tauri::command]
#[specta::specta]
pub async fn launch_game(
    state: State<'_, AppState>,
    game_path: String,
    mode: GameStartMode,
    command_line_template: String,
    account: String,
    password: String,
) -> Result<(), CommandError> {
    launch_game_from_storage_root(
        state.storage_root.clone(),
        game_path,
        mode,
        command_line_template,
        account,
        password,
    )
    .await
}

pub(crate) async fn launch_game_from_storage_root(
    storage_root: PathBuf,
    game_path: String,
    mode: GameStartMode,
    command_line_template: String,
    account: String,
    password: String,
) -> Result<(), CommandError> {
    let template_len = command_line_template.len();
    let has_account = !account.is_empty();
    let has_password = !password.is_empty();
    let req = build_launch_request(
        &storage_root,
        game_path,
        mode,
        command_line_template,
        account,
        password,
    );

    tracing::info!(
        game_path = %req.game_path.display(),
        mode = ?mode,
        template_len,
        command_line_len = req.command_line.len(),
        target_dir = %req.target_dir.display(),
        has_account,
        has_password,
        "launch_game: preparing to spawn"
    );

    tokio::task::spawn_blocking(move || game::launch_game(&req))
        .await
        .map_err(|join_err| {
            CommandError::new(
                SPAWN_BLOCKING_FAILED_CODE,
                format!("launch_game spawn_blocking failed: {join_err}"),
            )
            .with_details(json!({
                "is_panic": join_err.is_panic(),
                "is_cancelled": join_err.is_cancelled(),
            }))
        })??;

    Ok(())
}

/// Persist the user-chosen game install path for `game_code` into
/// `Config.xml`.
///
/// Thin wrapper over [`crate::services::config::set_value`] —
/// see the D5b section in the module docs for the Config key format
/// and the rationale for keeping `dir_value_name` / `game_code` as
/// explicit parameters (INI separation).
///
/// # Parameters
///
/// - `game_code` — composite key the settings page supplies (e.g.
///   `"610074_T9"`, from `service_code + "_" + service_region`).
/// - `dir_value_name` — INI-sourced column name (e.g. `"ExecPath"`);
///   becomes the prefix of the Config.xml key.
/// - `path` — the chosen executable-dir path. Empty string is
///   accepted and written verbatim; callers that want to *remove*
///   the entry entirely should use
///   [`crate::commands::config::set_config`] with `value = None`.
///
/// # Errors
///
/// - `config.io_failed` / `config.xml_write_failed` — see
///   [`crate::services::config::ConfigError`] for the full surface.
///
/// # Platform
///
/// Unconditional — Config I/O is portable. Only
/// [`detect_game_path`] requires Windows (registry lookup).
#[tauri::command]
#[specta::specta]
pub async fn set_game_path(
    state: State<'_, AppState>,
    game_code: String,
    dir_value_name: String,
    path: String,
) -> Result<(), CommandError> {
    let config_path = config_xml_path(&state);
    let key = game_path_config_key(&dir_value_name, &game_code);
    svc_config::set_value(&config_path, &key, Some(&path)).await?;
    Ok(())
}

/// Resolve the install path for `game_code`, consulting
/// `Config.xml` first and falling back to the Windows registry,
/// then common install directories. Writes any freshly-discovered
/// value back to Config so future calls are fast.
///
/// Returns:
/// - `Ok(Some(path))` — Config already had a value, the registry
///   supplied one, or a default install-path probe found the exe.
///   Freshly-discovered values are written back to Config.
/// - `Ok(None)` — Config, registry, and default install-path probes
///   all came up empty. WPF shows an empty `t_GamePath` textbox in
///   this case; this shape lets the frontend render the same way
///   without another round-trip.
///
/// # Parameters
///
/// - `game_code` — composite identifier (`service_code_region`).
/// - `dir_value_name` — INI-sourced Config column name and
///   registry `REG_SZ` value name (WPF reuses the same string for
///   both, L574 / L587).
/// - `dir_reg` — INI-sourced registry subkey path. Explicit
///   `HKEY_LOCAL_MACHINE\` / `HKEY_CURRENT_USER\` prefixes drive
///   the matching hive lookup; unprefixed paths try HKCU first,
///   then HKLM.
/// - `command_line_template` — INI-sourced executable template.
///   The first token is used to derive the executable filename for
///   default install-path probes.
///
/// # Errors
///
/// - `config.io_failed` / `config.xml_write_failed` — the Config
///   write-back step failed after a successful registry read.
/// - `registry.open_key_failed` / `registry.read_value_failed` —
///   the registry lookup surfaced a non-NotFound IO error (e.g.
///   permission denied). NotFound / empty value / missing subkey
///   are **not** errors — they fold into `Ok(None)` per WPF's
///   silent fallback at L596-599.
/// - `launcher.spawn_blocking_failed` — the registry-read
///   `spawn_blocking` task panicked or was cancelled.
/// - `launcher.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn detect_game_path(
    state: State<'_, AppState>,
    game_code: String,
    dir_value_name: String,
    dir_reg: String,
    command_line_template: String,
) -> Result<Option<String>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        detect_imp::detect_game_path_impl(
            &state,
            game_code,
            dir_value_name,
            dir_reg,
            command_line_template,
        )
        .await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (
            state,
            game_code,
            dir_value_name,
            dir_reg,
            command_line_template,
        );
        Err(platform_unsupported_error())
    }
}

/// Enumerate every running process whose executable path byte-equals
/// `game_path`.
///
/// Thin wrapper over
/// [`crate::services::process::game::find_game_processes`] — see the
/// D5c section in the module docs for the WPF parity contract and the
/// IPC DTO rationale. The returned [`GameProcessInfo`] list is empty
/// when nothing matches (not an error).
///
/// # Parameters
///
/// - `game_path` — absolute path to the game executable. Typically
///   the same value the frontend later passes to [`launch_game`];
///   the match is byte-exact against `Win32_Process.ExecutablePath`,
///   so "same exe name under a different install directory" is
///   deliberately treated as a different game (e.g. two MapleStory
///   installs don't interfere).
///
/// # Fire-pattern
///
/// Designed to be called **before** [`launch_game`] so the UI can
/// prompt the user to close existing instances first. The frontend
/// then forwards any pids the user confirmed into
/// [`kill_game_processes`]. The separation (list vs kill) keeps the
/// confirm dialog on the Vue side and lets the backend stay
/// stateless (P10.3 Q4 = A).
///
/// # Errors
///
/// - `process.wmi_init_failed` / `process.wmi_connect_failed` /
///   `process.wmi_query_failed` — from the underlying WMI round-trip,
///   via [`From<ProcessError> for CommandError`][pfrom].
/// - `launcher.spawn_blocking_failed` — the `spawn_blocking` task
///   panicked or was cancelled.
/// - `launcher.platform_unsupported` — non-Windows build target.
///
/// [pfrom]: crate::commands::error#processerror--commanderror-servicesprocess
#[tauri::command]
#[specta::specta]
pub async fn list_game_processes(game_path: String) -> Result<Vec<GameProcessInfo>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        list_imp::list_game_processes_impl(game_path).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = game_path;
        Err(platform_unsupported_error())
    }
}

/// Best-effort terminate every pid in `pids`, returning the subset
/// that was actually killed.
///
/// Thin wrapper over
/// [`crate::services::process::game::kill_game_processes`] — see the
/// D5c section in the module docs for the best-effort semantics and
/// the frontend trust-boundary rationale.
///
/// # Parameters
///
/// - `pids` — the pids to terminate. **Not re-validated** against
///   any game path; the frontend is expected to have just called
///   [`list_game_processes`] and obtained explicit user consent
///   before forwarding the pids here. This matches the WPF
///   inline "Yes" branch at `MainWindow.xaml.cs` L1821-1833 which
///   kills from the list it just computed without a second
///   validation pass.
///
/// # Returns
///
/// `Vec<u32>` of pids that were successfully terminated, in input
/// order. Per-pid failures (process exited mid-kill, permission
/// denied, protected process) are silently skipped — callers that
/// need to surface leftovers should re-invoke [`list_game_processes`]
/// and diff. An empty input produces an empty output without any
/// `OpenProcess`/`TerminateProcess` calls.
///
/// # Errors
///
/// - `launcher.spawn_blocking_failed` — the `spawn_blocking` task
///   panicked or was cancelled. No `process.*` errors surface here
///   because the service-layer primitive swallows per-pid failures
///   by design.
/// - `launcher.platform_unsupported` — non-Windows build target.
#[tauri::command]
#[specta::specta]
pub async fn kill_game_processes(pids: Vec<u32>) -> Result<Vec<u32>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        list_imp::kill_game_processes_impl(pids).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pids;
        Err(platform_unsupported_error())
    }
}

/// Type the account name + OTP into the MapleStory launcher's
/// login dialog and press Enter, replicating the tail of
/// `getOtpWorker_RunWorkerCompleted`
/// (`Beanfun/MainWindow.xaml.cs` L2158-2238).
///
/// Thin wrapper over
/// [`crate::services::process::auto_paste::paste_credentials`] —
/// see the D5d section in the module docs for the full design
/// breakdown, DTO rationale, and `specialClick` dispatch contract.
///
/// # Parameters (shape pinned by [`AutoPasteRequest`])
///
/// - `className` — launcher window class to target; the
///   `MapleStoryClassTW` fallback is applied automatically when
///   `className == "MapleStoryClass"`.
/// - `account` / `password` — credentials to type. Both must be
///   ASCII; non-ASCII surfaces as `process.non_ascii`.
/// - `specialClick` — run the SEA pre-login dismiss + click
///   pipeline (`true` on MapleStory SEA / TW, `false` elsewhere).
///
/// # Fire pattern
///
/// Frontend typically calls this **after** successfully retrieving
/// the OTP for the selected account, and **after** the user has
/// either let the auto-launch happen or opened the launcher dialog
/// manually. On a `process.window_not_found` response, the UI is
/// expected to fall back to clipboard-copying the OTP (mirrors
/// WPF L2169-2174).
///
/// # Errors
///
/// - `process.window_not_found` — no launcher window of the given
///   class exists. Frontend should copy the password to clipboard
///   and surface the OTP for manual paste.
/// - `process.post_message_failed` / `process.win32_call_failed` —
///   the target window went away mid-paste.
/// - `process.non_ascii` — `account` or `password` contains a
///   non-ASCII codepoint; WPF silently replaces with `'?'`
///   (corrupting credentials), the Rust port refuses loudly.
/// - `launcher.spawn_blocking_failed` — the `spawn_blocking` task
///   panicked or was cancelled.
/// - `launcher.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn auto_paste(req: AutoPasteRequest) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        paste_imp::auto_paste_impl(req).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = req;
        Err(platform_unsupported_error())
    }
}

// =====================================================================
// Windows-only registry lookup for detect_game_path
// =====================================================================

#[cfg(target_os = "windows")]
mod detect_imp {
    use super::*;
    use crate::services::registry::{self, Hive};

    /// Registry hive prefixes accepted from INI `dir_reg` values.
    pub(super) const HKLM_PREFIX: &str = "HKEY_LOCAL_MACHINE\\";
    pub(super) const HKCU_PREFIX: &str = "HKEY_CURRENT_USER\\";

    pub(super) fn registry_lookup_candidates(dir_reg: &str) -> Vec<(Hive, String)> {
        let dir_reg = dir_reg.trim();
        if dir_reg.is_empty() {
            return Vec::new();
        }

        let mut candidates = if let Some(subkey) = dir_reg.strip_prefix(HKLM_PREFIX) {
            vec![
                (Hive::LocalMachine, subkey.to_string()),
                (Hive::CurrentUser, subkey.to_string()),
            ]
        } else if let Some(subkey) = dir_reg.strip_prefix(HKCU_PREFIX) {
            vec![(Hive::CurrentUser, subkey.to_string())]
        } else {
            vec![
                (Hive::CurrentUser, dir_reg.to_string()),
                (Hive::LocalMachine, dir_reg.to_string()),
            ]
        };

        candidates.retain(|(_, subkey)| !subkey.is_empty());
        candidates
    }

    fn executable_name_from_template(command_line_template: &str) -> Option<String> {
        let first_token = command_line_template.split_whitespace().next()?;
        Path::new(first_token)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    }

    fn default_install_dir_names(dir_reg: &str, exe_name: &str) -> Vec<String> {
        let mut names = Vec::new();
        let subkey = dir_reg
            .strip_prefix(HKLM_PREFIX)
            .or_else(|| dir_reg.strip_prefix(HKCU_PREFIX))
            .unwrap_or(dir_reg);
        if let Some(leaf) = subkey.rsplit('\\').next().filter(|leaf| !leaf.is_empty()) {
            names.push(leaf.to_string());
        }

        if let Some(stem) = Path::new(exe_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
        {
            if !names.iter().any(|name| name.eq_ignore_ascii_case(stem)) {
                names.push(stem.to_string());
            }
        }

        names
    }

    fn program_files_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for key in ["ProgramFiles(x86)", "ProgramW6432", "ProgramFiles"] {
            if let Some(path) = std::env::var_os(key).map(PathBuf::from) {
                if !roots
                    .iter()
                    .any(|existing: &PathBuf| paths_eq_ignore_ascii_case(existing, &path))
                {
                    roots.push(path);
                }
            }
        }
        roots
    }

    fn paths_eq_ignore_ascii_case(left: &Path, right: &Path) -> bool {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }

    pub(super) fn default_install_path_candidates(
        dir_reg: &str,
        command_line_template: &str,
    ) -> Vec<PathBuf> {
        let Some(exe_name) = executable_name_from_template(command_line_template) else {
            return Vec::new();
        };

        let mut candidates = Vec::new();
        let dir_names = default_install_dir_names(dir_reg, &exe_name);
        for root in program_files_roots() {
            for dir_name in &dir_names {
                candidates.push(root.join("Gamania").join(dir_name).join(&exe_name));
                candidates.push(root.join(dir_name).join(&exe_name));
            }
        }

        let mut unique = Vec::new();
        for candidate in candidates {
            if !unique
                .iter()
                .any(|existing: &PathBuf| paths_eq_ignore_ascii_case(existing, &candidate))
            {
                unique.push(candidate);
            }
        }
        unique
    }

    fn first_existing_default_path(dir_reg: &str, command_line_template: &str) -> Option<String> {
        default_install_path_candidates(dir_reg, command_line_template)
            .into_iter()
            .find(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned())
    }

    pub(super) async fn detect_game_path_impl(
        state: &AppState,
        game_code: String,
        dir_value_name: String,
        dir_reg: String,
        command_line_template: String,
    ) -> Result<Option<String>, CommandError> {
        let config_path = config_xml_path(state);
        let key = game_path_config_key(&dir_value_name, &game_code);

        let cached = svc_config::get_value(&config_path, &key).await;
        if !cached.is_empty() {
            return Ok(Some(cached));
        }

        let registry_candidates = registry_lookup_candidates(&dir_reg);
        let value_name = dir_value_name.clone();
        let registry_value = tokio::task::spawn_blocking(
            move || -> Result<Option<String>, registry::RegistryError> {
                for (hive, subkey) in registry_candidates {
                    if let Some(value) = registry::read_game_path(hive, &subkey, &value_name)? {
                        return Ok(Some(value));
                    }
                }
                Ok(None)
            },
        )
        .await
        .map_err(|join_err| {
            CommandError::new(
                SPAWN_BLOCKING_FAILED_CODE,
                format!("detect_game_path spawn_blocking failed: {join_err}"),
            )
            .with_details(json!({
                "is_panic": join_err.is_panic(),
                "is_cancelled": join_err.is_cancelled(),
            }))
        })??;

        if let Some(ref v) = registry_value {
            svc_config::set_value(&config_path, &key, Some(v.as_str())).await?;
            return Ok(registry_value);
        }

        let default_path = first_existing_default_path(&dir_reg, &command_line_template);
        if let Some(ref path) = default_path {
            svc_config::set_value(&config_path, &key, Some(path.as_str())).await?;
        }

        Ok(default_path)
    }
}

// =====================================================================
// Windows-only game process enumeration + kill for D5c
// =====================================================================

#[cfg(target_os = "windows")]
mod list_imp {
    //! Windows-only implementations for [`super::list_game_processes`]
    //! and [`super::kill_game_processes`]. Kept in a sub-module so
    //! the `#[cfg]` gate applies to one place — the outer commands
    //! stay cross-platform and route into here only on Windows
    //! builds.
    use super::*;
    use crate::services::process::game::{
        find_game_processes as svc_find_game_processes,
        kill_game_processes as svc_kill_game_processes,
    };
    use crate::services::process::ProcessInfo;

    /// Convert a service-layer [`ProcessInfo`] into the IPC-shaped
    /// [`GameProcessInfo`] DTO. Kept as a named helper (rather than
    /// inlined in the closure) so the path encoding rule has one
    /// home and is independently unit-testable.
    pub(super) fn into_dto(info: ProcessInfo) -> GameProcessInfo {
        GameProcessInfo {
            pid: info.pid,
            name: info.name,
            executable_path: info
                .executable_path
                .map(|p| p.to_string_lossy().into_owned()),
        }
    }

    pub(super) async fn list_game_processes_impl(
        game_path: String,
    ) -> Result<Vec<GameProcessInfo>, CommandError> {
        let infos =
            tokio::task::spawn_blocking(move || svc_find_game_processes(&PathBuf::from(game_path)))
                .await
                .map_err(|join_err| {
                    CommandError::new(
                        SPAWN_BLOCKING_FAILED_CODE,
                        format!("list_game_processes spawn_blocking failed: {join_err}"),
                    )
                    .with_details(json!({
                        "is_panic": join_err.is_panic(),
                        "is_cancelled": join_err.is_cancelled(),
                    }))
                })??;

        Ok(infos.into_iter().map(into_dto).collect())
    }

    pub(super) async fn kill_game_processes_impl(pids: Vec<u32>) -> Result<Vec<u32>, CommandError> {
        tokio::task::spawn_blocking(move || svc_kill_game_processes(&pids))
            .await
            .map_err(|join_err| {
                CommandError::new(
                    SPAWN_BLOCKING_FAILED_CODE,
                    format!("kill_game_processes spawn_blocking failed: {join_err}"),
                )
                .with_details(json!({
                    "is_panic": join_err.is_panic(),
                    "is_cancelled": join_err.is_cancelled(),
                }))
            })
    }
}

// =====================================================================
// Windows-only auto-paste orchestration (D5d)
// =====================================================================

#[cfg(target_os = "windows")]
mod paste_imp {
    //! Windows-only implementation for [`super::auto_paste`]. The
    //! whole `paste_credentials` orchestration is synchronous
    //! (Win32 FFI + `thread::sleep`), so the body is one
    //! `spawn_blocking` call — mirrors D5a's "whole orchestrator
    //! under one boundary" rule rather than D5b's fine-grained
    //! split.
    use super::*;
    use crate::services::process::auto_paste::{
        paste_credentials as svc_paste_credentials, PasteRequest,
    };

    pub(super) async fn auto_paste_impl(req: AutoPasteRequest) -> Result<(), CommandError> {
        tokio::task::spawn_blocking(move || {
            svc_paste_credentials(PasteRequest {
                class_name: &req.class_name,
                account: &req.account,
                password: &req.password,
                special_click: req.special_click,
            })
        })
        .await
        .map_err(|join_err| {
            CommandError::new(
                SPAWN_BLOCKING_FAILED_CODE,
                format!("auto_paste spawn_blocking failed: {join_err}"),
            )
            .with_details(json!({
                "is_panic": join_err.is_panic(),
                "is_cancelled": join_err.is_cancelled(),
            }))
        })??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for D5a launcher commands.
    //!
    //! Coverage is intentionally narrow at the command layer — the
    //! heavy lifting (path validation, mode resolution, LR release,
    //! ShellExecuteW wiring, Normal-mode spawn) is already exercised
    //! by the ~30 chunk 8.1 / 8.2 tests in
    //! [`crate::services::game`], and the
    //! `GameError → CommandError` mapping is pinned by the
    //! `from_impls_tests::game_*` cases in
    //! [`crate::commands::error`]. The cases here cover the pieces
    //! **this** module adds on top:
    //!
    //! - [`build_command_line`] empty-short-circuit + substitute pass-through
    //! - [`GameStartMode`] IPC serde shape (bare unit-variant string, per Q2 decision)
    //! - `launch_game` command-layer error paths that don't touch
    //!   platform-specific APIs (empty `game_path` → `game.path_empty`,
    //!   missing file → `game.path_not_found`).
    //!
    //! The two platform-dependent success paths (Normal spawns
    //! `cmd.exe`; LR on Windows invokes `ShellExecuteW`) are
    //! covered by the service-layer tests under
    //! [`crate::services::game::launcher::tests`] — reproducing them
    //! here would duplicate fixture setup without exercising any
    //! command-layer code.
    use super::*;

    // ---- build_command_line ---------------------------------------------

    #[test]
    fn build_command_line_all_present_substitutes() {
        let got = build_command_line("/u:%s /p:%s", "alice", "swordfish");
        assert_eq!(got, "/u:alice /p:swordfish");
    }

    #[test]
    fn build_command_line_empty_template_returns_empty() {
        // Empty template guard — matches WPF L1872 `game_commandLine != ""`.
        let got = build_command_line("", "alice", "swordfish");
        assert_eq!(got, "");
    }

    #[test]
    fn build_command_line_empty_account_returns_empty() {
        // WPF L1870 `account != ""` guard. Even though
        // `substitute_credentials` would happily produce
        // `/u: /p:swordfish` on its own, the wrapper short-circuits
        // to parity with WPF's "no-credentials" launch path.
        let got = build_command_line("/u:%s /p:%s", "", "swordfish");
        assert_eq!(got, "");
    }

    #[test]
    fn build_command_line_empty_password_returns_empty() {
        // WPF L1871 `password != ""` guard.
        let got = build_command_line("/u:%s /p:%s", "alice", "");
        assert_eq!(got, "");
    }

    #[test]
    fn build_command_line_three_slots_leaves_third_literal() {
        // Delegated to substitute_credentials — this test exists to
        // lock the delegation (not a re-test of the helper itself):
        // if someone replaces `substitute_credentials` with a `%s`
        // regex that touches every slot, this case will trip.
        let got = build_command_line("%s/%s/%s", "alice", "swordfish");
        assert_eq!(got, "alice/swordfish/%s");
    }

    #[test]
    fn build_launch_request_uses_storage_root_as_lr_target_dir() {
        let storage_root = PathBuf::from(r"C:\Users\Alice\AppData\Roaming\Beanfun");
        let req = build_launch_request(
            &storage_root,
            r"C:\Games\MapleStory\MapleStory.exe".into(),
            GameStartMode::LocaleRemulator,
            "MapleStory.exe /u:%s /p:%s".into(),
            "alice".into(),
            "swordfish".into(),
        );

        assert_eq!(req.target_dir, storage_root);
        assert_eq!(req.command_line, "/u:alice /p:swordfish");
    }

    // ---- GameStartMode IPC serde ----------------------------------------

    #[test]
    fn game_start_mode_serializes_as_bare_string() {
        // Frontend reads the legacy `startGameMode` integer from
        // Config (`"0"` / `"1"` / `"2"`) and maps to the enum via
        // plain string match before calling `launch_game`. Pin the
        // serialisation shape to catch an accidental
        // `#[serde(tag = "kind")]` that would wrap the value in an
        // object and silently break the frontend form.
        let auto = serde_json::to_string(&GameStartMode::Auto).expect("serialize");
        let normal = serde_json::to_string(&GameStartMode::Normal).expect("serialize");
        let lr = serde_json::to_string(&GameStartMode::LocaleRemulator).expect("serialize");
        assert_eq!(auto, "\"Auto\"");
        assert_eq!(normal, "\"Normal\"");
        assert_eq!(lr, "\"LocaleRemulator\"");
    }

    #[test]
    fn game_start_mode_deserializes_from_bare_string() {
        let auto: GameStartMode = serde_json::from_str("\"Auto\"").expect("deserialize");
        let normal: GameStartMode = serde_json::from_str("\"Normal\"").expect("deserialize");
        let lr: GameStartMode = serde_json::from_str("\"LocaleRemulator\"").expect("deserialize");
        assert_eq!(auto, GameStartMode::Auto);
        assert_eq!(normal, GameStartMode::Normal);
        assert_eq!(lr, GameStartMode::LocaleRemulator);
    }

    // ---- launch_game error-path integration -----------------------------

    #[tokio::test]
    async fn launch_game_empty_path_surfaces_game_path_empty() {
        // Exercise the full command body (target_dir resolve +
        // build_command_line + spawn_blocking + GameError →
        // CommandError) on an error that doesn't require a real
        // game binary on the test runner. The service-layer test
        // `launch_game_surfaces_validate_path_errors` already
        // covers the underlying GameError::PathEmpty; this test
        // adds the command-layer contract (correct code string,
        // async+spawn_blocking wiring).
        let dir = tempfile::TempDir::new().expect("tempdir");
        let err = launch_game_from_storage_root(
            dir.path().to_path_buf(),
            String::new(),
            GameStartMode::Normal,
            String::new(),
            String::new(),
            String::new(),
        )
        .await
        .expect_err("empty game_path must surface an error");
        assert_eq!(err.code, "game.path_empty");
    }

    #[tokio::test]
    async fn launch_game_missing_file_surfaces_game_path_not_found() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let missing = dir.path().join("does-not-exist.exe");
        let err = launch_game_from_storage_root(
            dir.path().to_path_buf(),
            missing.to_string_lossy().into_owned(),
            GameStartMode::Normal,
            String::new(),
            String::new(),
            String::new(),
        )
        .await
        .expect_err("missing game_path must surface an error");
        assert_eq!(err.code, "game.path_not_found");
    }

    // ---- game_path_config_key -------------------------------------------

    #[test]
    fn game_path_config_key_format_is_dir_then_game() {
        // Parity lock with WPF `MainWindow.xaml.cs` L575 / L590 /
        // L604: `dir_value_name + "." + gameCode`. A refactor that
        // flipped the order or changed the separator would silently
        // orphan every user's saved path on upgrade; this test
        // pins the wire shape at the one helper the rest of the
        // module routes through.
        let got = game_path_config_key("ExecPath", "610074_T9");
        assert_eq!(got, "ExecPath.610074_T9");
    }

    #[test]
    fn game_path_config_key_with_empty_game_code_still_includes_dot() {
        // Defensive: WPF would produce `"ExecPath."` too — caller
        // passing an empty game_code is already off-script but we
        // don't want to silently produce a *different* key shape
        // that might collide with another game's entry.
        let got = game_path_config_key("ExecPath", "");
        assert_eq!(got, "ExecPath.");
    }

    // ---- set_game_path / detect_game_path (cross-platform path) ---------

    fn temp_app_state() -> (tempfile::TempDir, std::sync::Arc<AppState>) {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let state = std::sync::Arc::new(AppState::new(dir.path().to_path_buf()));
        (dir, state)
    }

    #[tokio::test]
    async fn set_game_path_writes_value_that_get_config_value_reads_back() {
        // Mirrors the commands/config.rs test style: exercise the
        // helpers that the `#[tauri::command]` body delegates to,
        // without standing up a full `tauri::Manager` for the
        // `State<_, AppState>` wrapper. If the key format or
        // set_value semantics change, this test fails loudly.
        let (_dir, state) = temp_app_state();
        let config_path = config_xml_path(&state);
        let key = game_path_config_key("ExecPath", "610074_T9");

        svc_config::set_value(&config_path, &key, Some(r"C:\\Games\\MapleStory"))
            .await
            .expect("set");

        let read_back = svc_config::get_value(&config_path, &key).await;
        assert_eq!(read_back, r"C:\\Games\\MapleStory");
    }

    #[tokio::test]
    async fn set_game_path_empty_string_writes_empty_value() {
        // Empty `path` is accepted verbatim (see command docs). This
        // makes future `detect_game_path` treat the slot as "unset"
        // (WPF `Config[key] == ""` guard) without a separate
        // remove-key branch.
        let (_dir, state) = temp_app_state();
        let config_path = config_xml_path(&state);
        let key = game_path_config_key("ExecPath", "610074_T9");

        svc_config::set_value(&config_path, &key, Some(""))
            .await
            .expect("set empty");

        let read_back = svc_config::get_value(&config_path, &key).await;
        assert_eq!(read_back, "");
    }

    // ---- detect_game_path Windows-only pieces ---------------------------

    #[cfg(target_os = "windows")]
    #[test]
    fn detect_registry_candidates_parse_hive_prefixes() {
        use super::detect_imp::{registry_lookup_candidates, HKCU_PREFIX, HKLM_PREFIX};
        use crate::services::registry::Hive;

        assert_eq!(HKLM_PREFIX, "HKEY_LOCAL_MACHINE\\");
        assert_eq!(HKCU_PREFIX, "HKEY_CURRENT_USER\\");
        assert_eq!(
            registry_lookup_candidates(r"HKEY_LOCAL_MACHINE\SOFTWARE\Gamania\MapleStory"),
            vec![
                (Hive::LocalMachine, r"SOFTWARE\Gamania\MapleStory".into()),
                (Hive::CurrentUser, r"SOFTWARE\Gamania\MapleStory".into()),
            ]
        );
        assert_eq!(
            registry_lookup_candidates(r"HKEY_CURRENT_USER\SOFTWARE\Gamania"),
            vec![(Hive::CurrentUser, r"SOFTWARE\Gamania".into())]
        );
        assert_eq!(
            registry_lookup_candidates(r"SOFTWARE\Gamania\MapleStory"),
            vec![
                (Hive::CurrentUser, r"SOFTWARE\Gamania\MapleStory".into()),
                (Hive::LocalMachine, r"SOFTWARE\Gamania\MapleStory".into()),
            ],
            "unprefixed INI paths keep the historical HKCU-first probe"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn detect_default_install_candidates_use_ini_leaf_and_exe_name() {
        use super::detect_imp::default_install_path_candidates;

        let candidates = default_install_path_candidates(
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Gamania\MapleStory",
            "MapleStory.exe tw.login.maplestory.beanfun.com 8484 BeanFun %s %s",
        );

        assert!(
            candidates.iter().any(|path| {
                let s = path.to_string_lossy();
                s.ends_with(r"Gamania\MapleStory\MapleStory.exe")
                    || s.ends_with(r"Gamania/MapleStory/MapleStory.exe")
            }),
            "expected a Gamania\\MapleStory default candidate, got {candidates:?}"
        );
    }

    // Unit tests bypass Tauri's `State<_, AppState>` wrapper by
    // calling the inner impl fn (`detect_imp::detect_game_path_impl`)
    // directly with an `&AppState`, matching the pattern the
    // `commands/config.rs` / `commands/storage.rs` test suites
    // already use. The `#[tauri::command]` attribute is only a
    // specta + IPC shim — the underlying logic we care about
    // exercises identically through the inner helper.

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn detect_game_path_returns_config_value_without_touching_registry() {
        let (_dir, state) = temp_app_state();
        let config_path = config_xml_path(&state);
        let key = game_path_config_key("ExecPath", "610074_T9");
        svc_config::set_value(&config_path, &key, Some(r"D:\already\cached"))
            .await
            .expect("seed");

        let got = detect_imp::detect_game_path_impl(
            &state,
            "610074_T9".into(),
            "ExecPath".into(),
            // dir_reg is intentionally a junk subkey — if the
            // Config short-circuit is working, we never touch the
            // registry, so the bogus subkey doesn't matter.
            r"SOFTWARE\__UNLIKELY_SUBKEY__".into(),
            "MapleStory.exe %s %s".into(),
        )
        .await
        .expect("detect");

        assert_eq!(got.as_deref(), Some(r"D:\already\cached"));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn detect_game_path_returns_none_when_config_empty_and_dir_reg_empty() {
        let (_dir, state) = temp_app_state();
        let got = detect_imp::detect_game_path_impl(
            &state,
            "610074_T9".into(),
            "ExecPath".into(),
            String::new(),
            String::new(),
        )
        .await
        .expect("detect");
        assert!(got.is_none(), "expected None, got {got:?}");
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn detect_game_path_reads_registry_and_writes_back_to_config() {
        // `HKCU\Environment@TEMP` is present on every Windows
        // install — the same stable probe the registry service
        // layer uses in its happy-path unit test. We pretend the
        // INI tells us to look up `dir_reg = "Environment"` with
        // `dir_value_name = "TEMP"` so we read a guaranteed-present
        // value. Afterwards, the command must have seeded
        // Config.xml with the registry value.
        let (_dir, state) = temp_app_state();
        let config_path = config_xml_path(&state);
        let key = game_path_config_key("TEMP", "PROBE_GAME");

        assert_eq!(
            svc_config::get_value(&config_path, &key).await,
            "",
            "precondition: config must start empty"
        );

        let got = detect_imp::detect_game_path_impl(
            &state,
            "PROBE_GAME".into(),
            "TEMP".into(),
            "Environment".into(),
            "MapleStory.exe %s %s".into(),
        )
        .await
        .expect("detect");

        let registry_value = got.expect("HKCU\\Environment@TEMP should be present");
        assert!(!registry_value.is_empty());

        let cached = svc_config::get_value(&config_path, &key).await;
        assert_eq!(
            cached, registry_value,
            "detect_game_path must write the registry value back to Config"
        );
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn detect_game_path_reads_hklm_when_dir_reg_has_hklm_prefix() {
        // Drive the same happy path as above but pass `dir_reg`
        // with an explicit HKLM prefix. If the hive parsing regresses,
        // the lookup falls through instead of reading the stable
        // HKLM Windows version value.
        let (_dir, state) = temp_app_state();
        let got = detect_imp::detect_game_path_impl(
            &state,
            "PROBE_GAME_2".into(),
            "ProductName".into(),
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion".into(),
            "MapleStory.exe %s %s".into(),
        )
        .await
        .expect("detect");

        assert!(
            got.as_deref()
                .is_some_and(|v| v.to_ascii_lowercase().contains("windows")),
            "HKLM prefix must use HKLM lookup; got {got:?}"
        );
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn detect_game_path_missing_registry_subkey_returns_none() {
        // Config empty + dir_reg set to a GUID-like nonce that
        // can't exist → registry read returns Ok(None) → command
        // returns Ok(None), and the Config must *not* have been
        // mutated (nothing to write back).
        let (_dir, state) = temp_app_state();
        let config_path = config_xml_path(&state);
        let key = game_path_config_key("NoSuchValue", "NO_GAME");

        let got = detect_imp::detect_game_path_impl(
            &state,
            "NO_GAME".into(),
            "NoSuchValue".into(),
            r"SOFTWARE\__BEANFUN_NEXT_P10_NONCE_9F3C1A__".into(),
            String::new(),
        )
        .await
        .expect("detect");

        assert!(got.is_none());
        assert_eq!(svc_config::get_value(&config_path, &key).await, "");
    }

    // ---- Non-Windows fallback -------------------------------------------
    //
    // On non-Windows builds the `detect_imp` sub-module does not
    // exist; the IPC body goes straight to `platform_unsupported_error()`.
    // The tests below pin that behaviour via the helper directly
    // since we can't easily fabricate a `tauri::State<_, AppState>`.

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn platform_unsupported_error_carries_stable_code_and_message() {
        let err = platform_unsupported_error();
        assert_eq!(err.code, "launcher.platform_unsupported");
        assert!(err.message.contains("Windows"));
    }

    // ---- D5c: GameProcessInfo IPC shape ---------------------------------

    #[test]
    fn game_process_info_serializes_as_camel_case_with_optional_path() {
        // Contract lock: the frontend consumes `executablePath`
        // (camelCase); a refactor that removes `#[serde(rename_all
        // = "camelCase")]` would silently break every caller. Also
        // pins that `None` becomes JSON `null` (not omitted) so the
        // frontend can `?.` / destructure without a conditional.
        let info = GameProcessInfo {
            pid: 1234,
            name: "MapleStory.exe".to_string(),
            executable_path: Some(r"C:\MapleStory\MapleStory.exe".to_string()),
        };
        let v: serde_json::Value = serde_json::to_value(&info).expect("serialize");
        assert_eq!(v["pid"], 1234);
        assert_eq!(v["name"], "MapleStory.exe");
        assert_eq!(v["executablePath"], r"C:\MapleStory\MapleStory.exe");
    }

    #[test]
    fn game_process_info_serializes_none_executable_path_as_null() {
        let info = GameProcessInfo {
            pid: 1,
            name: "protected.exe".to_string(),
            executable_path: None,
        };
        let v: serde_json::Value = serde_json::to_value(&info).expect("serialize");
        assert!(
            v["executablePath"].is_null(),
            "expected JSON null for None, got {}",
            v["executablePath"]
        );
    }

    // ---- D5c: service → DTO conversion ----------------------------------

    #[cfg(target_os = "windows")]
    #[test]
    fn list_imp_into_dto_mirrors_service_layer_fields() {
        use crate::services::process::ProcessInfo;
        use std::path::PathBuf;

        let svc = ProcessInfo {
            pid: 7,
            name: "MapleStory.exe".into(),
            executable_path: Some(PathBuf::from(r"C:\MapleStory\MapleStory.exe")),
        };
        let dto = list_imp::into_dto(svc);
        assert_eq!(dto.pid, 7);
        assert_eq!(dto.name, "MapleStory.exe");
        assert_eq!(
            dto.executable_path.as_deref(),
            Some(r"C:\MapleStory\MapleStory.exe")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn list_imp_into_dto_preserves_none_executable_path() {
        // Protected / mid-exit processes report `NULL`
        // ExecutablePath from WMI; the DTO must keep that shape
        // rather than coercing to an empty string.
        use crate::services::process::ProcessInfo;

        let svc = ProcessInfo {
            pid: 4,
            name: "System".into(),
            executable_path: None,
        };
        let dto = list_imp::into_dto(svc);
        assert!(dto.executable_path.is_none());
    }

    // ---- D5c: non-Windows fallback for list/kill commands ---------------

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn list_game_processes_on_non_windows_returns_platform_unsupported() {
        let err = list_game_processes(r"/games/MapleStory.exe".into())
            .await
            .expect_err("non-Windows list_game_processes must error");
        assert_eq!(err.code, "launcher.platform_unsupported");
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn kill_game_processes_on_non_windows_returns_platform_unsupported() {
        let err = kill_game_processes(vec![1, 2, 3])
            .await
            .expect_err("non-Windows kill_game_processes must error");
        assert_eq!(err.code, "launcher.platform_unsupported");
    }

    // ---- D5d: AutoPasteRequest IPC shape --------------------------------

    #[test]
    fn auto_paste_request_deserializes_from_camel_case() {
        // Contract lock: the frontend sends `{ className, account,
        // password, specialClick }` (camelCase). A refactor that
        // removed `#[serde(rename_all = "camelCase")]` would
        // silently produce a 400 Bad Request on every call.
        let wire = r#"{
            "className": "MapleStoryClass",
            "account": "user1",
            "password": "pw42",
            "specialClick": true
        }"#;
        let req: AutoPasteRequest = serde_json::from_str(wire).expect("deserialize");
        assert_eq!(req.class_name, "MapleStoryClass");
        assert_eq!(req.account, "user1");
        assert_eq!(req.password, "pw42");
        assert!(req.special_click);
    }

    #[test]
    fn auto_paste_request_requires_special_click_field() {
        // `specialClick` must be explicit — omitting it would let a
        // frontend bug silently default to `false` and skip the
        // SEA pre-click sequence for MapleStory TW users. This
        // assertion catches an accidental `#[serde(default)]`
        // addition to the struct.
        let wire = r#"{
            "className": "MapleStoryClass",
            "account": "a",
            "password": "b"
        }"#;
        let err =
            serde_json::from_str::<AutoPasteRequest>(wire).expect_err("specialClick required");
        assert!(
            err.to_string().contains("specialClick"),
            "expected missing-field error to mention specialClick, got {err}"
        );
    }

    // ---- D5d: non-Windows fallback for auto_paste -----------------------

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn auto_paste_on_non_windows_returns_platform_unsupported() {
        let err = auto_paste(AutoPasteRequest {
            class_name: "MapleStoryClass".into(),
            account: "a".into(),
            password: "b".into(),
            special_click: false,
        })
        .await
        .expect_err("non-Windows auto_paste must error");
        assert_eq!(err.code, "launcher.platform_unsupported");
    }

    // ---- D5d: error surface surfaces process.window_not_found -----------

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn auto_paste_returns_window_not_found_when_no_launcher_open() {
        // The test runner never has a `__unlikely_class_%%%` window
        // open, so the service layer surfaces
        // `ProcessError::WindowNotFound`, which maps to
        // `process.window_not_found` via the existing From impl.
        // Pins both sides of the contract: the command layer
        // routes the error unchanged, and the mapping hasn't
        // regressed.
        let err = auto_paste(AutoPasteRequest {
            class_name: "__beanfun_no_such_class__".into(),
            account: "a".into(),
            password: "b".into(),
            special_click: false,
        })
        .await
        .expect_err("missing launcher window must error");
        assert_eq!(err.code, "process.window_not_found");
        let details = err.details.expect("details present");
        assert_eq!(
            details.get("primary_class"),
            Some(&serde_json::json!("__beanfun_no_such_class__"))
        );
    }

    // ---- Code-name drift pins -------------------------------------------

    #[test]
    fn command_layer_codes_are_stable_strings() {
        // Lock the exact spelling of the command-only codes
        // so a refactor that touches the module docs or the
        // `CommandError::new(...)` call-site doesn't silently
        // change the code the frontend branches on.
        assert_eq!(SPAWN_BLOCKING_FAILED_CODE, "launcher.spawn_blocking_failed");
        assert_eq!(PLATFORM_UNSUPPORTED_CODE, "launcher.platform_unsupported");
    }

    #[test]
    fn platform_unsupported_code_is_stable() {
        // Explicit cross-module contract with the frontend — the
        // Vue settings page branches on this string to show the
        // "Windows-only feature" affordance. Named on its own so
        // grep for `launcher.platform_unsupported` finds this
        // assertion as the canonical source of truth.
        assert_eq!(PLATFORM_UNSUPPORTED_CODE, "launcher.platform_unsupported");
    }
}
