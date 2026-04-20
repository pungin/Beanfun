//! MapleStory cache-cleanup Tauri commands — the thin async
//! boundary between the frontend's "Recycling" button (in
//! `windows/MapleTools.vue`) and the
//! [`crate::services::maple_cache`] sweep service.
//!
//! Ports the WPF `Beanfun/Windows/MapleTools.xaml.cs`
//! `btn_Recycling_Click` (L52-112) entry point — the only Tools
//! action that needs a backend touchpoint because the Tauri
//! sandbox forbids the frontend from recursively deleting
//! arbitrary directories on its own.
//!
//! # Design notes
//!
//! - The command is intentionally **thin**: it forwards the
//!   `game_path` parameter (the `.exe` path the user picked in
//!   Settings) verbatim and lets the service layer derive the
//!   parent directory + run the sweep. All path validation,
//!   filesystem walking, and per-item failure aggregation lives
//!   under `services/maple_cache/`.
//! - Returns the full [`CleanCacheReport`] so the frontend can
//!   render either the WPF-equivalent "MsgRecyclingDone" toast
//!   (when `errors` is empty) or a richer "completed with errors"
//!   notification — the report shape is stable across future
//!   `.dmp` / stray-DLL additions because the wire format only
//!   carries names + I/O kinds.
//!
//! # Command-layer error codes
//!
//! All failures flow through the existing
//! [`From<MapleCacheError> for CommandError`][mfrom] in
//! [`crate::commands::error`] — see that module's
//! [`maple_cache.*` table][mtable] for the full mapping.
//!
//! [mfrom]: super::error
//! [mtable]: super::error#maple_cacheerror--maple_cache

use crate::commands::error::CommandError;
use crate::services::maple_cache::{self, CleanCacheReport};

/// Sweep MapleStory's per-launch cache from the directory holding
/// `game_path`. Mirrors WPF `btn_Recycling_Click` exactly — wipes
/// `blob_storage` / `GPUCache` / `VideoDecodeStats` / `XignCode`,
/// recursively removes any stale `*.$$$` update directories, and
/// deletes any `*.dmp` files plus the two Locale-Emulator helper
/// DLLs (`localeemulator.dll` / `loaderdll.dll`) the launcher
/// drops next to the executable.
///
/// `game_path` is the full path to the game's `.exe` (the same
/// value `Settings.gamePath` shows the user); the parent
/// directory is derived inside the service so the frontend
/// doesn't need to do any path manipulation.
///
/// Returns a [`CleanCacheReport`] describing exactly which items
/// were removed and which item-level failures (if any) occurred.
/// Per-item failures do **not** abort the sweep — they're
/// captured into [`CleanCacheReport::errors`] so the toast can
/// surface "X removed, Y failed" instead of giving the user a
/// false success.
///
/// # Errors
///
/// Pre-flight failures bubble up as `CommandError` and skip the
/// sweep entirely:
///
/// - `maple_cache.path_empty` — `game_path` was empty.
/// - `maple_cache.path_no_parent` — `game_path` had no parent.
/// - `maple_cache.path_not_found` — resolved game directory does
///   not exist.
/// - `maple_cache.path_not_a_dir` — resolved path exists but is a
///   regular file.
/// - `maple_cache.read_dir_failed` — listing the directory's
///   children failed before any cleanup could run.
/// - `maple_cache.spawn_blocking_failed` — the blocking task
///   panicked or was cancelled (should not happen in steady
///   state).
#[tauri::command]
#[specta::specta]
pub async fn clean_maple_game_cache(game_path: String) -> Result<CleanCacheReport, CommandError> {
    let report = maple_cache::clean_maple_game_cache(&game_path).await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Error-path symbol tests — happy path is covered by the
    // service-layer integration suite under
    // `services::maple_cache::clean::tests`. Here we only assert
    // the IPC contract surfaces the `maple_cache.*` codes
    // unchanged through the `From<MapleCacheError> for
    // CommandError` conversion.
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn rejects_empty_game_path_as_maple_cache_path_empty() {
        let err = clean_maple_game_cache(String::new())
            .await
            .expect_err("empty game_path must surface PathEmpty");
        assert_eq!(err.code, "maple_cache.path_empty");
    }

    #[tokio::test]
    async fn rejects_bare_filename_as_maple_cache_path_no_parent() {
        let err = clean_maple_game_cache("MapleStory.exe".to_string())
            .await
            .expect_err("bare filename must surface PathNoParent");
        assert_eq!(err.code, "maple_cache.path_no_parent");
    }

    #[tokio::test]
    async fn rejects_missing_directory_as_maple_cache_path_not_found() {
        // `tempfile::TempDir` gives us a valid parent that we
        // immediately drop, then we point the command at a child
        // path under the (now-gone) directory — the resolved
        // game directory is missing, the service must surface
        // `path_not_found`.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let exe = tmp.path().join("nope").join("MapleStory.exe");
        let err = clean_maple_game_cache(exe.to_string_lossy().into_owned())
            .await
            .expect_err("missing dir must surface PathNotFound");
        assert_eq!(err.code, "maple_cache.path_not_found");
    }
}
