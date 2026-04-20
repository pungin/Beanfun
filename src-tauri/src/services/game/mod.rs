//! Game launching service.
//!
//! Covers the WPF `MainWindow::btn_Run_Game_Click` pipeline
//! (`Beanfun/MainWindow.xaml.cs` L1727-1900) and the
//! `MainWindow::startByLR` helper (L1902-1947) plus the
//! `App::ReleaseResource` resource unpacker
//! (`Beanfun/App.xaml.cs` L131-167), split into two modules:
//!
//! | Module                | Scope                                                                               |
//! | --------------------- | ----------------------------------------------------------------------------------- |
//! | [`error`]             | `GameError` — every typed failure `services/game` can surface                       |
//! | [`launcher`]          | path / mode validation, `Normal` spawn, `Auto` resolution, top-level `launch_game`  |
//! | [`locale_remulator`]  | LR resource release + SHA-256 integrity check + `ShellExecuteW` runas launch        |
//!
//! # Top-level call graph
//!
//! ```text
//!                       launch_game(req)
//!                             │
//!                             ▼
//!                     validate_path(game_path)
//!                             │
//!                             ▼
//!                     resolve_mode(mode)
//!                      /              \
//!            Normal   ▼                ▼  LocaleRemulator
//!            launch_normal    locale_remulator::release_all
//!            (Command.spawn)          │
//!                                     ▼  (Windows only)
//!                           locale_remulator::launch_via_lr
//!                                   (ShellExecuteW + runas)
//! ```
//!
//! `LaunchRequest` is built by the Tauri command layer (P10) from the
//! user's settings + active account; [`default_target_dir`] resolves
//! to `current_exe().parent()` for LR staging (mirrors WPF
//! `App.AppDir`).
//!
//! The process-find / kill-existing flow that WPF interleaves with
//! launching (L1765-1832, WMI-backed) belongs to
//! `services/process` (P9) — this module only accepts an already-resolved
//! [`std::path::Path`] and trusts the caller did the preflight.
//!
//! # Service-layer contract
//!
//! - Returns typed [`error::GameError`] values; does **not** show
//!   dialogs, call `MessageBox`, or depend on any UI layer. P10 Tauri
//!   commands will map errors to user-facing messages
//!   (`MsgGamePathHaveWChar`, `MsgLocalePluginReleaseError`, …).
//! - Does **not** read the registry for game path (that's P9
//!   `services/registry`). Callers pass an absolute [`Path`][std::path::Path].
//! - Does **not** manage process lifecycles beyond `spawn` — fire-and-forget
//!   for Normal mode, `ShellExecuteW` for LR (which spawns the elevated
//!   `LRProc.exe` that then spawns the game).
//!
//! # SHA-256 security upgrade over WPF
//!
//! WPF's `App.ReleaseResource` (L131-167) uses `FileInfo.Length ==
//! stream.Length` to decide whether to skip rewriting a LocaleRemulator
//! binary. Beanfun upgrades that to a byte-exact SHA-256 computed
//! at build time by [`build.rs`](../../../build.rs.html) and consumed
//! by [`locale_remulator::release_file`], closing the "same-length
//! tampered DLL" bypass. Runtime cost is a single ~240 KB hash on
//! cold launch; benefit is integrity enforcement before any elevated
//! `runas` process sees the files.

pub mod error;
pub mod launcher;
pub mod locale_remulator;

pub use error::GameError;
pub use launcher::{
    default_target_dir, launch_game, launch_normal, locale_to_resolved_mode, resolve_mode,
    substitute_credentials, validate_path, GameStartMode, LaunchRequest, ResolvedMode,
};
pub use locale_remulator::{
    build_lr_arguments, release_all, release_file, verify_file, ReleaseOutcome, LR_ASSETS, LR_GUID,
};
