//! On-disk logs, so a bug report can carry evidence.
//!
//! # Why this exists
//!
//! `main.rs` sets `windows_subsystem = "windows"` for release builds, so
//! there is no console and everything `tracing` writes to stderr goes
//! nowhere. Every user-reported problem therefore arrived as prose —
//! "I pressed the button and nothing happened" — and had to be
//! diagnosed by guesswork about a machine we cannot see. Issue #356
//! took several rounds of that before the reporter happened to mention
//! their hardware-acceleration setting, which was the whole answer.
//!
//! The app already emits exactly the lines that would have shortened
//! it (`ready-timeout` naming the stuck document state, the WebView2
//! `ERROR_INVALID_STATE` failure, the proxy's refusals). They just had
//! nowhere to land.
//!
//! # One file per launch
//!
//! Multi-instance use is normal here (issue #340 exists because people
//! run eight launchers at once), and several processes appending to one
//! file interleave their lines. So each launch writes its own
//! `beanfun-<timestamp>-<pid>.log` and the startup sweep keeps only the
//! newest [`MAX_LOG_FILES`].
//!
//! The sweep cannot hurt a live instance: Windows refuses to delete a
//! file another process holds open, and the resulting error is ignored.

use std::path::{Path, PathBuf};

/// How many log files to keep.
///
/// Sized for the multi-instance habit rather than for one launch at a
/// time: issue #340 exists because people run eight launchers at once,
/// and at eight files per session a smaller cap would discard yesterday
/// on this morning's first launch — exactly when "it happened a couple
/// of days ago" reports need it. The files are a few KB each, so the
/// generous bound costs nothing worth counting.
pub const MAX_LOG_FILES: usize = 30;

/// Prefix every log file shares, so the sweep can recognise its own
/// files and leave anything else in the folder alone.
const LOG_PREFIX: &str = "beanfun-";
const LOG_SUFFIX: &str = ".log";

/// Folder holding the log files, alongside `Config.xml`.
pub fn logs_dir(storage_root: &Path) -> PathBuf {
    storage_root.join("logs")
}

/// File name for a launch at `stamp` (`%Y%m%d-%H%M%S`) with process id
/// `pid`.
///
/// The timestamp leads so a lexicographic sort is a chronological sort,
/// which is what [`sweep_old_logs`] relies on.
pub fn log_file_name(stamp: &str, pid: u32) -> String {
    format!("{LOG_PREFIX}{stamp}-{pid}{LOG_SUFFIX}")
}

/// Delete all but the newest [`MAX_LOG_FILES`] of our log files.
///
/// Errors are swallowed by design: a locked file belongs to a running
/// instance, and failing to tidy up is never worth failing a launch
/// over. Returns how many files were removed (for tests and for the
/// startup log line).
pub fn sweep_old_logs(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    let mut ours: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(LOG_PREFIX) && name.ends_with(LOG_SUFFIX))
        })
        .collect();

    if ours.len() <= MAX_LOG_FILES {
        return 0;
    }

    // Newest last (the timestamp prefix makes name order time order),
    // so the excess to drop is the head of the list.
    ours.sort();
    let doomed = ours.len() - MAX_LOG_FILES;
    ours.into_iter()
        .take(doomed)
        .filter(|path| std::fs::remove_file(path).is_ok())
        .count()
}

/// Create the logs folder, sweep it, and open this launch's file.
///
/// Returns `None` when the folder or the file cannot be created — the
/// caller then runs without file logging rather than failing to start.
pub fn open_log_file(
    storage_root: &Path,
    stamp: &str,
    pid: u32,
) -> Option<(PathBuf, std::fs::File)> {
    let dir = logs_dir(storage_root);
    std::fs::create_dir_all(&dir).ok()?;
    sweep_old_logs(&dir);

    let path = dir.join(log_file_name(stamp, pid));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    Some((path, file))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").expect("write");
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = std::fs::read_dir(dir)
            .expect("read_dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn log_file_names_sort_chronologically() {
        let earlier = log_file_name("20260809-064512", 4242);
        let later = log_file_name("20260809-071500", 17);
        assert!(earlier < later, "{earlier} should sort before {later}");
    }

    #[test]
    fn a_folder_under_the_limit_is_left_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..MAX_LOG_FILES {
            touch(
                dir.path(),
                &log_file_name(&format!("20260809-0000{i:02}"), 1),
            );
        }
        assert_eq!(sweep_old_logs(dir.path()), 0);
        assert_eq!(names(dir.path()).len(), MAX_LOG_FILES);
    }

    #[test]
    fn the_sweep_keeps_the_newest_and_drops_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..(MAX_LOG_FILES + 5) {
            touch(
                dir.path(),
                &log_file_name(&format!("20260809-0000{i:02}"), 1),
            );
        }

        assert_eq!(sweep_old_logs(dir.path()), 5);

        let left = names(dir.path());
        assert_eq!(left.len(), MAX_LOG_FILES);
        // The five oldest went; the newest survived. Derived from the
        // constant so raising the retention cannot silently rot this.
        assert_eq!(
            left.first().map(String::as_str),
            Some(log_file_name("20260809-000005", 1).as_str())
        );
        assert_eq!(
            left.last().map(String::as_str),
            Some(log_file_name(&format!("20260809-0000{:02}", MAX_LOG_FILES + 4), 1).as_str())
        );
    }

    #[test]
    fn the_sweep_ignores_files_that_are_not_ours() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..(MAX_LOG_FILES + 3) {
            touch(
                dir.path(),
                &log_file_name(&format!("20260809-0000{i:02}"), 1),
            );
        }
        touch(dir.path(), "Config.xml.bak");
        touch(dir.path(), "notes.txt");

        sweep_old_logs(dir.path());

        let left = names(dir.path());
        assert!(left.contains(&"Config.xml.bak".to_string()));
        assert!(left.contains(&"notes.txt".to_string()));
        assert_eq!(left.len(), MAX_LOG_FILES + 2);
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        assert_eq!(sweep_old_logs(Path::new("Z:/definitely/not/here")), 0);
    }

    #[test]
    fn opening_creates_the_folder_and_the_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let (path, _file) =
            open_log_file(root.path(), "20260809-120000", 99).expect("opens a log file");

        assert!(path.exists());
        assert_eq!(path.parent(), Some(logs_dir(root.path()).as_path()));
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("beanfun-20260809-120000-99.log")
        );
    }
}
