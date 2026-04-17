//! Integration tests for `services::storage` chunk 5.1 — DPAPI primitives
//! plus registry-backed entropy round-trips.
//!
//! The Win32 DPAPI + registry APIs are only available on Windows; every
//! test in this file is `#[cfg(target_os = "windows")]` gated so the
//! suite still compiles on CI runners that happen to be Linux.
//!
//! # Registry isolation
//!
//! Tests that touch the registry use **unique per-test sub-keys** under
//! `SOFTWARE\BEANFUN_NEXT_TEST\<test_name>_<pid>` via the
//! [`read_from_registry_at`][r] / [`write_to_registry_at`][w] public
//! overrides. This guarantees:
//!
//! - Tests never overwrite the production `SOFTWARE\BEANFUN\ENTROPY`
//!   value the real Beanfun Next (or the legacy WPF build) may depend
//!   on.
//! - Parallel test runs cannot race each other because each test name +
//!   PID combination is unique per invocation.
//!
//! Each test also best-effort cleans up its sub-key in a final `Drop`
//! guard so repeated runs on the same machine don't accumulate orphan
//! registry entries.
//!
//! [r]: beanfun_next_lib::services::storage::entropy::read_from_registry_at
//! [w]: beanfun_next_lib::services::storage::entropy::write_to_registry_at

#![cfg(target_os = "windows")]

use beanfun_next_lib::services::storage::entropy::{
    read_from_registry_at, write_to_registry_at, Entropy,
};
use beanfun_next_lib::services::storage::{dpapi_protect, dpapi_unprotect, StorageError};

/// Parent registry path under which every test sub-key is created. Used
/// for a best-effort cleanup of the empty parent in [`RegistryScope::Drop`]
/// once its last child has been removed.
const TEST_REGISTRY_PARENT: &str = "SOFTWARE\\BEANFUN_NEXT_TEST";

/// Registry clean-up guard — deletes
/// `HKCU\SOFTWARE\BEANFUN_NEXT_TEST\<name>_<pid>` when dropped, whether
/// the test passed or panicked, and best-effort removes the empty
/// `BEANFUN_NEXT_TEST` parent so repeated test runs do not accumulate
/// orphan keys.
struct RegistryScope {
    subkey: String,
}

impl RegistryScope {
    fn new(name: &str) -> Self {
        let subkey = format!("{TEST_REGISTRY_PARENT}\\{name}_{}", std::process::id());
        // Make sure we start clean even if a previous aborted run left
        // a stale sub-key behind.
        let _ = delete_subkey(&subkey);
        Self { subkey }
    }
}

impl Drop for RegistryScope {
    fn drop(&mut self) {
        let _ = delete_subkey(&self.subkey);
        // Best-effort: try to remove the empty parent. `delete_subkey`
        // (non-recursive) fails when other parallel tests still own
        // sibling sub-keys, which is fine — the next teardown will
        // succeed once the last child is gone.
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

#[test]
fn end_to_end_save_load_cycle_round_trips_payload() {
    let scope = RegistryScope::new("end_to_end_save_load");

    // --- "save" side ---
    let fresh = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &fresh).expect("write entropy");
    let payload = b"{\"accountList\":[\"alice\",\"bob\"]}";
    let cipher = dpapi_protect(payload, fresh.as_bytes()).expect("protect");

    // --- "load" side (simulates a fresh process) ---
    let reread = read_from_registry_at(&scope.subkey, "ENTROPY").expect("read entropy");
    assert_eq!(reread.as_str(), fresh.as_str());
    let plain = dpapi_unprotect(&cipher, reread.as_bytes()).expect("unprotect");
    assert_eq!(plain, payload);
}

#[test]
fn read_from_registry_returns_entropy_missing_when_subkey_absent() {
    // Intentionally *do not* create the sub-key under the scope — reading
    // it must return the typed `EntropyMissing` variant rather than a raw
    // Registry I/O error so callers can treat it as "first-time run".
    // The scope is still constructed so that (a) parent cleanup runs in
    // `Drop` and (b) any future modification of this test that *does*
    // create a sub-key inherits automatic teardown.
    let scope = RegistryScope::new("never_exists");

    let err = read_from_registry_at(&scope.subkey, "ENTROPY")
        .expect_err("reading a missing sub-key must fail");
    assert!(
        matches!(err, StorageError::EntropyMissing),
        "expected EntropyMissing, got {err:?}"
    );
}

#[test]
fn read_from_registry_returns_entropy_missing_when_value_absent() {
    let scope = RegistryScope::new("value_absent");
    // Create the sub-key but leave ENTROPY value unset.
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.create_subkey(&scope.subkey).expect("create subkey");
    }

    let err = read_from_registry_at(&scope.subkey, "ENTROPY")
        .expect_err("reading a missing value must fail");
    assert!(
        matches!(err, StorageError::EntropyMissing),
        "expected EntropyMissing, got {err:?}"
    );
}

#[test]
fn read_from_registry_returns_entropy_shape_when_value_is_malformed() {
    let scope = RegistryScope::new("value_malformed");
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(&scope.subkey).expect("create subkey");
        // Deliberately write a malformed value — lowercase letters are
        // outside the [A-Z0-9]{8} grammar.
        key.set_value("ENTROPY", &"hellocat")
            .expect("set malformed value");
    }

    let err = read_from_registry_at(&scope.subkey, "ENTROPY")
        .expect_err("malformed entropy must fail shape check");
    assert!(
        matches!(err, StorageError::EntropyShape),
        "expected EntropyShape, got {err:?}"
    );
}

#[test]
fn large_payload_round_trips_through_entire_flow() {
    // 256 KB — well above any realistic Users.dat size, exercises the
    // LocalAlloc / LocalFree path with non-trivial allocations.
    let scope = RegistryScope::new("large_payload");
    let entropy = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &entropy).expect("write");

    let payload: Vec<u8> = (0..(256 * 1024)).map(|i| (i % 251) as u8).collect();
    let cipher = dpapi_protect(&payload, entropy.as_bytes()).expect("protect large");

    let reread = read_from_registry_at(&scope.subkey, "ENTROPY").expect("read");
    let plain = dpapi_unprotect(&cipher, reread.as_bytes()).expect("unprotect large");
    assert_eq!(plain, payload);
}

#[test]
fn mismatched_entropy_across_sessions_fails_unprotect() {
    // Simulates the scenario where the registry entropy value got
    // clobbered between save and load — unprotect must fail loudly
    // rather than silently return garbage.
    let scope = RegistryScope::new("mismatched_entropy");
    let original = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &original).expect("write original");
    let cipher = dpapi_protect(b"payload", original.as_bytes()).expect("protect");

    // Oops — something rewrote the registry with a different entropy.
    let clobbered = Entropy::generate();
    write_to_registry_at(&scope.subkey, "ENTROPY", &clobbered).expect("write clobbered");

    let reread = read_from_registry_at(&scope.subkey, "ENTROPY").expect("read clobbered");
    let err = dpapi_unprotect(&cipher, reread.as_bytes())
        .expect_err("unprotect with clobbered entropy must fail");
    assert!(
        matches!(
            err,
            StorageError::Dpapi {
                operation: "CryptUnprotectData",
                ..
            }
        ),
        "expected DPAPI unprotect error, got {err:?}"
    );
}

#[test]
fn write_then_read_preserves_exact_value() {
    let scope = RegistryScope::new("exact_value");
    let e = Entropy::parse("Q7X9PLMN").expect("static 8-char");
    write_to_registry_at(&scope.subkey, "ENTROPY", &e).expect("write");
    let reread = read_from_registry_at(&scope.subkey, "ENTROPY").expect("read");
    assert_eq!(reread.as_str(), "Q7X9PLMN");
    assert_eq!(reread.as_bytes(), b"Q7X9PLMN");
}
