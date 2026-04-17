//! Game launching service.
//!
//! Covers the WPF `MainWindow::btn_Run_Game_Click` pipeline
//! (`Beanfun/MainWindow.xaml.cs` L1727-1900) and the
//! `MainWindow::startByLR` helper (L1902-1947) plus the
//! `App::ReleaseResource` resource unpacker
//! (`Beanfun/App.xaml.cs` L131-167), split into two modules:
//!
//! | Module                          | Scope                                                                          |
//! | ------------------------------- | ------------------------------------------------------------------------------ |
//! | [`error`]                       | `GameError` — every typed failure `services/game` can surface                  |
//! | [`launcher`]                    | path / mode validation + `Normal` spawn + `Auto` resolution via system locale  |
//! | `locale_remulator` (P8 chunk 8.2) | LR resource release + SHA-256 integrity check + `ShellExecuteW` runas launch |
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

pub mod error;
pub mod launcher;

pub use error::GameError;
pub use launcher::{
    launch_normal, locale_to_resolved_mode, resolve_mode, substitute_credentials, validate_path,
    GameStartMode, ResolvedMode,
};
