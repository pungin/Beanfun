//! Integration tests for `services::game::locale_remulator` — P8 chunk 8.2.
//!
//! These exercise the full public API surface:
//!
//! - [`release_all`] writes 5 binaries with SHA-256 matching the embedded
//!   build-time hashes.
//! - Second call in the same directory reports `Skipped` for every slot.
//! - Tampering one file (length-preserving byte flip) triggers exactly
//!   one `Rewritten` outcome and four `Skipped` — this is the lock-in
//!   for the "SHA-256 upgrade over WPF length-only" security change.
//! - `verify_file` round-trips against the embedded digests.
//!
//! Runs on every platform (the `launch_via_lr` Win32 spawner is the only
//! `#[cfg(windows)]` piece and is not exercised here — that needs a UAC
//! prompt + a real game binary, out of scope for unattended CI).
//!
//! [`release_all`]: beanfun_lib::services::game::release_all

use std::fs;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use beanfun_lib::services::game::{release_all, verify_file, ReleaseOutcome, LR_ASSETS};

/// Compute SHA-256 of a byte slice — dup'd here instead of reaching
/// into the crate's private `expected_sha256` helper so the
/// integration test stays on the public API contract.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}

/// Helper: assert every file in `target_dir` matches the exact bytes
/// the crate embedded via `include_bytes!`. The embedded bytes are
/// the single source of truth — `build.rs` hashes the same files at
/// compile time, so byte-equality implies SHA-256 equality.
fn assert_all_files_match_embedded(target_dir: &std::path::Path) {
    for (name, embedded) in LR_ASSETS {
        let path = target_dir.join(name);
        let written = fs::read(&path)
            .unwrap_or_else(|e| panic!("release_all must have produced {}: {e}", path.display()));
        assert_eq!(written.len(), embedded.len(), "{name} length mismatch");
        assert_eq!(
            sha256(&written),
            sha256(embedded),
            "{name} SHA-256 mismatch",
        );
    }
}

#[test]
fn release_all_populates_empty_dir_with_five_assets() {
    let dir = TempDir::new().unwrap();
    let outcomes = release_all(dir.path()).expect("release_all should succeed on empty dir");

    assert_eq!(outcomes.len(), 5);
    for (idx, outcome) in outcomes.iter().enumerate() {
        assert_eq!(
            *outcome,
            ReleaseOutcome::Created,
            "slot {idx} ({}) expected Created, got {:?}",
            LR_ASSETS[idx].0,
            outcome
        );
    }

    assert_all_files_match_embedded(dir.path());
}

#[test]
fn release_all_second_call_rewrites_every_asset() {
    // The release always writes: each launch leaves files that ARE the
    // embedded bytes, rather than files that once hashed like them.
    let dir = TempDir::new().unwrap();
    let _ = release_all(dir.path()).unwrap();

    let outcomes = release_all(dir.path()).expect("second release_all should succeed");
    for (idx, outcome) in outcomes.iter().enumerate() {
        assert_eq!(
            *outcome,
            ReleaseOutcome::Rewritten,
            "slot {idx} ({}) expected Rewritten, got {:?}",
            LR_ASSETS[idx].0,
            outcome
        );
    }
    assert_all_files_match_embedded(dir.path());
}

#[test]
fn release_all_restores_a_tampered_file() {
    // Under always-write every slot reports `Rewritten`; the guarantee
    // worth pinning is that tampering cannot survive a launch.
    let dir = TempDir::new().unwrap();
    let _ = release_all(dir.path()).unwrap();

    let victim_idx = 3;
    let (name, embedded) = LR_ASSETS[victim_idx];
    let victim = dir.path().join(name);
    let mut tampered = fs::read(&victim).unwrap();
    tampered[0] ^= 0xFF;
    fs::write(&victim, &tampered).unwrap();

    let outcomes = release_all(dir.path()).unwrap();

    assert!(outcomes.iter().all(|o| *o == ReleaseOutcome::Rewritten));
    assert_eq!(&fs::read(&victim).unwrap(), embedded);
    assert_all_files_match_embedded(dir.path());
}

#[test]
fn release_all_survives_deleted_file_in_populated_dir() {
    let dir = TempDir::new().unwrap();
    let _ = release_all(dir.path()).unwrap();

    // Delete one asset — it comes back as `Created` (nothing to
    // replace) while the rest are refreshed in place.
    let target_idx = 1;
    let name = LR_ASSETS[target_idx].0;
    fs::remove_file(dir.path().join(name)).unwrap();

    let outcomes = release_all(dir.path()).unwrap();
    for (idx, outcome) in outcomes.iter().enumerate() {
        let expected = if idx == target_idx {
            ReleaseOutcome::Created
        } else {
            ReleaseOutcome::Rewritten
        };
        assert_eq!(
            *outcome, expected,
            "slot {idx} ({}) expected {:?}, got {:?}",
            LR_ASSETS[idx].0, expected, outcome
        );
    }
    assert_all_files_match_embedded(dir.path());
}

#[test]
fn verify_file_returns_true_against_embedded_digest() {
    let dir = TempDir::new().unwrap();
    let _ = release_all(dir.path()).unwrap();

    for (name, bytes) in LR_ASSETS {
        let path = dir.path().join(name);
        let expected = sha256(bytes);
        assert!(
            verify_file(&path, &expected).unwrap(),
            "{name} must verify against embedded SHA-256 after release_all",
        );
    }
}

#[test]
fn embedded_bytes_length_matches_wpf_tree_files() {
    // Sanity: `include_bytes!` pulled the same files `build.rs` read.
    // A length mismatch here would mean the crate was built against a
    // different working copy of `Beanfun/LocaleRemulator/` — which
    // shouldn't happen, but is cheap to guard.
    let expected_lengths: &[(&str, usize)] = &[
        ("LRConfig.xml", 462),
        ("LRHookx32.dll", 57344),
        ("LRHookx64.dll", 77312),
        ("LRProc.exe", 91648),
        ("LRSubMenus.dll", 16384),
    ];
    for (name, expected) in expected_lengths {
        let bytes = LR_ASSETS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| *b)
            .unwrap_or_else(|| panic!("LR_ASSETS missing {name}"));
        assert_eq!(
            bytes.len(),
            *expected,
            "embedded bytes for {name} are not the expected size",
        );
    }
}
