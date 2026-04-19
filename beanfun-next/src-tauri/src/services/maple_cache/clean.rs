//! Clean MapleStory's per-launch cache directories and stale files
//! from the game's install folder.
//!
//! Direct port of WPF `Beanfun/Windows/MapleTools.xaml.cs`
//! `btn_Recycling_Click` (L52-112). The WPF flow:
//!
//! 1. Resolve `gameDir = Path.GetDirectoryName(t_GamePath.Text)`
//!    — i.e. the directory the game's `.exe` lives in.
//! 2. Recursively delete four well-known subdirectories the game
//!    creates at runtime:
//!    `blob_storage`, `GPUCache`, `VideoDecodeStats`, `XignCode`.
//!    Per-item failures are silently ignored (`try { ... } catch { }`).
//! 3. Iterate every immediate subdirectory and recursively delete
//!    those whose name ends with `.$$$` — the game's marker for
//!    a partial / aborted update. Per-item failures silently
//!    ignored.
//! 4. Iterate every immediate file and delete those that either
//!    end with `.dmp` (case-insensitive) or are exactly named
//!    `localeemulator.dll` / `loaderdll.dll` (case-insensitive).
//!    Per-item failures silently ignored.
//! 5. Show a "recycling done" success toast regardless of how many
//!    items actually got removed.
//!
//! # Differences from WPF
//!
//! - **Path validation surfaces typed errors.** WPF would happily
//!   throw `NullReferenceException` / `ArgumentException` /
//!   `DirectoryNotFoundException` out to the unhandled-dispatcher
//!   handler if `t_GamePath.Text` was empty / malformed / pointed
//!   at a missing directory. Here those become
//!   [`MapleCacheError::PathEmpty`] / [`MapleCacheError::PathNoParent`]
//!   / [`MapleCacheError::PathNotFound`] / [`MapleCacheError::PathNotADir`]
//!   so the frontend can show a localized error rather than a
//!   crash dialog.
//! - **Per-item failures are reported, not silenced.** WPF's
//!   `catch { }` discards everything; we collect each failure
//!   into [`CleanCacheReport::errors`] so the frontend toast can
//!   surface "X items removed, Y failed" instead of giving the
//!   user a green checkmark while a locked DLL is still on disk.
//!   The cleanup itself does **not** abort on first failure —
//!   each loop iteration is independent, matching the WPF
//!   "best-effort cleanup" semantic.
//!
//! # Blocking isolation
//!
//! The whole filesystem walk runs inside [`tokio::task::spawn_blocking`]
//! (P10-Q5 = A) — `std::fs::read_dir` / `std::fs::remove_dir_all` /
//! `std::fs::remove_file` are synchronous syscalls that would stall
//! the async reactor on a slow disk. Granularity is the entire
//! walk so the async boundary has exactly one await point.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;

use crate::services::maple_cache::error::MapleCacheError;

/// Subdirectories the game creates at runtime that are safe to wipe
/// between launches. Names are case-sensitive on Linux / macOS dev
/// boxes but the production target (Windows) treats the filesystem
/// as case-insensitive — matches WPF behaviour, which uses the
/// strings verbatim with the OS comparator.
const FIXED_SUBDIRS: &[&str] = &["blob_storage", "GPUCache", "VideoDecodeStats", "XignCode"];

/// Suffix the game appends to a directory while applying an
/// update. If the update is aborted the suffix sticks around and
/// the next launch trips on it; this cleanup removes the carcass.
const STALE_DIR_SUFFIX: &str = ".$$$";

/// Suffix for crash-dump files the game (and Locale Emulator)
/// drops next to the executable. Compared case-insensitively.
const DMP_FILE_SUFFIX: &str = ".dmp";

/// Locale-Emulator helper DLLs that are released next to the game
/// `.exe` for non-Windows-Korean launches; we wipe them so a
/// later Normal-mode launch doesn't pick them up by accident.
/// Compared case-insensitively against the file's exact name.
const STRAY_DLLS: &[&str] = &["localeemulator.dll", "loaderdll.dll"];

/// Outcome of a single [`clean_maple_game_cache`] run.
///
/// Carries enough information for the frontend to render a
/// summary toast like "removed N directories and M files (K
/// errors)". Names are stored relative to the game directory so
/// the toast doesn't leak the user's full install path.
///
/// # Wire shape
///
/// Three flat arrays; the frontend pattern-matches on
/// `report.errors.length === 0` to decide between the
/// `MsgRecyclingDone` toast (matching WPF) and a richer
/// "completed with errors" notification.
#[derive(Debug, Clone, Default, Serialize, Type)]
pub struct CleanCacheReport {
    /// Names of directories that were successfully removed,
    /// relative to the game directory.
    pub deleted_dirs: Vec<String>,

    /// Names of files that were successfully removed, relative to
    /// the game directory.
    pub deleted_files: Vec<String>,

    /// Best-effort per-item failure descriptions in the form
    /// `"<name>: <io_kind>"` (e.g. `"XignCode: PermissionDenied"`).
    /// Mirrors WPF's "don't abort on partial failure" semantic
    /// while still letting the frontend surface a non-empty list
    /// to the user.
    pub errors: Vec<String>,
}

/// Resolve the game directory (parent of the `.exe` path) without
/// touching the filesystem yet. Pure / sync so the validation
/// path can be unit-tested without a real game install.
fn resolve_game_dir(game_path: &str) -> Result<PathBuf, MapleCacheError> {
    if game_path.is_empty() {
        return Err(MapleCacheError::PathEmpty);
    }
    let exe = PathBuf::from(game_path);
    let Some(parent) = exe.parent() else {
        return Err(MapleCacheError::PathNoParent {
            path: game_path.to_string(),
        });
    };
    // `Path::parent` of `"foo.exe"` returns `Some("")` rather than
    // `None` — that empty path is functionally a "no parent" for
    // our purposes (relative to nothing useful). Surface it as
    // PathNoParent so the frontend localizes it the same way.
    if parent.as_os_str().is_empty() {
        return Err(MapleCacheError::PathNoParent {
            path: game_path.to_string(),
        });
    }
    Ok(parent.to_path_buf())
}

/// Synchronous core that runs inside [`tokio::task::spawn_blocking`].
/// Keeps the cleanup loops in one place so future maintenance
/// doesn't have to track three identical `try-catch` blocks.
fn clean_dir_sync(game_dir: &Path) -> Result<CleanCacheReport, MapleCacheError> {
    let metadata = match fs::metadata(game_dir) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(MapleCacheError::PathNotFound {
                path: game_dir.to_path_buf(),
            });
        }
        Err(source) => {
            return Err(MapleCacheError::ReadDirFailed {
                path: game_dir.to_path_buf(),
                source,
            });
        }
    };
    if !metadata.is_dir() {
        return Err(MapleCacheError::PathNotADir {
            path: game_dir.to_path_buf(),
        });
    }

    let mut report = CleanCacheReport::default();

    // Stage 1 — fixed subdirs. WPF L66-83.
    for name in FIXED_SUBDIRS {
        let target = game_dir.join(name);
        if !target.exists() {
            continue;
        }
        match fs::remove_dir_all(&target) {
            Ok(()) => report.deleted_dirs.push((*name).to_string()),
            Err(err) => report.errors.push(format_item_error(name, &err)),
        }
    }

    // Stage 2 + 3 — single iteration over the directory entries
    // covers both the `.$$$` stale subdir sweep (WPF L86-94) and
    // the stale-file sweep (WPF L97-109). Walking once instead of
    // twice avoids re-listing a (possibly large) directory and
    // keeps the WPF semantic identical (each match is independent
    // and best-effort).
    let entries = fs::read_dir(game_dir).map_err(|source| MapleCacheError::ReadDirFailed {
        path: game_dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        // A failing `entry` mid-iteration is reported as an item
        // error rather than aborting the sweep — same "best
        // effort" stance as WPF's per-item `catch { }`.
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                report.errors.push(format!("<entry>: {:?}", err.kind()));
                continue;
            }
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            // Non-UTF-8 entry name — skip silently. WPF's
            // `DirectoryInfo.Name` is a `string` so it'd never
            // see this on Windows, but the cross-platform Rust
            // surface needs to handle it gracefully.
            continue;
        };
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                report.errors.push(format_item_error(name, &err));
                continue;
            }
        };

        if file_type.is_dir() {
            if name.ends_with(STALE_DIR_SUFFIX) {
                match fs::remove_dir_all(entry.path()) {
                    Ok(()) => report.deleted_dirs.push(name.to_string()),
                    Err(err) => report.errors.push(format_item_error(name, &err)),
                }
            }
        } else if file_type.is_file() && is_stale_file(name) {
            match fs::remove_file(entry.path()) {
                Ok(()) => report.deleted_files.push(name.to_string()),
                Err(err) => report.errors.push(format_item_error(name, &err)),
            }
        }
    }

    Ok(report)
}

/// Predicate matching WPF's L101-105 condition (case-insensitive
/// `.dmp` suffix or exact stray-DLL name).
fn is_stale_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(DMP_FILE_SUFFIX) || STRAY_DLLS.iter().any(|stray| lower == *stray)
}

/// Format a per-item failure as `"<name>: <io_kind>"`. Kept
/// private so the encoding stays in one place if the frontend
/// later asks for a richer payload (e.g. `{ name, kind }` JSON).
fn format_item_error(name: &str, err: &std::io::Error) -> String {
    format!("{name}: {:?}", err.kind())
}

/// Sweep MapleStory's per-launch cache from the game's install
/// directory.
///
/// # Errors
///
/// - [`MapleCacheError::PathEmpty`] — `game_path` was an empty
///   string.
/// - [`MapleCacheError::PathNoParent`] — `game_path` had no
///   parent directory.
/// - [`MapleCacheError::PathNotFound`] — resolved game directory
///   does not exist on disk.
/// - [`MapleCacheError::PathNotADir`] — resolved path exists but
///   is not a directory.
/// - [`MapleCacheError::ReadDirFailed`] — listing the directory's
///   children failed before any cleanup could run.
/// - [`MapleCacheError::SpawnBlockingFailed`] — the blocking task
///   was cancelled or panicked.
///
/// Per-item delete failures are reported in
/// [`CleanCacheReport::errors`] instead of aborting the sweep —
/// see module doc.
pub async fn clean_maple_game_cache(game_path: &str) -> Result<CleanCacheReport, MapleCacheError> {
    let game_dir = resolve_game_dir(game_path)?;
    tokio::task::spawn_blocking(move || clean_dir_sync(&game_dir))
        .await
        .map_err(MapleCacheError::SpawnBlockingFailed)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    // -----------------------------------------------------------------
    // resolve_game_dir — pure validation, no filesystem.
    // -----------------------------------------------------------------

    #[test]
    fn resolve_game_dir_rejects_empty_string() {
        assert!(matches!(
            resolve_game_dir(""),
            Err(MapleCacheError::PathEmpty)
        ));
    }

    #[test]
    fn resolve_game_dir_rejects_bare_filename() {
        // `Path::parent("MapleStory.exe")` returns `Some("")` —
        // resolved as `PathNoParent` per the explicit guard.
        let err = resolve_game_dir("MapleStory.exe").expect_err("bare filename must fail");
        match err {
            MapleCacheError::PathNoParent { path } => {
                assert_eq!(path, "MapleStory.exe");
            }
            other => panic!("expected PathNoParent, got {other:?}"),
        }
    }

    #[test]
    fn resolve_game_dir_returns_parent_for_windows_path() {
        let resolved = resolve_game_dir(r"C:\Games\MapleStory\MapleStory.exe")
            .expect("rooted path with parent must resolve");
        // PathBuf preserves the original separator on the host
        // platform; we don't assert the exact string to keep the
        // test cross-platform — only that the parent component
        // landed correctly.
        let last = resolved
            .file_name()
            .and_then(|s| s.to_str())
            .expect("parent should have a file name");
        // On Linux the entire backslash blob is one segment so
        // `file_name` returns the whole string; on Windows it
        // returns "MapleStory". Accept both shapes.
        assert!(
            last == "MapleStory" || last.contains("MapleStory"),
            "parent file name should mention MapleStory, got: {last}"
        );
    }

    // -----------------------------------------------------------------
    // clean_dir_sync — exercise the filesystem walk against a
    // tempdir with hand-crafted layouts.
    // -----------------------------------------------------------------

    fn make_dir(parent: &Path, name: &str) {
        fs::create_dir_all(parent.join(name)).expect("create_dir_all");
    }

    fn make_file(parent: &Path, name: &str) {
        let path = parent.join(name);
        let mut f = File::create(&path).expect("create file");
        // Some delete tests run on Linux CI; ensure the file isn't
        // empty so any "oops we mistakenly created a directory"
        // bug surfaces as a metadata mismatch rather than a no-op.
        f.write_all(b"placeholder")
            .expect("write placeholder bytes");
    }

    #[test]
    fn clean_dir_sync_returns_empty_report_for_empty_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let report = clean_dir_sync(tmp.path()).expect("empty dir should succeed");
        assert!(report.deleted_dirs.is_empty());
        assert!(report.deleted_files.is_empty());
        assert!(report.errors.is_empty());
    }

    #[test]
    fn clean_dir_sync_errors_on_missing_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let missing = tmp.path().join("does_not_exist");
        let err = clean_dir_sync(&missing).expect_err("missing dir must fail");
        assert!(matches!(err, MapleCacheError::PathNotFound { .. }));
    }

    #[test]
    fn clean_dir_sync_errors_when_path_is_a_file() {
        let tmp = TempDir::new().expect("tempdir");
        make_file(tmp.path(), "MapleStory.exe");
        let err =
            clean_dir_sync(&tmp.path().join("MapleStory.exe")).expect_err("file path must fail");
        assert!(matches!(err, MapleCacheError::PathNotADir { .. }));
    }

    #[test]
    fn clean_dir_sync_removes_all_four_fixed_subdirs() {
        let tmp = TempDir::new().expect("tempdir");
        for name in FIXED_SUBDIRS {
            make_dir(tmp.path(), name);
            // Drop a placeholder file inside so the recursive
            // delete actually has to recurse — guards against a
            // subtle regression where we'd accidentally use
            // `remove_dir` (non-recursive) instead of
            // `remove_dir_all`.
            make_file(&tmp.path().join(name), "placeholder.bin");
        }
        let report = clean_dir_sync(tmp.path()).expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut deleted = report.deleted_dirs.clone();
        deleted.sort();
        let mut expected: Vec<String> = FIXED_SUBDIRS.iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(deleted, expected);
        for name in FIXED_SUBDIRS {
            assert!(
                !tmp.path().join(name).exists(),
                "subdir {name} should have been removed"
            );
        }
    }

    #[test]
    fn clean_dir_sync_skips_fixed_subdirs_that_do_not_exist() {
        let tmp = TempDir::new().expect("tempdir");
        // Only create 2 of the 4 fixed subdirs; the missing two
        // must be silently skipped (no `errors` entry, matching
        // WPF's `Directory.Exists` early-continue on L76).
        make_dir(tmp.path(), "GPUCache");
        make_dir(tmp.path(), "XignCode");
        let report = clean_dir_sync(tmp.path()).expect("partial cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut deleted = report.deleted_dirs.clone();
        deleted.sort();
        assert_eq!(
            deleted,
            vec!["GPUCache".to_string(), "XignCode".to_string()]
        );
    }

    #[test]
    fn clean_dir_sync_removes_dollar_suffix_dirs() {
        let tmp = TempDir::new().expect("tempdir");
        make_dir(tmp.path(), "patch.$$$");
        make_dir(tmp.path(), "Data.$$$");
        // Sibling directory without the suffix must be left alone.
        make_dir(tmp.path(), "untouched");
        let report = clean_dir_sync(tmp.path()).expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut deleted = report.deleted_dirs.clone();
        deleted.sort();
        assert_eq!(
            deleted,
            vec!["Data.$$$".to_string(), "patch.$$$".to_string()]
        );
        assert!(
            tmp.path().join("untouched").exists(),
            "untouched subdir must remain"
        );
    }

    #[test]
    fn clean_dir_sync_removes_dmp_files_case_insensitively() {
        let tmp = TempDir::new().expect("tempdir");
        make_file(tmp.path(), "crash01.dmp");
        make_file(tmp.path(), "MIXED.DMP");
        make_file(tmp.path(), "Lower.Dmp");
        // Sibling file without the suffix must be left alone.
        make_file(tmp.path(), "settings.ini");
        let report = clean_dir_sync(tmp.path()).expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut deleted = report.deleted_files.clone();
        deleted.sort();
        assert_eq!(
            deleted,
            vec![
                "Lower.Dmp".to_string(),
                "MIXED.DMP".to_string(),
                "crash01.dmp".to_string(),
            ]
        );
        assert!(
            tmp.path().join("settings.ini").exists(),
            "untouched file must remain"
        );
    }

    #[test]
    fn clean_dir_sync_removes_stray_dlls_case_insensitively() {
        let tmp = TempDir::new().expect("tempdir");
        make_file(tmp.path(), "localeemulator.dll");
        make_file(tmp.path(), "LoaderDll.DLL");
        // Other DLLs (game internals) must be left alone — only
        // the two named LR helpers get swept.
        make_file(tmp.path(), "MapleStory.dll");
        let report = clean_dir_sync(tmp.path()).expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut deleted = report.deleted_files.clone();
        deleted.sort();
        assert_eq!(
            deleted,
            vec![
                "LoaderDll.DLL".to_string(),
                "localeemulator.dll".to_string()
            ]
        );
        assert!(
            tmp.path().join("MapleStory.dll").exists(),
            "unrelated DLL must remain"
        );
    }

    #[test]
    fn clean_dir_sync_handles_mixed_layout_in_one_pass() {
        let tmp = TempDir::new().expect("tempdir");
        // All three categories present at once — a smoke test for
        // the combined sweep so the FIXED → `.$$$` → file order
        // doesn't accidentally trip on shared iteration state.
        make_dir(tmp.path(), "blob_storage");
        make_dir(tmp.path(), "patch.$$$");
        make_file(tmp.path(), "crash.dmp");
        make_file(tmp.path(), "localeemulator.dll");
        make_file(tmp.path(), "keep_me.txt");
        let report = clean_dir_sync(tmp.path()).expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        let mut dirs = report.deleted_dirs.clone();
        dirs.sort();
        assert_eq!(
            dirs,
            vec!["blob_storage".to_string(), "patch.$$$".to_string()]
        );
        let mut files = report.deleted_files.clone();
        files.sort();
        assert_eq!(
            files,
            vec!["crash.dmp".to_string(), "localeemulator.dll".to_string()]
        );
        assert!(tmp.path().join("keep_me.txt").exists());
    }

    #[test]
    fn is_stale_file_matches_dmp_suffix_case_insensitively() {
        assert!(is_stale_file("a.dmp"));
        assert!(is_stale_file("A.DMP"));
        assert!(is_stale_file("MixedCase.Dmp"));
        assert!(!is_stale_file("a.dmpx"));
        assert!(!is_stale_file("dmp"));
    }

    #[test]
    fn is_stale_file_matches_stray_dlls_case_insensitively() {
        assert!(is_stale_file("localeemulator.dll"));
        assert!(is_stale_file("LOCALEEMULATOR.DLL"));
        assert!(is_stale_file("loaderdll.dll"));
        assert!(is_stale_file("LoaderDLL.DLL"));
        // Substring matches must NOT trigger — only exact name.
        assert!(!is_stale_file("xlocaleemulator.dll"));
        assert!(!is_stale_file("loaderdll2.dll"));
    }

    // -----------------------------------------------------------------
    // clean_maple_game_cache — async wrapper end-to-end.
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn clean_maple_game_cache_end_to_end_smoke() {
        let tmp = TempDir::new().expect("tempdir");
        make_dir(tmp.path(), "GPUCache");
        make_file(tmp.path(), "crash.dmp");
        let exe_path = tmp.path().join("MapleStory.exe");
        // Touch the .exe so the parent-resolution lands somewhere
        // legitimate; the cleanup itself never touches the .exe.
        make_file(tmp.path(), "MapleStory.exe");
        let report = clean_maple_game_cache(exe_path.to_str().unwrap())
            .await
            .expect("cleanup should succeed");
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert_eq!(report.deleted_dirs, vec!["GPUCache".to_string()]);
        assert_eq!(report.deleted_files, vec!["crash.dmp".to_string()]);
        assert!(tmp.path().join("MapleStory.exe").exists());
    }

    #[tokio::test]
    async fn clean_maple_game_cache_propagates_validation_errors() {
        let err = clean_maple_game_cache("")
            .await
            .expect_err("empty path must propagate as PathEmpty");
        assert!(matches!(err, MapleCacheError::PathEmpty));
    }
}
