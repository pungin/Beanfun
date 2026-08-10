//! System-level OS integration helpers.
//!
//! Framework-agnostic wrappers around OS APIs that the P10.3+ Tauri
//! commands surface to the frontend. Currently only `open_url`; the
//! module exists as its own sub-tree so future additions
//! (`open_folder`, `show_in_finder`, `reveal_in_explorer`, …) have a
//! home that is **not** tied to the Tauri runtime.
//!
//! # Why not use `tauri-plugin-opener` directly?
//!
//! The plugin's Rust API requires an `AppHandle<R>`, which would
//! drag the Tauri runtime into the service layer and break the
//! "services are framework-agnostic" invariant set in P10.1
//! (`services/mod.rs` module doc). The [`open`] crate (already a
//! transitive dependency via the plugin) gives us the same
//! `ShellExecuteW` / `LSOpenCFURLRef` / `xdg-open` behaviour without
//! the `AppHandle` coupling. The plugin itself stays wired in for
//! frontend JS callers (`import { open } from '@tauri-apps/plugin-opener'`).
//!
//! # Modules
//!
//! | Module         | Responsibility                                                    |
//! | -------------- | ----------------------------------------------------------------- |
//! | [`error`]      | `SystemError` — typed failures across the system service          |
//! | [`mod@open_url`] | `open_url` — scheme-allowlisted wrapper over [`open::that`]; `open_directory` — reveal a local folder |

pub mod error;
pub mod open_url;

pub use error::SystemError;
pub use open_url::{open_directory, open_url};
