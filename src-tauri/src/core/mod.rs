//! Framework-agnostic domain logic shared across services and Tauri commands.
//!
//! Everything under `core::` must be:
//! - Pure Rust (no Tauri runtime / no tokio runtime coupling)
//! - Deterministic and easy to unit-test in isolation
//! - Byte-compatible with the legacy C# WPF behaviour where applicable
//!
//! HTTP / IO / async orchestration belongs under `services::` (added in P3+).

pub mod legacy;
pub mod parser;
pub mod time;
pub mod version;
pub mod wcdes;
