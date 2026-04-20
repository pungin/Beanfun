//! MapleStory cache cleanup — sweep per-launch artefacts and stale
//! files from the game's install directory.
//!
//! Ports WPF `Beanfun/Windows/MapleTools.xaml.cs` `btn_Recycling_Click`
//! (L52-112) — the only "Tools" action that needs a backend touchpoint
//! because the frontend can't recursively delete arbitrary directories
//! through the Tauri sandbox.
//!
//! # Scope
//!
//! - Pure filesystem cleanup of the directory containing the game
//!   `.exe`.
//! - **Not** a launcher / process / config helper — those already
//!   live in [`super::game`] / [`super::process`] / [`super::config`].
//! - **Not** game-discovery — `Path.<gameCode>` lookup happens in
//!   [`super::config`] before the value is handed to this service.
//!
//! # Modules
//!
//! | Module          | Responsibility                                                  |
//! | --------------- | --------------------------------------------------------------- |
//! | [`error`]       | `MapleCacheError` — typed pre-flight failures                   |
//! | [`mod@clean`]   | `clean_maple_game_cache` + `CleanCacheReport` — the cleanup loop |

pub mod clean;
pub mod error;

pub use clean::{clean_maple_game_cache, CleanCacheReport};
pub use error::MapleCacheError;
