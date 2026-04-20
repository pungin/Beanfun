//! Accounts (Users.dat) storage commands.
//!
//! Ports the WPF `AccountManager` read / mutate / import / export
//! surface (`Beanfun/Helper/AccountManager.cs`) to the Tauri IPC
//! boundary. Five commands cover the full surface:
//!
//! - [`load_accounts`] — decrypt `Users.dat` and return the row-
//!   shaped [`Account`] list (WPF `loadRecord` / `AccRec`).
//! - [`save_account`] — upsert by `(region, account_id)` and
//!   return the updated list (WPF `storeRecord` rolled together
//!   with the row-find-or-append branch from
//!   `btnAddService_Click`).
//! - [`remove_account`] — delete by `(region, account_id)` and
//!   return the updated list (WPF `removeRecord` L474-492).
//! - [`import_records`] — read an external plaintext JSON file
//!   and overwrite `Users.dat` (WPF `importRecord` L385-439 —
//!   user-picked JSON file from the "匯入帳號" menu).
//! - [`export_records`] — serialise the current `Users.dat`
//!   contents as plaintext JSON to an external path (WPF
//!   `exportRecord` L440-471).
//! - [`backup_export`] — same as [`export_records`] but the JSON
//!   is then AES-128-CBC encrypted under a user-supplied password
//!   and returned as base64 (WPF `AccRecovery.Export_Button_Click`,
//!   `Beanfun/Windows/AccRecovery.xaml.cs` L28-45). Wire format is
//!   1:1 compatible — see [`crate::services::storage::aes_backup`].
//! - [`backup_restore`] — inverse of [`backup_export`]: AES-decrypt
//!   the user-supplied base64 ciphertext, then route through the
//!   same `Users.dat` overwrite pipeline as [`import_records`]
//!   (WPF `AccRecovery.Recovery_Button_Click` L47-79).
//!
//! # Q7 = A: plaintext pass-through
//!
//! The P10.3 pre-flight Q7 decision commits to surfacing decrypted
//! passwords **verbatim** to the frontend and writing them
//! **verbatim** into export JSON files. Rationale:
//!
//! 1. **WPF parity.** The legacy client already hands the UI the
//!    decrypted password (for auto-fill / auto-launch) and writes
//!    plaintext JSON on export. Matching this keeps existing user
//!    workflows (exporting from WPF, importing here) lossless.
//! 2. **Shared trust boundary.** The Tauri webview and the Rust
//!    backend run inside the same Windows user session; there is
//!    no process isolation between the UI surface and the on-
//!    disk DPAPI ciphertext. Redacting before the webview does
//!    not change what an attacker with the user session can read.
//! 3. **Future-proofing stays cheap.** If P12+ introduces a
//!    separate unprivileged renderer (e.g. remote-admin mode) the
//!    redaction can be re-applied at that IPC boundary without
//!    churning this module.
//!
//! Export JSON files carry a plaintext password column and should
//! be treated as equivalent to a saved-password dump — users are
//! expected to store them with the same care as a `Users.dat`
//! backup.
//!
//! # Platform gating
//!
//! `services::storage::{load_records, save_records, import_records,
//! load_records_with_legacy_migration}` are all
//! `#[cfg(target_os = "windows")]` because they ride DPAPI
//! (`CryptProtectData` / `CryptUnprotectData`) for which there is
//! no portable equivalent. The commands themselves stay
//! unconditional (so `bindings.ts` exposes the same symbol set on
//! every host) and surface a `storage.platform_unsupported`
//! [`CommandError`] on non-Windows builds; production ships
//! Windows-only regardless.
//!
//! # `mutate_records_internal` helper
//!
//! `save_account` and `remove_account` share a `load → mutate →
//! save_records` pipeline. Centralising this under one
//! `mutate_records_internal` helper (P10.2 pattern — see
//! `list_accounts_internal` in `commands/account.rs`) avoids
//! duplicating the entropy re-gen / DPAPI re-encrypt / atomic-
//! overwrite dance in two command bodies. The mutator closure is
//! pure (infallible) — the only errors are IO / DPAPI from the
//! enclosing pipeline, already typed through
//! [`crate::services::storage::StorageError`].

use tauri::State;

use crate::commands::error::CommandError;
use crate::commands::state::AppState;
#[cfg(target_os = "windows")]
use crate::services::storage;
use crate::services::storage::Account;

/// On-disk filename under [`AppState::storage_root`] — matches WPF
/// `AccountManager.cs` L14-16
/// (`SpecialFolder.ApplicationData\Beanfun\Users.dat`).
const USERS_DAT_FILE_NAME: &str = "Users.dat";

/// Error code returned by every command in this module when built
/// for a non-Windows target. The Rust toolchain / CI lets us
/// `cargo check` on macOS / Linux dev boxes; this error lets the
/// frontend distinguish "no accounts" from "not a real OS".
///
/// Kept at module scope (rather than inlined into
/// `platform_unsupported_error`) so the unit test in
/// `tests::platform_unsupported_code_is_stable` can pin the
/// exact string against rename drift — the frontend contract
/// depends on this specific value. (Both referenced items are
/// `cfg(not(target_os = "windows"))` / `cfg(test)` gated and
/// therefore not visible to `cargo doc` on a Windows host build,
/// so they are intentionally not intra-doc links.)
#[cfg_attr(target_os = "windows", allow(dead_code))]
const PLATFORM_UNSUPPORTED_CODE: &str = "storage.platform_unsupported";

#[cfg(not(target_os = "windows"))]
fn platform_unsupported_error() -> CommandError {
    CommandError::new(
        PLATFORM_UNSUPPORTED_CODE,
        "storage commands require Windows (DPAPI-backed Users.dat)",
    )
}

// =====================================================================
// Windows implementation
// =====================================================================

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use crate::services::storage::Records;
    use serde_json::json;
    use std::path::PathBuf;

    pub(super) fn users_dat_path(state: &AppState) -> PathBuf {
        state.storage_root.join(USERS_DAT_FILE_NAME)
    }

    pub(super) async fn load_accounts_impl(state: &AppState) -> Result<Vec<Account>, CommandError> {
        let path = users_dat_path(state);
        let records = storage::load_records_with_legacy_migration(&path).await?;
        Ok(records.0)
    }

    /// Load → mutate → save pipeline wrapper. The mutator is
    /// **infallible** by design — save/remove semantics are local
    /// list operations (`Vec::push` / `retain`) that cannot fail —
    /// keeping the closure error-free lets the compiler inline it
    /// and centralises the error surface on the IO / DPAPI calls.
    pub(super) async fn mutate_records_internal<F>(
        state: &AppState,
        mutator: F,
    ) -> Result<Vec<Account>, CommandError>
    where
        F: FnOnce(&mut Records),
    {
        let path = users_dat_path(state);
        let mut records = storage::load_records_with_legacy_migration(&path).await?;
        mutator(&mut records);
        storage::save_records(&path, &records).await?;
        Ok(records.0)
    }

    pub(super) async fn save_account_impl(
        state: &AppState,
        account: Account,
    ) -> Result<Vec<Account>, CommandError> {
        mutate_records_internal(state, |records| {
            upsert_account(records, account);
        })
        .await
    }

    pub(super) async fn remove_account_impl(
        state: &AppState,
        region: String,
        account_id: String,
    ) -> Result<Vec<Account>, CommandError> {
        mutate_records_internal(state, |records| {
            records
                .0
                .retain(|a| !(a.region == region && a.account_id == account_id));
        })
        .await
    }

    pub(super) async fn import_records_impl(
        state: &AppState,
        external_json_path: String,
    ) -> Result<Vec<Account>, CommandError> {
        let users_dat = users_dat_path(state);
        let json = tokio::fs::read_to_string(&external_json_path)
            .await
            .map_err(|err| {
                CommandError::new(
                    "storage.import_read_failed",
                    format!("failed to read import file `{external_json_path}`: {err}"),
                )
                .with_details(json!({
                    "path": external_json_path,
                    "io_kind": format!("{:?}", err.kind()),
                }))
            })?;
        let records = storage::import_records(&users_dat, &json).await?;
        Ok(records.0)
    }

    pub(super) async fn export_records_impl(
        state: &AppState,
        external_json_path: String,
    ) -> Result<(), CommandError> {
        let users_dat = users_dat_path(state);
        let records = storage::load_records_with_legacy_migration(&users_dat).await?;
        let json = storage::export_records(&records)?;
        tokio::fs::write(&external_json_path, json)
            .await
            .map_err(|err| {
                CommandError::new(
                    "storage.export_write_failed",
                    format!("failed to write export file `{external_json_path}`: {err}"),
                )
                .with_details(json!({
                    "path": external_json_path,
                    "io_kind": format!("{:?}", err.kind()),
                }))
            })?;
        Ok(())
    }

    /// Decrypt → serialize → AES-encrypt → base64 the current
    /// `Users.dat` contents. Returns the base64 ciphertext directly
    /// (no IO — the frontend renders it in a textarea so the user
    /// can copy it elsewhere). Mirrors WPF
    /// `AccRecovery.Export_Button_Click`.
    ///
    /// `password` is consumed by reference and never logged or
    /// persisted; it lives only on the request stack frame for the
    /// duration of the call.
    pub(super) async fn backup_export_impl(
        state: &AppState,
        password: String,
    ) -> Result<String, CommandError> {
        let users_dat = users_dat_path(state);
        let records = storage::load_records_with_legacy_migration(&users_dat).await?;
        let json = storage::export_records(&records)?;
        Ok(storage::aes_backup_encrypt(&json, &password))
    }

    /// AES-decrypt the user-supplied `ciphertext_b64` under
    /// `password`, then route the decrypted JSON through the
    /// shared `Users.dat` import pipeline (same path as
    /// [`import_records_impl`]). Returns the post-restore account
    /// list so the frontend can refresh in one round-trip without
    /// a follow-up [`load_accounts`] call. Mirrors WPF
    /// `AccRecovery.Recovery_Button_Click`.
    ///
    /// # Error mapping
    ///
    /// - `storage.aes_backup_*` — AES / base64 / UTF-8 failure
    ///   (the frontend maps these to the WPF `MsgDecryptFailed`
    ///   toast: "wrong password / corrupted blob").
    /// - `storage.json_failed` / `storage.dpapi_*` /
    ///   `storage.io_failed` / etc. — post-decrypt JSON validation
    ///   or `Users.dat` overwrite failure (frontend maps to WPF
    ///   `RecoveryFailed` toast: "decrypt OK, but the contents
    ///   were not a valid Users.dat backup").
    pub(super) async fn backup_restore_impl(
        state: &AppState,
        password: String,
        ciphertext_b64: String,
    ) -> Result<Vec<Account>, CommandError> {
        let plaintext_json = storage::aes_backup_decrypt(&ciphertext_b64, &password)?;
        let users_dat = users_dat_path(state);
        let records = storage::import_records(&users_dat, &plaintext_json).await?;
        Ok(records.0)
    }

    /// Upsert `account` into `records` by `(region, account_id)`
    /// — update in place when the row exists, append otherwise.
    /// Mirrors the WPF `btnAddService_Click` branch at
    /// `Beanfun/MainWindow.xaml.cs` ≈L700-730 (add vs. update
    /// decision).
    fn upsert_account(records: &mut Records, account: Account) {
        if let Some(existing) = records
            .0
            .iter_mut()
            .find(|a| a.region == account.region && a.account_id == account.account_id)
        {
            *existing = account;
        } else {
            records.0.push(account);
        }
    }

    #[cfg(test)]
    mod upsert_tests {
        use super::*;

        fn acc(region: &str, id: &str, name: &str) -> Account {
            Account {
                region: region.into(),
                account_id: id.into(),
                account_name: name.into(),
                password: format!("pw_{id}"),
                verify: String::new(),
                method: 0,
                auto_login: false,
                last_login_at: None,
            }
        }

        #[test]
        fn upsert_appends_when_account_absent() {
            let mut records = Records(vec![acc("TW", "u1", "u1name")]);
            upsert_account(&mut records, acc("TW", "u2", "u2name"));
            assert_eq!(records.0.len(), 2);
            assert_eq!(records.0[1].account_id, "u2");
        }

        #[test]
        fn upsert_updates_in_place_when_region_and_id_match() {
            let mut records = Records(vec![acc("TW", "u1", "old"), acc("HK", "u1", "hk")]);
            let mut updated = acc("TW", "u1", "new");
            updated.password = "new_pw".to_string();
            upsert_account(&mut records, updated);
            assert_eq!(records.0.len(), 2, "no append for matching row");
            assert_eq!(records.0[0].account_name, "new");
            assert_eq!(records.0[0].password, "new_pw");
            // HK row untouched — `(region, account_id)` is the
            // composite key; a TW/u1 update must not affect HK/u1.
            assert_eq!(records.0[1].region, "HK");
            assert_eq!(records.0[1].account_name, "hk");
        }

        #[test]
        fn upsert_preserves_order_across_many_rows() {
            let mut records = Records(vec![
                acc("TW", "a", "na"),
                acc("TW", "b", "nb"),
                acc("TW", "c", "nc"),
            ]);
            upsert_account(&mut records, acc("TW", "b", "nb_updated"));
            let ids: Vec<&str> = records.0.iter().map(|a| a.account_id.as_str()).collect();
            assert_eq!(ids, vec!["a", "b", "c"]);
            assert_eq!(records.0[1].account_name, "nb_updated");
        }
    }
}

// =====================================================================
// Commands (unconditional signatures; body delegates to `imp` on
// Windows and returns `storage.platform_unsupported` elsewhere so
// `bindings.ts` stays uniform across build targets)
// =====================================================================

/// Decrypt `Users.dat` and return every saved account as a row-
/// shaped [`Account`] list.
///
/// First-time boot (file missing) returns an empty list — WPF
/// behaviour. Legacy P6 NRBF files are auto-migrated to the new
/// JSON format via
/// [`crate::services::storage::load_records_with_legacy_migration`]
/// before the decrypted rows are returned.
///
/// # Errors
///
/// - `storage.dpapi_failed` / `storage.io_failed` / `storage.json_failed` /
///   `storage.entropy_missing` / `storage.entropy_shape` — see
///   [`crate::services::storage::StorageError`] for each cause.
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn load_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::load_accounts_impl(&state).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        Err(platform_unsupported_error())
    }
}

/// Upsert a single `account` into `Users.dat`, matched by
/// `(region, account_id)`. Returns the full updated list so the
/// frontend can refresh without another round-trip.
///
/// See module docs for the Q7 = A plaintext-password policy that
/// governs `account.password`.
///
/// # Errors
///
/// - `storage.*` — DPAPI / IO / registry surface from
///   [`crate::services::storage::StorageError`].
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn save_account(
    state: State<'_, AppState>,
    account: Account,
) -> Result<Vec<Account>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::save_account_impl(&state, account).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, account);
        Err(platform_unsupported_error())
    }
}

/// Delete the row matching `(region, account_id)` from `Users.dat`.
/// No-op (not an error) when the row is absent. Returns the full
/// updated list so the frontend can refresh in one round-trip.
///
/// # Errors
///
/// - `storage.*` — DPAPI / IO / registry surface from
///   [`crate::services::storage::StorageError`].
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn remove_account(
    state: State<'_, AppState>,
    region: String,
    account_id: String,
) -> Result<Vec<Account>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::remove_account_impl(&state, region, account_id).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, region, account_id);
        Err(platform_unsupported_error())
    }
}

/// Read an external plaintext JSON file at `path` and overwrite
/// `Users.dat` with its contents (re-encrypted under a fresh
/// DPAPI entropy). Matches WPF `importRecord` — the JSON format is
/// the WPF parallel-columns wire shape, byte-for-byte compatible
/// with files exported by either the legacy client or
/// [`export_records`].
///
/// Returns the newly-persisted account list (same shape as
/// [`load_accounts`]).
///
/// # Errors
///
/// - `storage.import_read_failed` — external file I/O failure
///   (file missing, permission denied, …). Details include `path`
///   and `io_kind`.
/// - `storage.json_failed` — the external file is not valid
///   `WireRecords` JSON.
/// - `storage.*` (DPAPI / registry / IO) — failure during the
///   re-encrypt + overwrite step against `Users.dat`.
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn import_records(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<Account>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::import_records_impl(&state, path).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, path);
        Err(platform_unsupported_error())
    }
}

/// Serialise the current `Users.dat` contents as the WPF parallel-
/// columns JSON wire format and write to `path` (external file).
/// Matches WPF `exportRecord`.
///
/// **Plaintext password caveat:** the output file includes every
/// account password in clear text (Q7 = A policy — module docs
/// spell out the rationale). Treat the resulting file as a
/// password backup.
///
/// # Errors
///
/// - `storage.*` (DPAPI / registry / IO) — failure during the
///   `Users.dat` decrypt step.
/// - `storage.export_write_failed` — external file I/O failure
///   while writing to `path`. Details include `path` and `io_kind`.
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn export_records(state: State<'_, AppState>, path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::export_records_impl(&state, path).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, path);
        Err(platform_unsupported_error())
    }
}

/// AES-128-CBC backup of the current `Users.dat` contents under a
/// user-supplied `password`. Returns the base64 ciphertext directly
/// (no file IO — the frontend `windows/AccRecovery.vue` shows it
/// in a textarea so users can copy/paste it to their preferred
/// transport channel).
///
/// Wire format is byte-for-byte compatible with WPF
/// `Beanfun/Windows/AccRecovery.xaml.cs::Export_Button_Click` so
/// users migrating between launchers can copy backups in either
/// direction. See [`crate::services::storage::aes_backup`] for the
/// crypto specification and threat-model caveats.
///
/// # Errors
///
/// - `storage.*` (DPAPI / registry / IO) — failure during the
///   `Users.dat` decrypt step.
/// - `storage.platform_unsupported` — non-Windows build.
///
/// AES encryption itself cannot fail on the happy path (key/IV
/// derivation is infallible and `cbc::Encryptor` returns `Vec<u8>`
/// rather than `Result`).
#[tauri::command]
#[specta::specta]
pub async fn backup_export(
    state: State<'_, AppState>,
    password: String,
) -> Result<String, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::backup_export_impl(&state, password).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, password);
        Err(platform_unsupported_error())
    }
}

/// Inverse of [`backup_export`]: AES-decrypt the user-supplied
/// `ciphertext_b64` under `password`, validate the recovered JSON
/// against the `WireRecords` shape, and overwrite `Users.dat`.
/// Returns the post-restore account list so the frontend can
/// refresh without a follow-up [`load_accounts`] call.
///
/// Mirrors WPF `AccRecovery.Recovery_Button_Click`.
///
/// # Errors
///
/// - `storage.aes_backup_invalid_ciphertext` — `ciphertext_b64`
///   is not valid base64. Frontend maps to WPF `MsgDecryptFailed`
///   toast.
/// - `storage.aes_backup_decrypt_failed` — AES-CBC PKCS7 unpad
///   failure (almost always wrong password). Frontend maps to WPF
///   `MsgDecryptFailed` toast.
/// - `storage.aes_backup_invalid_utf8` — decrypted bytes are not
///   valid UTF-8 (rare wrong-password symptom). Frontend maps to
///   WPF `MsgDecryptFailed` toast.
/// - `storage.json_failed` — decryption succeeded but the recovered
///   plaintext is not a valid `WireRecords` JSON. Frontend maps to
///   WPF `RecoveryFailed` toast.
/// - `storage.*` (DPAPI / registry / IO) — failure during the
///   `Users.dat` re-encrypt + overwrite step. Frontend maps to WPF
///   `RecoveryFailed` toast.
/// - `storage.platform_unsupported` — non-Windows build.
#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    state: State<'_, AppState>,
    password: String,
    ciphertext: String,
) -> Result<Vec<Account>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        imp::backup_restore_impl(&state, password, ciphertext).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, password, ciphertext);
        Err(platform_unsupported_error())
    }
}

// =====================================================================
// Command-layer symbol tests — the #[tauri::command] /
// #[specta::specta] attribute wiring is exercised by D6's bindings
// file test; this module focuses on pure helpers (the upsert
// logic is tested inside `imp` on Windows).
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_unsupported_code_is_stable() {
        // Frontend branches on `storage.platform_unsupported` to
        // show the "Windows-only feature" affordance; pinning the
        // string prevents a rename from silently breaking the
        // bindings contract.
        assert_eq!(PLATFORM_UNSUPPORTED_CODE, "storage.platform_unsupported");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn platform_unsupported_error_carries_stable_code_and_message() {
        let err = platform_unsupported_error();
        assert_eq!(err.code, "storage.platform_unsupported");
        assert!(err.message.contains("Windows"));
    }

    // ---------------------------------------------------------------
    // Serialisation round-trip for the command-exposed row shape.
    // Guards the Q7 = A wire contract: `Account` crosses IPC as a
    // row object with every field visible (including `password`).
    // If a future refactor adds `#[serde(skip)]` on a field, this
    // test fails loudly rather than silently breaking the frontend.
    // ---------------------------------------------------------------

    #[test]
    fn account_serde_round_trip_preserves_every_field() {
        let original = Account {
            region: "TW".into(),
            account_id: "u1".into(),
            account_name: "Alice".into(),
            password: "plaintext_pw".into(),
            verify: "vtoken".into(),
            method: 2,
            auto_login: true,
            last_login_at: None,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        assert!(json.contains("plaintext_pw"), "password must pass through");
        assert!(json.contains("vtoken"));
        assert!(json.contains("auto_login"));
        let decoded: Account = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }
}
