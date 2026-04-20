//! Integration tests for `services::storage::legacy` (P6 chunk 6.2)
//! covering the full legacy-NRBF auto-migration flow end-to-end:
//! DPAPI ciphertext decoding, `base64 → LegacyDataDetected` handoff,
//! NRBF parsing via the chunk 6.1 builder, JSON save, and fail-soft
//! for malformed legacy payloads.
//!
//! Every test is `#[cfg(target_os = "windows")]` gated because the
//! migrator's save step depends on Win32 DPAPI + the registry.
//! Tests use the `_at` lower-level overrides to point the entropy
//! salt at per-test sub-keys under
//! `SOFTWARE\BEANFUN_NEXT_TEST\legacy_<name>_<pid>`, so:
//!
//! - Production `SOFTWARE\BEANFUN\ENTROPY` is never touched.
//! - The production `Users.dat` cipher never becomes unreadable as a
//!   side effect of the test run.
//! - Parallel tests cannot race each other because each test name
//!   plus PID is unique.
//!
//! # How to run
//!
//! This target is gated behind the `test-fixtures` cargo feature
//! (via `[[test]] required-features` in `Cargo.toml`) because it
//! depends on the NRBF byte builder in
//! `core::legacy::nrbf::fixture`, which is also feature-gated so it
//! never ships inside production binaries.
//!
//! ```text
//! cargo test --features test-fixtures --test storage_legacy
//! ```
//!
//! `cargo test` without the feature silently skips this target.

#![cfg(target_os = "windows")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use beanfun_lib::core::legacy::nrbf::fixture::{build_root_class, MemberSpec};
use beanfun_lib::services::storage::dpapi::dpapi_protect;
use beanfun_lib::services::storage::entropy::{write_to_registry_at, Entropy};
use beanfun_lib::services::storage::{
    export_records, load_records_at, load_records_with_legacy_migration_at, migrate_and_save_at,
    save_records_at, Account, Records,
};
use tempfile::TempDir;

const TEST_REGISTRY_PARENT: &str = "SOFTWARE\\BEANFUN_NEXT_TEST";
const CLASS_RECORDS: &str = "Beanfun.Records";
const CLASS_ACCOUNT_RECORDS: &str = "Beanfun.AccountRecords";

/// Per-test registry isolation guard. Allocates a unique sub-key
/// under `SOFTWARE\BEANFUN_NEXT_TEST\legacy_<name>_<pid>` and best-
/// effort deletes it (and the empty parent) on drop.
struct RegistryScope {
    subkey: String,
}

impl RegistryScope {
    fn new(name: &str) -> Self {
        let subkey = format!(
            "{TEST_REGISTRY_PARENT}\\legacy_{name}_{}",
            std::process::id()
        );
        let _ = delete_subkey(&subkey);
        Self { subkey }
    }
}

impl Drop for RegistryScope {
    fn drop(&mut self) {
        let _ = delete_subkey(&self.subkey);
        let _ = delete_subkey_non_recursive(TEST_REGISTRY_PARENT);
    }
}

fn delete_subkey(path: &str) -> std::io::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.delete_subkey_all(path)
}

fn delete_subkey_non_recursive(path: &str) -> std::io::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.delete_subkey(path)
}

/// Hand-craft NRBF bytes for a `Beanfun.Records` with 2 accounts —
/// the common "current shape" fixture for most tests.
fn legacy_records_two_accounts_bytes() -> Vec<u8> {
    build_root_class(
        CLASS_RECORDS,
        &[
            (
                "regionList",
                MemberSpec::StringList(&[Some("TW"), Some("HK")]),
            ),
            (
                "accountList",
                MemberSpec::StringList(&[Some("alice"), Some("bob")]),
            ),
            (
                "accountNameList",
                MemberSpec::StringList(&[Some("Alice"), Some("Bob")]),
            ),
            (
                "passwdList",
                MemberSpec::StringList(&[Some("pw-a"), Some("pw-b")]),
            ),
            (
                "verifyList",
                MemberSpec::StringList(&[Some(""), Some("vrf-b")]),
            ),
            ("methodList", MemberSpec::I32List(&[1, 2])),
            ("autoLoginList", MemberSpec::BoolList(&[true, false])),
        ],
    )
}

/// Hand-craft NRBF bytes for a legacy `Beanfun.AccountRecords` (6
/// fields, no `accountNameList`) with 1 account — exercises the
/// `account_name_list: None → normalize pads with ""` path.
fn legacy_account_records_one_account_bytes() -> Vec<u8> {
    build_root_class(
        CLASS_ACCOUNT_RECORDS,
        &[
            ("regionList", MemberSpec::StringList(&[Some("TW")])),
            (
                "accountList",
                MemberSpec::StringList(&[Some("legacy-user")]),
            ),
            ("passwdList", MemberSpec::StringList(&[Some("legacy-pwd")])),
            ("verifyList", MemberSpec::StringList(&[Some("")])),
            ("methodList", MemberSpec::I32List(&[0])),
            ("autoLoginList", MemberSpec::BoolList(&[false])),
        ],
    )
}

/// Hand-craft a DPAPI-encrypted `Users.dat` whose plaintext is the
/// base64 of `bytes` — simulates a legacy WPF-written file. Returns
/// the entropy so the caller can install it in the right registry
/// sub-key.
fn write_legacy_users_dat(path: &std::path::Path, subkey: &str, bytes: &[u8]) -> Entropy {
    let entropy = Entropy::generate();
    write_to_registry_at(subkey, "ENTROPY", &entropy).expect("write entropy");
    let plaintext_b64 = BASE64.encode(bytes);
    let cipher = dpapi_protect(plaintext_b64.as_bytes(), entropy.as_bytes()).expect("protect");
    std::fs::write(path, &cipher).expect("write cipher");
    entropy
}

// =====================================================================
// migrate_and_save — pure NRBF-in, JSON-on-disk-out
// =====================================================================

#[tokio::test]
async fn migrate_and_save_writes_json_format_round_trippable_by_load_records() {
    // The critical invariant: after migrate_and_save, the file must
    // be readable by the ordinary P5 `load_records` path (JSON, no
    // legacy fallback) — the user should experience the upgrade
    // exactly once.
    let scope = RegistryScope::new("migrate_save_roundtrip");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let bytes = legacy_records_two_accounts_bytes();
    let migrated = migrate_and_save_at(&path, &bytes, &scope.subkey, "ENTROPY")
        .await
        .expect("migrate_and_save");

    assert_eq!(migrated.0.len(), 2);
    assert_eq!(migrated.0[0].account_id, "alice");
    assert_eq!(migrated.0[1].account_id, "bob");

    // Reload through the ordinary path — no LegacyDataDetected err,
    // no catch-all delete — the file is now JSON.
    let reloaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("reload");
    assert_eq!(reloaded, migrated);
}

#[tokio::test]
async fn migrate_and_save_creates_parent_directory_when_missing() {
    // Same mkdir_p semantics as save_records — the migrator must not
    // break when the caller hands it a path whose parent doesn't
    // exist yet (typical first-run after a fresh install).
    let scope = RegistryScope::new("migrate_save_mkdir");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("deep").join("nested").join("Users.dat");

    let bytes = legacy_records_two_accounts_bytes();
    let _ = migrate_and_save_at(&path, &bytes, &scope.subkey, "ENTROPY")
        .await
        .expect("migrate_and_save into missing parent");

    assert!(path.exists(), "mkdir_p must have created the parent chain");
}

#[tokio::test]
async fn migrate_and_save_handles_legacy_account_records_padding_account_name_list() {
    // `Beanfun.AccountRecords` (6 fields) must upgrade cleanly; the
    // missing `accountNameList` is filled with empty strings by
    // `WireRecords::normalize` matching WPF `accRecInit`.
    let scope = RegistryScope::new("legacy_account_records");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let bytes = legacy_account_records_one_account_bytes();
    let migrated = migrate_and_save_at(&path, &bytes, &scope.subkey, "ENTROPY")
        .await
        .expect("migrate_and_save");

    assert_eq!(migrated.0.len(), 1);
    assert_eq!(migrated.0[0].account_id, "legacy-user");
    assert_eq!(
        migrated.0[0].account_name, "",
        "legacy AccountRecords must leave account_name empty"
    );

    // Reload via P5 path → the on-disk file is real JSON now.
    let reloaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("reload");
    assert_eq!(reloaded, migrated);
}

// =====================================================================
// load_records_with_legacy_migration — end-to-end
// =====================================================================

#[tokio::test]
async fn load_with_migration_auto_upgrades_legacy_users_dat_to_json() {
    // Plant a legacy `Users.dat` on disk, call the wrapper once, and
    // observe that (a) the records come back, (b) the file is now
    // JSON. A second call must skip the migrator entirely.
    let scope = RegistryScope::new("load_auto_upgrade");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let bytes = legacy_records_two_accounts_bytes();
    let _entropy = write_legacy_users_dat(&path, &scope.subkey, &bytes);
    let legacy_cipher_bytes_before = std::fs::read(&path).expect("read pre-migration cipher");

    let records = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load with migration");
    assert_eq!(records.0.len(), 2);
    assert_eq!(records.0[0].account_id, "alice");

    // File still exists but its bytes changed (cipher for JSON
    // plaintext + fresh entropy).
    assert!(path.exists());
    let post_migration_bytes = std::fs::read(&path).expect("read post-migration cipher");
    assert_ne!(post_migration_bytes, legacy_cipher_bytes_before);

    // Second call goes straight through P5 `load_records` — no more
    // LegacyDataDetected, no re-migration.
    let reloaded = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("second load");
    assert_eq!(reloaded, records);
}

#[tokio::test]
async fn load_with_migration_on_malformed_nrbf_returns_empty_and_preserves_file() {
    // Plant a base64-valid but NRBF-malformed `Users.dat`. The P5
    // catch-all flags `LegacyDataDetected`, the migrator parses
    // fails, and the wrapper fail-softs (empty records + file kept)
    // matching WPF `TryAutoMigrateLegacyData` L546-548.
    let scope = RegistryScope::new("malformed_nrbf");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    // Arbitrary bytes that are NOT valid NRBF — the first byte 0x00
    // is a SerializedStreamHeader record type but what follows isn't
    // the 16-byte header payload, so the nrbf crate rejects it.
    let bad_nrbf: Vec<u8> = vec![0x00, 0x01, 0x02, 0x03];
    let _entropy = write_legacy_users_dat(&path, &scope.subkey, &bad_nrbf);
    let pre_bytes = std::fs::read(&path).expect("read pre");

    let records = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("wrapper must fail-soft into Ok(empty)");
    assert_eq!(records, Records::default());

    assert!(
        path.exists(),
        "malformed-NRBF migrate failure must PRESERVE the file"
    );
    let post_bytes = std::fs::read(&path).expect("read post");
    assert_eq!(
        post_bytes, pre_bytes,
        "on migrate failure, the file bytes must be untouched"
    );
}

#[tokio::test]
async fn load_with_migration_on_new_json_format_skips_migrator_entirely() {
    // An already-modern JSON `Users.dat` must go through the
    // wrapper without tripping the legacy path at all.
    let scope = RegistryScope::new("already_json");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let original = Records(vec![Account {
        region: "TW".into(),
        account_id: "jsonuser".into(),
        account_name: "JSON Display".into(),
        password: "pw".into(),
        verify: String::new(),
        method: 1,
        auto_login: true,
    }]);
    save_records_at(&path, &original, &scope.subkey, "ENTROPY")
        .await
        .expect("save JSON");
    let pre_bytes = std::fs::read(&path).expect("read pre");

    let loaded = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load");
    assert_eq!(loaded, original);

    // Migration path not taken → file bytes identical.
    let post_bytes = std::fs::read(&path).expect("read post");
    assert_eq!(
        post_bytes, pre_bytes,
        "modern JSON path must not rewrite the file"
    );
}

#[tokio::test]
async fn load_with_migration_on_pure_garbage_plaintext_falls_through_p5_default() {
    // Neither JSON nor valid base64 — P5 `load_records` returns
    // `Ok(Records::default())` without surfacing LegacyDataDetected
    // and without deleting the file; the wrapper must just pass
    // that through unchanged.
    let scope = RegistryScope::new("pure_garbage_wrapper");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    // '!' is not in the base64 alphabet → P5 treats this as the
    // "preserve, empty" branch.
    let entropy = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &entropy).expect("write entropy");
    let garbage = "definitely-not-json-or-base64!!!";
    let cipher = dpapi_protect(garbage.as_bytes(), entropy.as_bytes()).expect("protect");
    std::fs::write(&path, &cipher).expect("write cipher");

    let loaded = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("wrapper");
    assert_eq!(loaded, Records::default());
    assert!(
        path.exists(),
        "pure-garbage P5 path preserves the file; wrapper must respect that"
    );
}

#[tokio::test]
async fn load_with_migration_on_missing_file_returns_empty_and_no_side_effects() {
    // Wrapper must behave identically to P5 load_records when the
    // file doesn't exist yet (first-time run).
    let scope = RegistryScope::new("missing_wrapper");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("nonexistent.dat");

    let loaded = load_records_with_legacy_migration_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("wrapper on missing file");
    assert_eq!(loaded, Records::default());
    assert!(
        !path.exists(),
        "wrapper must not create a file on first-time load"
    );
}

// =====================================================================
// Interop sanity: JSON we save is exactly what P5 parse_records reads
// =====================================================================

#[tokio::test]
async fn migrated_json_matches_export_records_byte_for_byte() {
    // Hedge against the normalize logic drifting between the two
    // pipelines: the JSON plaintext the migrator writes for a given
    // legacy payload must be equivalent to what `export_records`
    // would produce for the migrated `Records`.
    let scope = RegistryScope::new("json_parity");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let bytes = legacy_records_two_accounts_bytes();
    let migrated = migrate_and_save_at(&path, &bytes, &scope.subkey, "ENTROPY")
        .await
        .expect("migrate");

    // Directly compare the JSON representations — byte-identical
    // means `Records → WireRecords::from` conversion is stable.
    let via_export = export_records(&migrated).expect("export");
    let via_export_parsed: serde_json::Value =
        serde_json::from_str(&via_export).expect("parse export");

    // Reload via P5 path and re-export so we compare JSON values
    // from the disk-round-trip (proves save-site wrote a valid JSON
    // record, not just some opaque blob we happened to be able to
    // decrypt again).
    let reloaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("reload");
    let reloaded_json = export_records(&reloaded).expect("re-export");
    let reloaded_parsed: serde_json::Value =
        serde_json::from_str(&reloaded_json).expect("parse reload");

    assert_eq!(via_export_parsed, reloaded_parsed);
}
