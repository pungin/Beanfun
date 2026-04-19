//! Typed failure surface for the MapleStory cache-cleanup service.
//!
//! Each variant maps to a distinct `maple_cache.*` code on the
//! [`crate::commands::error::CommandError`] boundary so the frontend
//! can branch on cause without parsing free-form messages.
//!
//! # Variants
//!
//! | Variant                              | Cause                                                                        |
//! | ------------------------------------ | ---------------------------------------------------------------------------- |
//! | [`MapleCacheError::PathEmpty`]       | Caller passed an empty `game_path`.                                          |
//! | [`MapleCacheError::PathNoParent`]    | `Path::parent` of `game_path` returned `None` (e.g. a bare filename).        |
//! | [`MapleCacheError::PathNotFound`]    | Resolved game directory does not exist on disk.                              |
//! | [`MapleCacheError::PathNotADir`]     | Resolved path exists but is not a directory.                                 |
//! | [`MapleCacheError::ReadDirFailed`]   | Iterating the directory's children failed (permission denied, race, …).      |
//! | [`MapleCacheError::SpawnBlockingFailed`] | `tokio::task::spawn_blocking` panicked or was cancelled while cleaning.  |

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Typed error for the MapleStory cache-cleanup service. Every
/// per-item delete failure is captured into the
/// [`super::CleanCacheReport::errors`] vector instead of bubbling
/// here — only "couldn't even start the cleanup" failures surface
/// as `MapleCacheError` so the command layer can distinguish a
/// genuine pre-flight failure from a partial-success report.
#[derive(Debug, Error)]
pub enum MapleCacheError {
    /// Caller passed an empty `game_path`. Mirrors the WPF guard
    /// implicit in `Path.GetDirectoryName(t_GamePath.Text)` —
    /// passing `null` / `""` would throw `ArgumentNullException`
    /// before any cleanup happened.
    #[error("game path must not be empty")]
    PathEmpty,

    /// `Path::parent` of `game_path` returned `None`. WPF mirrors
    /// this via `Path.GetDirectoryName` returning `null` for
    /// rooted bare filenames; we surface it as a typed variant so
    /// the frontend can show a localized "invalid path" toast
    /// rather than a stack trace.
    #[error("game path has no parent directory: {path}")]
    PathNoParent {
        /// The original `game_path` that lacked a parent.
        path: String,
    },

    /// The resolved game directory (parent of `game_path`) does
    /// not exist. Distinct from
    /// [`MapleCacheError::PathNotADir`] so the frontend can show
    /// "path missing" vs "path is a file" separately.
    #[error("game directory does not exist: {path}")]
    PathNotFound {
        /// Resolved directory path that was not found.
        path: PathBuf,
    },

    /// The resolved game directory exists but is a regular file
    /// (or other non-directory entry). Catches the user pasting a
    /// file path where a directory is expected, before any
    /// destructive op runs.
    #[error("game directory is not a directory: {path}")]
    PathNotADir {
        /// Resolved path that exists but is not a directory.
        path: PathBuf,
    },

    /// Listing the children of the game directory failed before
    /// the per-item delete loops could run. Typical causes:
    /// permission denied, the directory was removed mid-call.
    /// Per-item delete failures are reported via
    /// [`super::CleanCacheReport::errors`] instead — this variant
    /// is only used when the iteration itself can't start.
    #[error("failed to read game directory {path}: {source}")]
    ReadDirFailed {
        /// Directory whose iteration failed.
        path: PathBuf,
        /// Underlying I/O error from [`std::fs::read_dir`].
        #[source]
        source: io::Error,
    },

    /// The [`tokio::task::spawn_blocking`] wrapper that hosts the
    /// synchronous filesystem walk panicked or was cancelled.
    /// Should not happen in steady state but we surface it as a
    /// distinct variant so the command layer can emit
    /// `maple_cache.spawn_blocking_failed` rather than conflating
    /// it with a real cleanup failure.
    #[error("blocking task panicked or was cancelled: {0}")]
    SpawnBlockingFailed(#[source] tokio::task::JoinError),
}
