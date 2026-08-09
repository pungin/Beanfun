//! IO / HTTP / async orchestration layer.
//!
//! Anything in here is allowed to:
//! - talk to the network (reqwest)
//! - be `async` and assume a tokio runtime
//! - depend on [`crate::core`] for pure parsing / crypto primitives
//!
//! The inverse is **not** true — `crate::core` must stay IO-free so it can be
//! unit-tested in isolation and reused from other runtimes (e.g. a future CLI
//! companion) without dragging reqwest/tokio in.
//!
//! Each service (beanfun, maplestory launcher, …) lives in its own submodule.

pub mod beanfun;
pub mod config;
pub mod game;
pub mod maple_cache;
pub mod storage;
pub mod system;
pub mod updater;
pub mod webview_proxy;

#[cfg(target_os = "windows")]
pub mod process;

#[cfg(target_os = "windows")]
pub mod registry;
