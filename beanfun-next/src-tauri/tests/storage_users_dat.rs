//! Integration tests for `services::storage::users_dat` (chunk 5.2)
//! covering the full save / load / import round-trip plus the
//! catch-all delete-on-corruption and the `LegacyDataDetected`
//! fallback paths.
//!
//! Every test is `#[cfg(target_os = "windows")]` gated because the
//! IO-bearing `save_records_at` / `load_records_at` /
//! `import_records_at` rely on Win32 DPAPI + the registry. Tests use
//! the public `_at` lower-level overrides to point the entropy salt
//! at unique per-test sub-keys under
//! `SOFTWARE\BEANFUN_NEXT_TEST\users_<name>_<pid>`, which guarantees:
//!
//! - Production `SOFTWARE\BEANFUN\ENTROPY` is never touched.
//! - The production `Users.dat` cipher never becomes unreadable as a
//!   side effect of the test run.
//! - Parallel tests cannot race each other because each test name
//!   plus PID is unique per invocation.
//!
//! Each test also creates a fresh `tempfile::TempDir` so the on-disk
//! cipher is isolated. Both the registry sub-key and the temp dir
//! are cleaned up automatically (`Drop`).

#![cfg(target_os = "windows")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use beanfun_next_lib::services::storage::dpapi::dpapi_protect;
use beanfun_next_lib::services::storage::entropy::{write_to_registry_at, Entropy};
use beanfun_next_lib::services::storage::{
    default_users_dat_path, export_records, import_records_at, load_records_at, save_records_at,
    Account, Records, StorageError,
};
use tempfile::TempDir;

/// Parent registry path under which every test sub-key is created.
/// Best-effort cleaned up in `RegistryScope::drop` once its last
/// child has been removed.
const TEST_REGISTRY_PARENT: &str = "SOFTWARE\\BEANFUN_NEXT_TEST";

/// Per-test registry isolation guard. Allocates a unique sub-key
/// under `SOFTWARE\BEANFUN_NEXT_TEST\users_<name>_<pid>` and best-
/// effort deletes it (and the empty parent) on drop.
struct RegistryScope {
    subkey: String,
}

impl RegistryScope {
    fn new(name: &str) -> Self {
        let subkey = format!(
            "{TEST_REGISTRY_PARENT}\\users_{name}_{}",
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

fn sample_records() -> Records {
    Records(vec![
        Account {
            region: "TW".to_string(),
            account_id: "alice".to_string(),
            account_name: "Alice Display".to_string(),
            password: "alice-pwd".to_string(),
            verify: String::new(),
            method: 1,
            auto_login: true,
        },
        Account {
            region: "HK".to_string(),
            account_id: "bob".to_string(),
            account_name: "Bob Display".to_string(),
            password: "bob-pwd".to_string(),
            verify: "vrf-bob".to_string(),
            method: 2,
            auto_login: false,
        },
    ])
}

// =====================================================================
// Save / load round-trip
// =====================================================================

#[tokio::test]
async fn save_then_load_round_trips_records_byte_for_byte() {
    let scope = RegistryScope::new("rt_save_load");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");
    let original = sample_records();

    save_records_at(&path, &original, &scope.subkey, "ENTROPY")
        .await
        .expect("save");
    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load");

    assert_eq!(loaded, original);
}

#[tokio::test]
async fn save_creates_parent_directory_when_missing() {
    let scope = RegistryScope::new("mkdir_p");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("nested").join("subdir").join("Users.dat");
    let records = sample_records();

    save_records_at(&path, &records, &scope.subkey, "ENTROPY")
        .await
        .expect("save into missing parent dir");

    assert!(
        path.exists(),
        "Users.dat should be written into mkdir_p'd parent"
    );
    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load");
    assert_eq!(loaded, records);
}

// =====================================================================
// Catch-all corruption / failure ladder
// =====================================================================

#[tokio::test]
async fn load_on_missing_file_returns_empty_and_does_not_create_file() {
    let scope = RegistryScope::new("missing_file");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("does_not_exist.dat");

    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load missing file");
    assert_eq!(loaded, Records::default());
    assert!(!path.exists(), "load must not create the file");
}

#[tokio::test]
async fn load_after_entropy_clobber_deletes_file_and_returns_empty() {
    // Save → mutate the registry entropy under our feet → load.
    // DPAPI unprotect must fail, the catch-all must fire, the file
    // must be deleted, and the second load must return empty.
    let scope = RegistryScope::new("entropy_clobber");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    save_records_at(&path, &sample_records(), &scope.subkey, "ENTROPY")
        .await
        .expect("save");
    assert!(path.exists());

    // Replace the entropy with a different shape-valid value.
    let bogus = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &bogus).expect("clobber entropy");

    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load returns Ok(empty) after corruption");
    assert_eq!(loaded, Records::default());
    assert!(
        !path.exists(),
        "catch-all must delete the unreadable Users.dat"
    );
}

#[tokio::test]
async fn load_after_entropy_deletion_deletes_file_and_returns_empty() {
    // Save → delete the registry sub-key entirely → load. The
    // EntropyMissing branch flows into the catch-all and the file
    // gets deleted just like the clobber case.
    let scope = RegistryScope::new("entropy_deletion");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    save_records_at(&path, &sample_records(), &scope.subkey, "ENTROPY")
        .await
        .expect("save");
    delete_subkey(&scope.subkey).expect("delete entropy subkey");

    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load returns Ok(empty) after entropy deletion");
    assert_eq!(loaded, Records::default());
    assert!(!path.exists());
}

#[tokio::test]
async fn load_with_non_utf8_plaintext_deletes_file_and_returns_empty() {
    // Hand-craft a cipher whose DPAPI unprotect succeeds but yields
    // non-UTF-8 bytes — exercises the explicit String::from_utf8
    // branch in the catch-all.
    let scope = RegistryScope::new("non_utf8");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let entropy = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &entropy).expect("write entropy");
    let bad_bytes = vec![0xFF, 0xFE, 0xFD, 0xFC]; // invalid UTF-8 prefix
    let cipher = dpapi_protect(&bad_bytes, entropy.as_bytes()).expect("protect");
    std::fs::write(&path, &cipher).expect("write cipher");

    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load");
    assert_eq!(loaded, Records::default());
    assert!(!path.exists(), "non-UTF-8 plaintext must delete file");
}

// =====================================================================
// LegacyDataDetected (base64 OK + JSON fail) — file preserved
// =====================================================================

#[tokio::test]
async fn load_with_base64_legacy_plaintext_returns_legacy_detected_without_deleting() {
    // Plaintext is valid base64 of arbitrary "legacy" bytes — load
    // must surface them in `LegacyDataDetected` and **preserve** the
    // file (so a P6 NRBF migrator can recover it on the next try).
    let scope = RegistryScope::new("base64_legacy");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let legacy_payload: Vec<u8> = (0..64).map(|i| (i % 200) as u8).collect();
    let plaintext_b64 = BASE64.encode(&legacy_payload);

    let entropy = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &entropy).expect("write entropy");
    let cipher = dpapi_protect(plaintext_b64.as_bytes(), entropy.as_bytes()).expect("protect");
    std::fs::write(&path, &cipher).expect("write cipher");

    let err = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect_err("legacy base64 must surface as typed Err");
    match err {
        StorageError::LegacyDataDetected { raw_bytes } => {
            assert_eq!(raw_bytes, legacy_payload);
        }
        other => panic!("expected LegacyDataDetected, got {other:?}"),
    }
    assert!(
        path.exists(),
        "legacy detection must preserve the file for the migrator"
    );
}

#[tokio::test]
async fn load_with_pure_garbage_plaintext_returns_empty_without_deleting() {
    // Plaintext is neither JSON nor valid base64 — matches WPF
    // L494-550 "log error, return empty, do NOT delete" branch.
    let scope = RegistryScope::new("pure_garbage");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let entropy = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &entropy).expect("write entropy");
    // Contains '!' which is not in the base64 alphabet, so base64
    // decoding will fail too.
    let bad_plain = "definitely-not-json-or-base64!!!";
    let cipher = dpapi_protect(bad_plain.as_bytes(), entropy.as_bytes()).expect("protect");
    std::fs::write(&path, &cipher).expect("write cipher");

    let loaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("load returns Ok(empty) for pure garbage");
    assert_eq!(loaded, Records::default());
    assert!(
        path.exists(),
        "pure-garbage path must NOT delete the file (matches WPF L494-550)"
    );
}

// =====================================================================
// import_records — JSON / base64 / garbage trichotomy
// =====================================================================

#[tokio::test]
async fn import_records_with_valid_json_writes_file_and_returns_records() {
    let scope = RegistryScope::new("import_json");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let original = sample_records();
    let json = export_records(&original).expect("export");

    let imported = import_records_at(&path, &json, &scope.subkey, "ENTROPY")
        .await
        .expect("import");
    assert_eq!(imported, original);
    assert!(
        path.exists(),
        "import must write the file (matches WPF importRecord)"
    );

    let reloaded = load_records_at(&path, &scope.subkey, "ENTROPY")
        .await
        .expect("reload");
    assert_eq!(reloaded, original);
}

#[tokio::test]
async fn import_records_with_legacy_base64_returns_legacy_detected_without_writing() {
    let scope = RegistryScope::new("import_base64");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let legacy = b"some legacy nrbf bytes here";
    let blob = BASE64.encode(legacy);

    let err = import_records_at(&path, &blob, &scope.subkey, "ENTROPY")
        .await
        .expect_err("base64 must surface as typed Err");
    match err {
        StorageError::LegacyDataDetected { raw_bytes } => {
            assert_eq!(raw_bytes.as_slice(), legacy);
        }
        other => panic!("expected LegacyDataDetected, got {other:?}"),
    }
    assert!(
        !path.exists(),
        "import on legacy data must NOT write the file"
    );
}

#[tokio::test]
async fn import_records_with_pure_garbage_returns_json_error_without_writing() {
    let scope = RegistryScope::new("import_garbage");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let err = import_records_at(&path, "this-is-not-json!!!", &scope.subkey, "ENTROPY")
        .await
        .expect_err("garbage must surface as JSON error");
    assert!(
        matches!(err, StorageError::Json(_)),
        "expected Json error, got {err:?}"
    );
    assert!(!path.exists(), "failed import must NOT write the file");
}

#[tokio::test]
async fn export_then_import_preserves_records_through_roundtrip() {
    let scope = RegistryScope::new("export_import_rt");
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Users.dat");

    let original = sample_records();
    let exported = export_records(&original).expect("export");
    let reimported = import_records_at(&path, &exported, &scope.subkey, "ENTROPY")
        .await
        .expect("re-import");

    assert_eq!(reimported, original);
}

// =====================================================================
// default_users_dat_path
// =====================================================================

#[test]
fn default_users_dat_path_resolves_under_appdata_beanfun() {
    // We don't mutate APPDATA — the standard Windows session always
    // sets it. We only assert that the resolved path lands under
    // %APPDATA%\Beanfun\Users.dat exactly as WPF
    // `SpecialFolder.ApplicationData + "\\Beanfun\\Users.dat"` does.
    let appdata = std::env::var_os("APPDATA").expect("APPDATA must be set on Windows");
    let expected = std::path::PathBuf::from(&appdata)
        .join("Beanfun")
        .join("Users.dat");
    let resolved = default_users_dat_path().expect("resolve default path");
    assert_eq!(resolved, expected);
}
