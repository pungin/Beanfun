//! Pure + I/O-bound migrator from legacy NRBF `Users.dat` payloads
//! to the modern JSON format.
//!
//! - [`migrate_legacy_payload`] (cross-platform, pure) — bytes →
//!   [`Records`] via the P6 chunk 6.1 NRBF parser + the P5
//!   `WireRecords::normalize` pipeline.
//! - [`migrate_and_save`] / [`migrate_and_save_at`] (Windows-only)
//!   — the above, then overwrite `Users.dat` with the JSON cipher
//!   via [`crate::services::storage::save_records`]. This aligns
//!   with WPF `AccountManager.TryAutoMigrateLegacyData` L526
//!   `storeRecord()` — the user sees one migration, never a
//!   second-boot upgrade.

use crate::core::legacy::{parse_legacy_payload, LegacyPayload};

use super::super::users_dat::{records_from_wire_lists, Records};
use super::error::LegacyMigrateError;

#[cfg(target_os = "windows")]
use std::path::Path;

#[cfg(target_os = "windows")]
use super::super::entropy::{REGISTRY_SUBKEY, REGISTRY_VALUE_NAME};
#[cfg(target_os = "windows")]
use super::super::users_dat::save_records_at;

/// Parse NRBF `raw_bytes` + convert to [`Records`] without touching
/// disk. Cross-platform and pure — exposed publicly so P6 chunk 6.2
/// unit tests and higher-level callers that want to inspect the
/// converted records before persisting can avoid the save step.
///
/// Conversion rules (matches WPF `BinaryFormatter.Deserialize` →
/// `JsonConvert.SerializeObject` → `DeserializeObject<Records>` →
/// `accRecInit` pipeline, minus the double JSON round-trip):
///
/// | Legacy shape              | `account_name_list` input | Normalize fills with |
/// | ------------------------- | ------------------------- | -------------------- |
/// | `Beanfun.Records` (7)     | `Some(verbatim)`          | verbatim             |
/// | `Beanfun.AccountRecords` (6) | `None`                  | `""` × N             |
///
/// The legacy 6-field shape predates `accountNameList`; passing
/// `None` routes through the internal `records_from_wire_lists`
/// helper (in `crate::services::storage::users_dat`), which in turn
/// applies `WireRecords::normalize` and pads to `account_list.len()`
/// empty strings — exactly what WPF `accRecInit` does when
/// `JsonConvert` deserialises a missing field as `null`.
pub fn migrate_legacy_payload(raw_bytes: &[u8]) -> Result<Records, LegacyMigrateError> {
    let payload = parse_legacy_payload(raw_bytes)?;
    Ok(match payload {
        LegacyPayload::Records(r) => records_from_wire_lists(
            r.region_list,
            r.account_list,
            Some(r.account_name_list),
            r.passwd_list,
            r.verify_list,
            r.method_list,
            r.auto_login_list,
        ),
        LegacyPayload::AccountRecords(r) => records_from_wire_lists(
            r.region_list,
            r.account_list,
            None,
            r.passwd_list,
            r.verify_list,
            r.method_list,
            r.auto_login_list,
        ),
    })
}

/// Migrate + save in one call. Returns the migrated [`Records`] on
/// success; leaves `path` pointing to the freshly-written JSON
/// ciphertext so subsequent [`load_records`][ld] calls skip the
/// NRBF fallback entirely.
///
/// Uses the production entropy registry location
/// (`HKCU\SOFTWARE\BEANFUN\ENTROPY`); integration tests should call
/// [`migrate_and_save_at`] to isolate the registry surface.
///
/// [ld]: crate::services::storage::load_records
#[cfg(target_os = "windows")]
pub async fn migrate_and_save(
    path: &Path,
    raw_bytes: &[u8],
) -> Result<Records, LegacyMigrateError> {
    migrate_and_save_at(path, raw_bytes, REGISTRY_SUBKEY, REGISTRY_VALUE_NAME).await
}

/// Lower-level variant — see
/// [`crate::services::storage::save_records_at`] for the rationale
/// behind the `_at` split (test isolation of the registry entropy
/// location).
#[cfg(target_os = "windows")]
pub async fn migrate_and_save_at(
    path: &Path,
    raw_bytes: &[u8],
    entropy_subkey: &str,
    entropy_value_name: &str,
) -> Result<Records, LegacyMigrateError> {
    let records = migrate_legacy_payload(raw_bytes)?;
    save_records_at(path, &records, entropy_subkey, entropy_value_name).await?;
    tracing::info!(
        accounts = records.0.len(),
        "legacy Users.dat migrated to JSON format"
    );
    Ok(records)
}

// =====================================================================
// D7 — Unit tests (cross-platform pure logic)
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::legacy::nrbf::fixture::{build_root_class, MemberSpec};
    use pretty_assertions::assert_eq;

    const CLASS_RECORDS: &str = "Beanfun.Records";
    const CLASS_ACCOUNT_RECORDS: &str = "Beanfun.AccountRecords";

    #[test]
    fn migrate_new_records_shape_preserves_all_seven_fields() {
        // Full 7-field `Beanfun.Records` round-trip; every column
        // should arrive verbatim with no normalise-time surprises.
        let bytes = build_root_class(
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
        );

        let records = migrate_legacy_payload(&bytes).expect("migrate");
        assert_eq!(records.0.len(), 2);
        assert_eq!(records.0[0].region, "TW");
        assert_eq!(records.0[0].account_id, "alice");
        assert_eq!(records.0[0].account_name, "Alice");
        assert_eq!(records.0[0].password, "pw-a");
        assert_eq!(records.0[0].verify, "");
        assert_eq!(records.0[0].method, 1);
        assert!(records.0[0].auto_login);
        assert_eq!(records.0[1].region, "HK");
        assert_eq!(records.0[1].account_id, "bob");
        assert_eq!(records.0[1].account_name, "Bob");
        assert_eq!(records.0[1].method, 2);
        assert!(!records.0[1].auto_login);
    }

    #[test]
    fn migrate_legacy_account_records_pads_account_name_list_to_empty_strings() {
        // `Beanfun.AccountRecords` is the pre-`accountNameList` shape.
        // After migration, each row must appear with `account_name == ""`
        // (matches WPF `accRecInit` default for a `null` string list).
        let bytes = build_root_class(
            CLASS_ACCOUNT_RECORDS,
            &[
                (
                    "regionList",
                    MemberSpec::StringList(&[Some("TW"), Some("HK")]),
                ),
                (
                    "accountList",
                    MemberSpec::StringList(&[Some("legacy-a"), Some("legacy-b")]),
                ),
                (
                    "passwdList",
                    MemberSpec::StringList(&[Some("pw-a"), Some("pw-b")]),
                ),
                (
                    "verifyList",
                    MemberSpec::StringList(&[Some(""), Some("vrf-b")]),
                ),
                ("methodList", MemberSpec::I32List(&[0, 1])),
                ("autoLoginList", MemberSpec::BoolList(&[false, true])),
            ],
        );

        let records = migrate_legacy_payload(&bytes).expect("migrate");
        assert_eq!(records.0.len(), 2);
        assert_eq!(records.0[0].account_id, "legacy-a");
        assert_eq!(records.0[0].account_name, "");
        assert_eq!(records.0[1].account_id, "legacy-b");
        assert_eq!(records.0[1].account_name, "");
    }

    #[test]
    fn migrate_empty_lists_yields_default_records() {
        // `Records` with every list null → normalize bottoms out to
        // `account_list.len() == 0` which produces `Records::default()`.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                ("regionList", MemberSpec::NullStringList),
                ("accountList", MemberSpec::NullStringList),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );

        let records = migrate_legacy_payload(&bytes).expect("migrate");
        assert_eq!(records, Records::default());
    }

    #[test]
    fn migrate_short_lists_normalize_pads_up_to_account_list_length() {
        // account_list is authoritative (WPF accRecInit). If other
        // lists are shorter, normalize must pad them out.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                // Only 1 region, but 3 accounts → pads to ["TW", "TW", "TW"]
                ("regionList", MemberSpec::StringList(&[Some("TW")])),
                (
                    "accountList",
                    MemberSpec::StringList(&[Some("a"), Some("b"), Some("c")]),
                ),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::StringList(&[Some("pw-a")])),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::I32List(&[7])),
                ("autoLoginList", MemberSpec::BoolList(&[true])),
            ],
        );

        let records = migrate_legacy_payload(&bytes).expect("migrate");
        assert_eq!(records.0.len(), 3);
        // Region pads with "TW"; name / verify / passwd pad with "";
        // method pads with 0; auto_login pads with false.
        for acc in &records.0 {
            assert_eq!(acc.region, "TW");
        }
        assert_eq!(records.0[0].account_id, "a");
        assert_eq!(records.0[0].password, "pw-a");
        assert_eq!(records.0[0].method, 7);
        assert!(records.0[0].auto_login);
        assert_eq!(records.0[1].account_id, "b");
        assert_eq!(records.0[1].password, "");
        assert_eq!(records.0[1].method, 0);
        assert!(!records.0[1].auto_login);
        assert_eq!(records.0[2].account_id, "c");
    }

    #[test]
    fn legacy_migrate_error_display_formats_nrbf_and_storage_variants() {
        // Guard the human-readable message shape — UI logs / bug
        // reports depend on these prefixes.
        let nrbf_err = LegacyMigrateError::from(crate::core::legacy::NrbfError::UnsupportedClass {
            name: "Foo.Bar".to_string(),
        });
        let storage_err =
            LegacyMigrateError::from(super::super::super::error::StorageError::AppDataMissing);

        let nrbf_msg = format!("{nrbf_err}");
        assert!(
            nrbf_msg.starts_with("legacy Users.dat NRBF parse failed"),
            "unexpected nrbf display: {nrbf_msg}"
        );
        assert!(nrbf_msg.contains("Foo.Bar"));

        let storage_msg = format!("{storage_err}");
        assert!(
            storage_msg.starts_with("save-after-migrate failed"),
            "unexpected storage display: {storage_msg}"
        );
    }

    #[test]
    fn legacy_migrate_error_from_impl_wires_nrbf_and_storage() {
        // `From` impls exist so migrator code can `?`-propagate
        // either error without an explicit `.map_err`.
        fn takes_nrbf(e: crate::core::legacy::NrbfError) -> LegacyMigrateError {
            e.into()
        }
        fn takes_storage(e: super::super::super::error::StorageError) -> LegacyMigrateError {
            e.into()
        }

        let wrapped_nrbf = takes_nrbf(crate::core::legacy::NrbfError::MissingMember {
            class: "Beanfun.Records",
            member: "accountList",
        });
        assert!(matches!(wrapped_nrbf, LegacyMigrateError::Nrbf(_)));

        let wrapped_storage =
            takes_storage(super::super::super::error::StorageError::EntropyMissing);
        assert!(matches!(wrapped_storage, LegacyMigrateError::Storage(_)));
    }
}
