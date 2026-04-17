//! `Users.dat` — encrypted account record store ported from WPF
//! `Beanfun/Helper/AccountManager.cs`.
//!
//! # On-disk layout
//!
//! `%APPDATA%\Beanfun\Users.dat` is a single file containing only the
//! raw cipher bytes returned by
//! `CryptProtectData(plaintext, entropy, CurrentUser)`. There is no
//! length header, magic, or framing — WPF `writeRawData` writes via
//! `BinaryWriter.Write(byte[])` which (for the `byte[]` overload, not
//! the `string` overload) emits the buffer verbatim.
//!
//! The plaintext, after DPAPI unwrap, is UTF-8 JSON in the
//! parallel-columns shape captured by [`WireRecords`].
//!
//! # Save flow ([`save_records`])
//!
//! 1. [`Records`] → [`WireRecords`] → [`WireRecords::normalize`]
//!    (matches WPF `accRecInit` — every list becomes `Some(_)` of
//!    length `account_list.len()` with appropriate defaults).
//! 2. `serde_json::to_string` → UTF-8 plaintext.
//! 3. Fresh [`Entropy::generate`] → entropy persisted via
//!    [`super::entropy::write_to_registry`]
//!    (`HKCU\SOFTWARE\BEANFUN\ENTROPY`).
//! 4. [`dpapi_protect`] (`CurrentUser` scope, entropy as
//!    `pOptionalEntropy`).
//! 5. `mkdir_p` parent + `std::fs::write(path, cipher)`
//!    (`FileMode.Create` semantics — overwrite).
//!
//! Steps 3-5 run inside `tokio::task::spawn_blocking`.
//!
//! ## Deviation from WPF — strict registry-write handling
//!
//! WPF `writeRawData` ignores the `bool` return value of
//! `ModifyRegistry.Write("Entropy", entropy)`: if the registry write
//! fails, it still goes on to write the DPAPI ciphertext with an
//! entropy that was never persisted, so the next load cannot
//! decrypt and the file gets deleted on first read — silent data
//! loss.
//!
//! We `?`-propagate registry failure as [`StorageError::Registry`]
//! before touching the ciphertext file, so a registry-write error
//! surfaces loudly and the existing `Users.dat` (and its still-
//! valid previous entropy) are left intact. This is a corrected
//! port rather than a behaviour change — WPF callers that hit this
//! code path would have lost data regardless.
//!
//! # Load flow ([`load_records`])
//!
//! WPF `readRawData` (lines 215-229) wraps DPAPI / registry / UTF-8
//! decode in a single catch-all `try { ... } catch { File.Delete; }`
//! block — every failure mode means the file is unreadable to the
//! current user and gets deleted before falling back to a first-time
//! run with an empty record list.
//!
//! We mirror that catch-all internally and expose:
//!
//! 1. File missing → `Ok(Records::default())`, no delete.
//! 2. Read / DPAPI unprotect / UTF-8 decode / JSON parse failures →
//!    log warn, **delete** the file, return `Ok(Records::default())`.
//! 3. JSON parses → normalize → return `Ok(Records)`.
//! 4. JSON parse fails (caught at step 2) — *but only* if the
//!    plaintext also parses as valid base64 — return
//!    [`StorageError::LegacyDataDetected`] **before** deleting the
//!    file, so the P6 NRBF migrator can take it from here.
//!
//! Note the asymmetry: a **valid base64 + JSON parse failure** does
//! **not** delete the file (preserving legacy data for migration);
//! every other failure mode does delete it.
//!
//! # Caller responsibility for `LegacyDataDetected`
//!
//! P5 itself ships no NRBF parser. Callers that receive
//! [`StorageError::LegacyDataDetected`] **must**:
//!
//! 1. Try the P6 NRBF migrator on `raw_bytes`.
//! 2. If migration **succeeds** → call [`save_records`] to overwrite
//!    `Users.dat` with the JSON wire format (matches WPF
//!    `TryAutoMigrateLegacyData` L526-528).
//! 3. If migration **fails** → return an empty [`Records`] to the UI,
//!    log warn, and **do not delete** the file (matches WPF L546-548).
//!
//! Until P6 ships, callers should treat `LegacyDataDetected` as
//! "return empty Records, preserve file" — equivalent to the
//! migration-failure branch above.

// `WireRecords` is `pub(crate)` (internal implementation detail) but
// referenced from public `Records` / `parse_records` / module docs to
// explain the wire shape — these intra-doc links to a non-pub item are
// intentional and well-defined within this crate.
#![allow(rustdoc::private_intra_doc_links)]

use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};

use super::error::StorageError;

#[cfg(target_os = "windows")]
use super::{
    dpapi::{dpapi_protect, dpapi_unprotect},
    entropy::{
        read_from_registry_at, write_to_registry_at, Entropy, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME,
    },
};

// =====================================================================
// D1 — Public types
// =====================================================================

/// One persisted account row. Mirrors the i-th element across WPF
/// `Records`'s seven parallel `List<...>` fields
/// (`Beanfun/Helper/AccountManager.cs` L34-43).
///
/// `region` defaults to `"TW"` and `method` to `0` to match the WPF
/// `accRecInit` defaults; everything else defaults to its type's
/// natural zero value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// e.g. `"TW"` / `"HK"`. Defaults to `"TW"` per WPF `accRecInit`.
    pub region: String,
    /// Login account / member ID — `accountList[i]` in WPF.
    pub account_id: String,
    /// User-assigned display name — `accountNameList[i]` in WPF.
    pub account_name: String,
    /// Saved password — `passwdList[i]` in WPF.
    pub password: String,
    /// Verify token (HK-only) — `verifyList[i]` in WPF.
    pub verify: String,
    /// Login method enum value (id/pass / QR / GamePass / TOTP) —
    /// `methodList[i]` in WPF; defaults to `0`.
    pub method: i32,
    /// Auto-login flag — `autoLoginList[i]` in WPF; defaults to false.
    pub auto_login: bool,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            region: "TW".to_string(),
            account_id: String::new(),
            account_name: String::new(),
            password: String::new(),
            verify: String::new(),
            method: 0,
            auto_login: false,
        }
    }
}

/// In-memory record store. The on-disk wire format is parallel
/// columns (see [`WireRecords`]); this struct is the row-shaped
/// representation that the rest of the app uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records(pub Vec<Account>);

// =====================================================================
// D2 + D3 — Wire format adapter + normalize
// =====================================================================

/// Parallel-columns JSON wire format — byte-for-byte interop with WPF
/// `Records` (`Beanfun/Helper/AccountManager.cs` L34-43). All seven
/// fields are `Option<Vec<...>>` to tolerate the WPF default
/// `JsonConvert.SerializeObject` output, which writes `null` for
/// uninitialised lists.
///
/// Field names are hand-rolled `#[serde(rename = "...")]` rather
/// than a blanket `rename_all = "camelCase"` because the WPF
/// `passwdList` is not standard camel case (`password` would be
/// expected; `passwd` is a historical artefact preserved for
/// byte-for-byte interop).
///
/// Internal-only — callers should always go through [`Records`].
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireRecords {
    #[serde(rename = "regionList", default)]
    region_list: Option<Vec<String>>,
    #[serde(rename = "accountList", default)]
    account_list: Option<Vec<String>>,
    #[serde(rename = "accountNameList", default)]
    account_name_list: Option<Vec<String>>,
    #[serde(rename = "passwdList", default)]
    passwd_list: Option<Vec<String>>,
    #[serde(rename = "verifyList", default)]
    verify_list: Option<Vec<String>>,
    #[serde(rename = "methodList", default)]
    method_list: Option<Vec<i32>>,
    #[serde(rename = "autoLoginList", default)]
    auto_login_list: Option<Vec<bool>>,
}

impl WireRecords {
    /// Normalize the wire shape — every `Option` becomes
    /// `Some(_)` and every list is padded out to `account_list.len()`
    /// with the appropriate WPF default value.
    ///
    /// Equivalent to WPF `AccountManager.accRecInit()`
    /// (`Beanfun/Helper/AccountManager.cs` L79-170):
    ///
    /// | Field             | Default  |
    /// |-------------------|----------|
    /// | `region`          | `"TW"`   |
    /// | `account_name`    | `""`     |
    /// | `password`        | `""`     |
    /// | `verify`          | `""`     |
    /// | `method`          | `0`      |
    /// | `auto_login`      | `false`  |
    ///
    /// `account_list` itself is the canonical length source — every
    /// other list is padded *up to* `account_list.len()` (WPF only
    /// pads upward, never truncates downward; we follow that exactly).
    pub(crate) fn normalize(mut self) -> Self {
        let account_list = self.account_list.get_or_insert_with(Vec::new);
        let n = account_list.len();

        pad(self.region_list.get_or_insert_with(Vec::new), n, || {
            "TW".to_string()
        });
        pad(
            self.account_name_list.get_or_insert_with(Vec::new),
            n,
            String::new,
        );
        pad(
            self.passwd_list.get_or_insert_with(Vec::new),
            n,
            String::new,
        );
        pad(
            self.verify_list.get_or_insert_with(Vec::new),
            n,
            String::new,
        );
        pad(self.method_list.get_or_insert_with(Vec::new), n, || 0);
        pad(self.auto_login_list.get_or_insert_with(Vec::new), n, || {
            false
        });

        self
    }
}

/// Pad `list` upwards to `target` length using `default` for new
/// elements. Idempotent on already-aligned lists; never truncates.
/// (WPF `accRecInit` does the same — only grows lists.)
fn pad<T, F: FnMut() -> T>(list: &mut Vec<T>, target: usize, mut default: F) {
    while list.len() < target {
        list.push(default());
    }
}

impl From<&Records> for WireRecords {
    /// Split row-shaped [`Records`] into parallel columns.
    fn from(records: &Records) -> Self {
        let n = records.0.len();
        let mut region_list = Vec::with_capacity(n);
        let mut account_list = Vec::with_capacity(n);
        let mut account_name_list = Vec::with_capacity(n);
        let mut passwd_list = Vec::with_capacity(n);
        let mut verify_list = Vec::with_capacity(n);
        let mut method_list = Vec::with_capacity(n);
        let mut auto_login_list = Vec::with_capacity(n);

        for acc in &records.0 {
            region_list.push(acc.region.clone());
            account_list.push(acc.account_id.clone());
            account_name_list.push(acc.account_name.clone());
            passwd_list.push(acc.password.clone());
            verify_list.push(acc.verify.clone());
            method_list.push(acc.method);
            auto_login_list.push(acc.auto_login);
        }

        WireRecords {
            region_list: Some(region_list),
            account_list: Some(account_list),
            account_name_list: Some(account_name_list),
            passwd_list: Some(passwd_list),
            verify_list: Some(verify_list),
            method_list: Some(method_list),
            auto_login_list: Some(auto_login_list),
        }
    }
}

impl From<WireRecords> for Records {
    /// Zip parallel columns into row-shaped [`Records`].
    ///
    /// [`WireRecords::normalize`] is called internally so callers
    /// never need to do it themselves. Any column **longer** than
    /// `account_list` is preserved on the wire side but its surplus
    /// entries do not appear in the resulting [`Records`] — that
    /// matches WPF `accRecInit` + `Records.regionList[i]` access
    /// pattern which iterates `0..accountList.Count` and ignores
    /// trailing surplus.
    fn from(wire: WireRecords) -> Self {
        let wire = wire.normalize();

        let region_list = wire.region_list.unwrap_or_default();
        let account_list = wire.account_list.unwrap_or_default();
        let account_name_list = wire.account_name_list.unwrap_or_default();
        let passwd_list = wire.passwd_list.unwrap_or_default();
        let verify_list = wire.verify_list.unwrap_or_default();
        let method_list = wire.method_list.unwrap_or_default();
        let auto_login_list = wire.auto_login_list.unwrap_or_default();

        let n = account_list.len();
        let mut accounts = Vec::with_capacity(n);
        for i in 0..n {
            accounts.push(Account {
                region: region_list[i].clone(),
                account_id: account_list[i].clone(),
                account_name: account_name_list[i].clone(),
                password: passwd_list[i].clone(),
                verify: verify_list[i].clone(),
                method: method_list[i],
                auto_login: auto_login_list[i],
            });
        }

        Records(accounts)
    }
}

// =====================================================================
// D7 — Pure parsers / serializers (cross-platform)
// =====================================================================

/// Parse a JSON blob produced by [`export_records`] (or by WPF
/// `JsonConvert.SerializeObject(accountRecords)`) into [`Records`].
///
/// Returns [`StorageError::Json`] on malformed JSON. Note that
/// successfully parsed JSON with missing or `null` fields yields an
/// empty [`Records`] (matches WPF `accRecInit` initialising every
/// list to empty when the deserialized object had `null`s).
pub fn parse_records(json: &str) -> Result<Records, StorageError> {
    let wire: WireRecords = serde_json::from_str(json).map_err(StorageError::Json)?;
    Ok(Records::from(wire))
}

/// Serialize [`Records`] into the parallel-columns JSON wire format
/// (matches WPF `JsonConvert.SerializeObject(accountRecords)`).
///
/// Returns [`StorageError::Json`] on the (effectively unreachable)
/// case where `serde_json::to_string` fails on a `WireRecords`
/// containing only `String` / `i32` / `bool` / `Vec` types.
pub fn export_records(records: &Records) -> Result<String, StorageError> {
    // `From<&Records>` already produces an aligned `WireRecords`
    // (every list `Some(_)` of length `records.0.len()`), so an
    // explicit `.normalize()` here would be a no-op.
    let wire = WireRecords::from(records);
    serde_json::to_string(&wire).map_err(StorageError::Json)
}

// =====================================================================
// D8 — Default path helper (Windows-only)
// =====================================================================

/// Resolve `%APPDATA%\Beanfun\Users.dat` — the production path WPF
/// `AccountManager` uses (`SpecialFolder.ApplicationData` →
/// `Roaming`).
///
/// Returns [`StorageError::AppDataMissing`] when the `APPDATA`
/// environment variable is unset or empty (the OS should always set
/// it on a normal Windows session; this only fails in unusual
/// sandbox contexts). The typed variant is shared with the sibling
/// [`crate::services::config::default_config_xml_path`] so both
/// default-path helpers surface env-var failure uniformly.
#[cfg(target_os = "windows")]
pub fn default_users_dat_path() -> Result<PathBuf, StorageError> {
    let appdata = std::env::var_os("APPDATA")
        .filter(|s| !s.is_empty())
        .ok_or(StorageError::AppDataMissing)?;
    Ok(PathBuf::from(appdata).join("Beanfun").join("Users.dat"))
}

// =====================================================================
// D5 + D6 + D7 — IO-bearing async APIs (Windows-only)
// =====================================================================

/// Persist [`Records`] to `path` — see module docs for the full save
/// flow. Always overwrites (no atomic-rename guarantees), matching
/// WPF `BinaryWriter` + `FileMode.Create`.
///
/// Generates a fresh entropy on every call; the previous registry
/// entropy value at `HKCU\SOFTWARE\BEANFUN\ENTROPY` is overwritten.
/// Existing `Users.dat` files become unreadable after this returns
/// until they too are overwritten by the next save (matches WPF
/// behaviour).
#[cfg(target_os = "windows")]
pub async fn save_records(path: &Path, records: &Records) -> Result<(), StorageError> {
    save_records_at(path, records, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME).await
}

/// Lower-level variant that targets an arbitrary registry sub-key /
/// value name for the entropy salt. Exposed publicly for integration
/// tests so they can avoid polluting the production
/// `SOFTWARE\BEANFUN\ENTROPY` location (and the production
/// `Users.dat` cipher that depends on it).
///
/// Production callers should prefer [`save_records`].
#[cfg(target_os = "windows")]
pub async fn save_records_at(
    path: &Path,
    records: &Records,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<(), StorageError> {
    let path = path.to_path_buf();
    let plaintext = export_records(records)?;
    let subkey = entropy_subkey.to_string();
    let value_name = entropy_value_name.to_string();
    spawn_blocking_storage(move || save_records_blocking(&path, &plaintext, &subkey, &value_name))
        .await
}

#[cfg(target_os = "windows")]
fn save_records_blocking(
    path: &Path,
    plaintext: &str,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<(), StorageError> {
    let entropy = Entropy::generate();
    write_to_registry_at(entropy_subkey, entropy_value_name, &entropy)?;
    let cipher = dpapi_protect(plaintext.as_bytes(), entropy.as_bytes())?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }
    }
    std::fs::write(path, &cipher).map_err(StorageError::Io)?;
    Ok(())
}

/// Load [`Records`] from `path` — see module docs for the full load
/// flow including the catch-all `delete-on-decrypt-failure` and the
/// `LegacyDataDetected` branch.
///
/// **Never returns DPAPI / registry / UTF-8 / plain JSON parse
/// failures as typed errors** — they are caught internally and
/// translated into either `Ok(Records::default())` (with the file
/// deleted) or [`StorageError::LegacyDataDetected`] (with the file
/// preserved). The only typed errors returned are
/// [`StorageError::Io`] for an actual file-read I/O failure that
/// could not be classified as "corrupted ciphertext", and
/// [`StorageError::LegacyDataDetected`] itself.
#[cfg(target_os = "windows")]
pub async fn load_records(path: &Path) -> Result<Records, StorageError> {
    load_records_at(path, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME).await
}

/// Lower-level variant — see [`save_records_at`] for rationale.
#[cfg(target_os = "windows")]
pub async fn load_records_at(
    path: &Path,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<Records, StorageError> {
    let path = path.to_path_buf();
    let subkey = entropy_subkey.to_string();
    let value_name = entropy_value_name.to_string();
    spawn_blocking_storage(move || load_records_blocking(&path, &subkey, &value_name)).await
}

#[cfg(target_os = "windows")]
fn load_records_blocking(
    path: &Path,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<Records, StorageError> {
    // Single syscall instead of `path.exists()` + `std::fs::read`:
    // the separate exists-check is a TOCTOU hazard (file could be
    // unlinked between the two calls) and `read` already encodes
    // "file missing" as `ErrorKind::NotFound`. Matches the terminal
    // state of WPF `readRawData` when `File.Exists` races against
    // another process deleting `Users.dat` — both settle on "empty
    // records, no throw" without bothering to delete a file that is
    // already gone.
    let cipher = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Records::default());
        }
        Err(err) => return Err(StorageError::Io(err)),
    };

    // Catch-all matching WPF `readRawData` lines 215-229: any failure
    // in registry read / DPAPI unprotect / UTF-8 decode means the
    // file is unreadable to the current user; delete it and behave
    // like a first-time run.
    let plaintext = match decrypt_users_dat(&cipher, entropy_subkey, entropy_value_name) {
        Ok(plain) => plain,
        Err(err) => {
            tracing::warn!(error = %err, "Users.dat decrypt failed; deleting and starting fresh");
            let _ = std::fs::remove_file(path);
            return Ok(Records::default());
        }
    };

    match parse_records(&plaintext) {
        Ok(records) => Ok(records),
        Err(_) => fall_back_to_legacy_or_empty(plaintext),
    }
}

#[cfg(target_os = "windows")]
fn decrypt_users_dat(
    cipher: &[u8],
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<String, StorageError> {
    let entropy = read_from_registry_at(entropy_subkey, entropy_value_name)?;
    let plain_bytes = dpapi_unprotect(cipher, entropy.as_bytes())?;
    String::from_utf8(plain_bytes).map_err(|err| {
        StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Users.dat plaintext is not valid UTF-8: {err}"),
        ))
    })
}

/// JSON parse failed; try base64 to detect legacy NRBF wire format.
///
/// - base64 OK → return [`StorageError::LegacyDataDetected`] for the
///   P6 migrator (file preserved by caller).
/// - base64 fail → log warn, return `Ok(Records::default())` (file
///   preserved; matches WPF L494-550 — the file is *not* deleted on
///   pure-parse failure, allowing the user to recover by themselves).
fn fall_back_to_legacy_or_empty(plaintext: String) -> Result<Records, StorageError> {
    match BASE64.decode(plaintext.as_bytes()) {
        Ok(raw_bytes) => Err(StorageError::LegacyDataDetected { raw_bytes }),
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Users.dat plaintext is neither JSON nor base64; preserving file and returning empty records"
            );
            Ok(Records::default())
        }
    }
}

/// Import a JSON blob into `path` — corresponds to WPF
/// `AccountManager.importRecord(string raw)`
/// (`Beanfun/Helper/AccountManager.cs` L469-483).
///
/// Flow:
///
/// 1. `parse_records(json)` succeeds → write through [`save_records`]
///    and return the parsed [`Records`] (matches WPF L473-475).
/// 2. `parse_records` fails → try `BASE64.decode(json)`:
///    - success → [`StorageError::LegacyDataDetected`] (file is
///      **not** touched; caller's NRBF migrator may recover).
///    - failure → return the original [`StorageError::Json`] so the
///      UI can surface "import failed" to the user.
#[cfg(target_os = "windows")]
pub async fn import_records(path: &Path, json: &str) -> Result<Records, StorageError> {
    import_records_at(path, json, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME).await
}

/// Lower-level variant — see [`save_records_at`] for rationale.
#[cfg(target_os = "windows")]
pub async fn import_records_at(
    path: &Path,
    json: &str,
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<Records, StorageError> {
    match parse_records(json) {
        Ok(records) => {
            save_records_at(path, &records, entropy_subkey, entropy_value_name).await?;
            Ok(records)
        }
        Err(json_err) => match BASE64.decode(json.as_bytes()) {
            Ok(raw_bytes) => Err(StorageError::LegacyDataDetected { raw_bytes }),
            Err(_) => Err(json_err),
        },
    }
}

// =====================================================================
// Internal — blocking helpers
// =====================================================================

/// Run a blocking storage closure on the tokio blocking pool, mapping
/// `JoinError` (panic / cancel) into [`StorageError::Io`].
#[cfg(target_os = "windows")]
async fn spawn_blocking_storage<F, R>(f: F) -> Result<R, StorageError>
where
    F: FnOnce() -> Result<R, StorageError> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|join_err| {
        StorageError::Io(std::io::Error::other(format!(
            "blocking storage task panicked: {join_err}"
        )))
    })?
}

// =====================================================================
// D10 — Unit tests (cross-platform pure logic)
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample_account(region: &str, id: &str) -> Account {
        Account {
            region: region.to_string(),
            account_id: id.to_string(),
            account_name: format!("{id}-display"),
            password: format!("{id}-pwd"),
            verify: format!("{id}-vrf"),
            method: 1,
            auto_login: true,
        }
    }

    // ---- D1: Account / Records defaults --------------------------------

    #[test]
    fn account_default_matches_wpf_acc_rec_init_per_row_defaults() {
        // WPF `accRecInit` defaults: region "TW", others "" / 0 / false.
        let acc = Account::default();
        assert_eq!(acc.region, "TW");
        assert_eq!(acc.account_id, "");
        assert_eq!(acc.account_name, "");
        assert_eq!(acc.password, "");
        assert_eq!(acc.verify, "");
        assert_eq!(acc.method, 0);
        assert!(!acc.auto_login);
    }

    #[test]
    fn records_default_is_empty() {
        let r = Records::default();
        assert!(r.0.is_empty());
    }

    // ---- D2: WireRecords round-trip -----------------------------------

    #[test]
    fn wire_records_round_trips_through_records_two_rows() {
        let records = Records(vec![
            sample_account("TW", "alice"),
            sample_account("HK", "bob"),
        ]);

        let wire = WireRecords::from(&records);
        let back = Records::from(wire);
        assert_eq!(back, records);
    }

    #[test]
    fn wire_records_empty_round_trip() {
        let records = Records::default();
        let wire = WireRecords::from(&records);
        let back = Records::from(wire);
        assert_eq!(back, Records::default());
    }

    // ---- D3: WireRecords::normalize ------------------------------------

    #[test]
    fn normalize_pads_short_lists_to_account_list_length_with_wpf_defaults() {
        let wire = WireRecords {
            region_list: None,
            account_list: Some(vec!["a".into(), "b".into(), "c".into()]),
            account_name_list: Some(vec!["a-name".into()]),
            passwd_list: None,
            verify_list: Some(vec![]),
            method_list: Some(vec![5]),
            auto_login_list: None,
        };

        let n = wire.normalize();
        let three_tw = vec!["TW".to_string(), "TW".to_string(), "TW".to_string()];
        assert_eq!(n.region_list.as_deref(), Some(three_tw.as_slice()));
        let three_acc = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(n.account_list.as_deref(), Some(three_acc.as_slice()));
        let three_names = vec!["a-name".to_string(), String::new(), String::new()];
        assert_eq!(n.account_name_list.as_deref(), Some(three_names.as_slice()));
        let three_empty = vec![String::new(), String::new(), String::new()];
        assert_eq!(n.passwd_list.as_deref(), Some(three_empty.as_slice()));
        assert_eq!(n.verify_list.as_deref(), Some(three_empty.as_slice()));
        assert_eq!(n.method_list.as_deref(), Some(&[5, 0, 0][..]));
        assert_eq!(
            n.auto_login_list.as_deref(),
            Some(&[false, false, false][..])
        );
    }

    #[test]
    fn normalize_does_not_truncate_already_long_lists() {
        // WPF only pads upward; if e.g. region_list already has more
        // entries than account_list (shouldn't happen, but the WPF
        // code wouldn't truncate either), we must not lose data.
        let wire = WireRecords {
            region_list: Some(vec!["TW".into(), "HK".into(), "JP".into()]),
            account_list: Some(vec!["a".into()]),
            account_name_list: None,
            passwd_list: None,
            verify_list: None,
            method_list: None,
            auto_login_list: None,
        };
        let n = wire.normalize();
        assert_eq!(n.region_list.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn normalize_initialises_all_none_to_some_empty_vec() {
        let wire = WireRecords::default();
        let n = wire.normalize();
        let empty_str: &[String] = &[];
        let empty_i32: &[i32] = &[];
        let empty_bool: &[bool] = &[];
        assert_eq!(n.region_list.as_deref(), Some(empty_str));
        assert_eq!(n.account_list.as_deref(), Some(empty_str));
        assert_eq!(n.account_name_list.as_deref(), Some(empty_str));
        assert_eq!(n.passwd_list.as_deref(), Some(empty_str));
        assert_eq!(n.verify_list.as_deref(), Some(empty_str));
        assert_eq!(n.method_list.as_deref(), Some(empty_i32));
        assert_eq!(n.auto_login_list.as_deref(), Some(empty_bool));
    }

    #[test]
    fn normalize_is_idempotent_on_aligned_input() {
        let wire = WireRecords {
            region_list: Some(vec!["TW".into()]),
            account_list: Some(vec!["a".into()]),
            account_name_list: Some(vec!["a-display".into()]),
            passwd_list: Some(vec!["pw".into()]),
            verify_list: Some(vec!["v".into()]),
            method_list: Some(vec![3]),
            auto_login_list: Some(vec![true]),
        };
        let once = wire.normalize();
        let twice = once.clone().normalize();
        assert_eq!(once, twice);
    }

    // ---- D7: parse_records / export_records ----------------------------

    #[test]
    fn parse_records_accepts_wpf_fixture_json() {
        // Hand-rolled to mirror what WPF
        // `JsonConvert.SerializeObject(Records)` would emit for two
        // accounts (one TW id/pass, one HK QR with verify token).
        let json = r#"{
            "regionList":["TW","HK"],
            "accountList":["alice","bob"],
            "accountNameList":["A","B"],
            "passwdList":["pw-a","pw-b"],
            "verifyList":["","vrfb"],
            "methodList":[1,2],
            "autoLoginList":[true,false]
        }"#;

        let records = parse_records(json).expect("parse WPF fixture");
        assert_eq!(records.0.len(), 2);
        assert_eq!(records.0[0].region, "TW");
        assert_eq!(records.0[0].account_id, "alice");
        assert_eq!(records.0[0].account_name, "A");
        assert_eq!(records.0[0].method, 1);
        assert!(records.0[0].auto_login);
        assert_eq!(records.0[1].region, "HK");
        assert_eq!(records.0[1].verify, "vrfb");
        assert_eq!(records.0[1].method, 2);
        assert!(!records.0[1].auto_login);
    }

    #[test]
    fn parse_records_treats_empty_object_as_default() {
        let records = parse_records("{}").expect("parse empty object");
        assert_eq!(records, Records::default());
    }

    #[test]
    fn parse_records_treats_all_null_fields_as_default() {
        let json = r#"{
            "regionList":null,
            "accountList":null,
            "accountNameList":null,
            "passwdList":null,
            "verifyList":null,
            "methodList":null,
            "autoLoginList":null
        }"#;
        let records = parse_records(json).expect("parse all-null object");
        assert_eq!(records, Records::default());
    }

    #[test]
    fn parse_records_propagates_json_error_on_malformed_input() {
        let err = parse_records("not json at all").expect_err("malformed JSON");
        assert!(matches!(err, StorageError::Json(_)));
    }

    #[test]
    fn export_records_round_trips_through_parse_records() {
        let original = Records(vec![
            sample_account("TW", "alice"),
            sample_account("HK", "bob"),
        ]);
        let json = export_records(&original).expect("export");
        let back = parse_records(&json).expect("parse");
        assert_eq!(back, original);
    }

    #[test]
    fn export_records_emits_method_as_json_int_and_auto_login_as_json_bool() {
        // WPF wire is `methodList: List<int>` and `autoLoginList:
        // List<bool>` — guard against an accidental serde rename to
        // String that would break byte-for-byte interop.
        let records = Records(vec![Account {
            method: 2,
            auto_login: true,
            ..sample_account("TW", "alice")
        }]);
        let json = export_records(&records).expect("export");
        assert!(
            json.contains("\"methodList\":[2]"),
            "method must serialize as JSON integer, got {json}"
        );
        assert!(
            json.contains("\"autoLoginList\":[true]"),
            "auto_login must serialize as JSON boolean, got {json}"
        );
    }

    #[test]
    fn export_records_for_empty_uses_seven_camel_cased_keys_with_empty_arrays() {
        let json = export_records(&Records::default()).expect("export empty");
        // Spot-check the camel-cased field names present after
        // normalize fills every Option to Some(empty Vec).
        assert!(json.contains("\"regionList\":[]"));
        assert!(json.contains("\"accountList\":[]"));
        assert!(json.contains("\"accountNameList\":[]"));
        assert!(json.contains("\"passwdList\":[]"));
        assert!(json.contains("\"verifyList\":[]"));
        assert!(json.contains("\"methodList\":[]"));
        assert!(json.contains("\"autoLoginList\":[]"));
    }
}
