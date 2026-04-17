//! NRBF → [`LegacyPayload`] adapter (pure).
//!
//! Entry point: [`parse_legacy_payload`]. The upstream `nrbf` crate
//! is responsible for the actual binary-format spec (record types,
//! string encoding, length prefixes, library references); we only
//! walk the resulting [`nrbf::value::Object`] graph and translate
//! the handful of `.NET` shapes we care about into plain Rust
//! collections.
//!
//! # `.NET` `List<T>` NRBF layout
//!
//! [MS-NRBF] §2.3.2.1 `ClassWithMembersAndTypes` wraps
//! `List<T>` with this member table (member count varies by
//! runtime — see next paragraph):
//!
//! | Member      | Type        | Meaning                                                                                  |
//! | ----------- | ----------- | ---------------------------------------------------------------------------------------- |
//! | `_items`    | array (`T`) | Backing array. Capacity; only the first `_size` slots are live.                          |
//! | `_size`     | `Int32`     | Authoritative element count. WPF / .NET both read this, not `_items.len()`.              |
//! | `_version`  | `Int32`     | Mutation counter. Ignored.                                                               |
//! | `_syncRoot` | object/null | Lazily-created lock object. Optional; present on some runtimes, absent on others.        |
//!
//! Different .NET runtimes serialise `List<T>` with either 3 members
//! (`_items` + `_size` + `_version`, typical .NET Framework) or 4
//! members (adds `_syncRoot`, typical .NET Core). We walk
//! `Object.members` by key so both shapes round-trip identically.
//!
//! # `null` vs empty list semantics (alignment with WPF)
//!
//! The `List<string>` field itself may arrive as:
//!
//! - `Value::Null` — WPF never initialised the field. Treated as an
//!   empty list, matching WPF `AccountManager.accRecInit` which pads
//!   every `null` list to length 0 before use.
//! - `Value::Object` with non-`null` `_items` — normal case, see
//!   extraction above.
//! - `Value::Object` with `null` `_items` and `_size == 0` — observed
//!   on some `List<T>` default-constructed and never appended to.
//!   Treated as empty.
//! - `Value::Object` with `null` `_items` and `_size > 0` — malformed;
//!   raise [`NrbfError::InconsistentListSize`].
//!
//! Individual `List<string>` elements may also be `Value::Null`. WPF
//! flows them through `JsonConvert.SerializeObject` (null → JSON
//! `null`) then `DeserializeObject<Records>` (null → `string`
//! default, which is `null`) then `accRecInit` (null → `""`). We
//! short-circuit that chain and substitute `""` directly.
//!
//! # Why we refuse arbitrary root classes
//!
//! An attacker that can plant a file in `%APPDATA%\Beanfun\Users.dat`
//! could otherwise use our NRBF surface to smuggle `.NET` types
//! Rust has no notion of (and which `nrbf` does not execute, but
//! would still hand us as `Value::Object`). Gating the root class to
//! `Beanfun.Records` / `Beanfun.AccountRecords` keeps the attack
//! surface to the two shapes the port actually needs, mirroring the
//! "allow-list deserialisation" pattern recommended by the
//! [`NrbfDecoder`][nrbfdec] guidance.
//!
//! [MS-NRBF]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-nrbf/
//! [nrbfdec]: https://learn.microsoft.com/en-us/dotnet/standard/serialization/binaryformatter-migration-guide/read-nrbf-payloads

use nrbf::{value::Object, RemotingMessage, Value};

use super::error::NrbfError;

const CLASS_RECORDS: &str = "Beanfun.Records";
const CLASS_ACCOUNT_RECORDS: &str = "Beanfun.AccountRecords";
const LIST_CLASS_PREFIX: &str = "System.Collections.Generic.List";

/// The current WPF shape — `Beanfun.Records` with 7 parallel lists.
///
/// All lists should have identical lengths once
/// `services::storage::legacy` (chunk 6.2) normalises the payload,
/// but the raw legacy stream may well contain mismatched lengths
/// (WPF `accRecInit` re-paired them on load). We keep the raw list
/// lengths verbatim here; normalisation happens one layer up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyRecords {
    /// Corresponds to `regionList: List<string>` in C#.
    pub region_list: Vec<String>,
    /// Corresponds to `accountList: List<string>` in C#.
    pub account_list: Vec<String>,
    /// Corresponds to `accountNameList: List<string>` in C# (present
    /// only in the current `Records` shape; legacy `AccountRecords`
    /// did not have this field).
    pub account_name_list: Vec<String>,
    /// Corresponds to `passwdList: List<string>` in C#.
    pub passwd_list: Vec<String>,
    /// Corresponds to `verifyList: List<string>` in C#.
    pub verify_list: Vec<String>,
    /// Corresponds to `methodList: List<int>` in C#.
    pub method_list: Vec<i32>,
    /// Corresponds to `autoLoginList: List<bool>` in C#.
    pub auto_login_list: Vec<bool>,
}

/// The pre-`accountNameList` WPF shape — `Beanfun.AccountRecords`
/// with 6 parallel lists.
///
/// Ported installs that never re-saved after an upgrade still have
/// this class at the root. The conversion layer synthesises an empty
/// `account_name_list` for these rows (matching WPF's
/// `JsonConvert.DeserializeObject<Records>` that leaves `null` for
/// unknown fields, which `accRecInit` then pads to empty strings).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyAccountRecords {
    /// Corresponds to `regionList: List<string>` in C#.
    pub region_list: Vec<String>,
    /// Corresponds to `accountList: List<string>` in C#.
    pub account_list: Vec<String>,
    /// Corresponds to `passwdList: List<string>` in C#.
    pub passwd_list: Vec<String>,
    /// Corresponds to `verifyList: List<string>` in C#.
    pub verify_list: Vec<String>,
    /// Corresponds to `methodList: List<int>` in C#.
    pub method_list: Vec<i32>,
    /// Corresponds to `autoLoginList: List<bool>` in C#.
    pub auto_login_list: Vec<bool>,
}

/// Discriminated union between the two legacy WPF shapes.
///
/// The discriminant is the NRBF root class name; each variant carries
/// the already-extracted list fields so the migration layer never has
/// to re-touch the raw [`nrbf::value::Object`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyPayload {
    /// Root class was `Beanfun.Records` (current 7-field WPF shape).
    Records(LegacyRecords),
    /// Root class was `Beanfun.AccountRecords` (legacy 6-field shape
    /// without `accountNameList`).
    AccountRecords(LegacyAccountRecords),
}

/// Parse a legacy `Users.dat` NRBF stream into a [`LegacyPayload`].
///
/// `bytes` must be the raw NRBF byte stream *after* the P5 base64
/// unwrap — i.e. the `raw_bytes` carried by
/// [`crate::services::storage::StorageError::LegacyDataDetected`].
///
/// See the module-level docs for the `List<T>` shape / `null` handling
/// contract.
pub fn parse_legacy_payload(bytes: &[u8]) -> Result<LegacyPayload, NrbfError> {
    let message =
        RemotingMessage::parse(bytes).map_err(|err| NrbfError::Internal(format!("{err}")))?;

    let value = match message {
        RemotingMessage::Value(v) => v,
        RemotingMessage::MethodCall(..) => {
            return Err(NrbfError::Internal(
                "unexpected .NET Remoting MethodCall at root (legacy Users.dat should be a single Value graph)".into(),
            ));
        }
        RemotingMessage::MethodReturn(..) => {
            return Err(NrbfError::Internal(
                "unexpected .NET Remoting MethodReturn at root (legacy Users.dat should be a single Value graph)".into(),
            ));
        }
    };

    let object = match value {
        Value::Object(o) => o,
        other => {
            return Err(NrbfError::UnsupportedClass {
                name: format!("non-object root ({})", value_kind(&other)),
            });
        }
    };

    match object.class {
        CLASS_RECORDS => Ok(LegacyPayload::Records(parse_records(&object)?)),
        CLASS_ACCOUNT_RECORDS => Ok(LegacyPayload::AccountRecords(parse_account_records(
            &object,
        )?)),
        other => Err(NrbfError::UnsupportedClass {
            name: other.to_owned(),
        }),
    }
}

fn parse_records(object: &Object<'_>) -> Result<LegacyRecords, NrbfError> {
    Ok(LegacyRecords {
        region_list: extract_list_of_strings(object, CLASS_RECORDS, "regionList")?,
        account_list: extract_list_of_strings(object, CLASS_RECORDS, "accountList")?,
        account_name_list: extract_list_of_strings(object, CLASS_RECORDS, "accountNameList")?,
        passwd_list: extract_list_of_strings(object, CLASS_RECORDS, "passwdList")?,
        verify_list: extract_list_of_strings(object, CLASS_RECORDS, "verifyList")?,
        method_list: extract_list_of_i32(object, CLASS_RECORDS, "methodList")?,
        auto_login_list: extract_list_of_bool(object, CLASS_RECORDS, "autoLoginList")?,
    })
}

fn parse_account_records(object: &Object<'_>) -> Result<LegacyAccountRecords, NrbfError> {
    Ok(LegacyAccountRecords {
        region_list: extract_list_of_strings(object, CLASS_ACCOUNT_RECORDS, "regionList")?,
        account_list: extract_list_of_strings(object, CLASS_ACCOUNT_RECORDS, "accountList")?,
        passwd_list: extract_list_of_strings(object, CLASS_ACCOUNT_RECORDS, "passwdList")?,
        verify_list: extract_list_of_strings(object, CLASS_ACCOUNT_RECORDS, "verifyList")?,
        method_list: extract_list_of_i32(object, CLASS_ACCOUNT_RECORDS, "methodList")?,
        auto_login_list: extract_list_of_bool(object, CLASS_ACCOUNT_RECORDS, "autoLoginList")?,
    })
}

fn extract_list_of_strings(
    object: &Object<'_>,
    class: &'static str,
    member: &'static str,
) -> Result<Vec<String>, NrbfError> {
    extract_list(
        object,
        class,
        member,
        "List<String>",
        |element| match element {
            Value::String(s) => Some((*s).to_owned()),
            Value::Null => Some(String::new()),
            _ => None,
        },
    )
}

fn extract_list_of_i32(
    object: &Object<'_>,
    class: &'static str,
    member: &'static str,
) -> Result<Vec<i32>, NrbfError> {
    extract_list(object, class, member, "List<Int32>", |element| {
        if let Value::Int32(n) = element {
            Some(*n)
        } else {
            None
        }
    })
}

fn extract_list_of_bool(
    object: &Object<'_>,
    class: &'static str,
    member: &'static str,
) -> Result<Vec<bool>, NrbfError> {
    extract_list(object, class, member, "List<Boolean>", |element| {
        if let Value::Boolean(b) = element {
            Some(*b)
        } else {
            None
        }
    })
}

/// Generic `List<T>` extractor — walks the `_items` / `_size` pair on
/// `object.members[member]` and maps each live element through
/// `extract_elem`. See module docs for the `null` / sizing contract.
fn extract_list<T, F>(
    object: &Object<'_>,
    class: &'static str,
    member: &'static str,
    expected: &'static str,
    mut extract_elem: F,
) -> Result<Vec<T>, NrbfError>
where
    F: FnMut(&Value<'_>) -> Option<T>,
{
    let list_value = object
        .members
        .get(member)
        .ok_or(NrbfError::MissingMember { class, member })?;

    let list_object = match list_value {
        Value::Null => return Ok(Vec::new()),
        Value::Object(o) => o,
        _ => {
            return Err(NrbfError::TypeMismatch {
                class,
                member,
                expected,
            });
        }
    };

    let class_base = list_object
        .class
        .split_once('`')
        .map(|(head, _)| head)
        .unwrap_or(list_object.class);
    if class_base != LIST_CLASS_PREFIX {
        return Err(NrbfError::TypeMismatch {
            class,
            member,
            expected,
        });
    }

    let size = match list_object.members.get("_size") {
        Some(Value::Int32(n)) => *n,
        Some(_) => {
            return Err(NrbfError::TypeMismatch {
                class,
                member,
                expected: "List<T>._size (Int32)",
            });
        }
        None => return Err(NrbfError::MissingMember { class, member }),
    };

    let items_value = list_object
        .members
        .get("_items")
        .ok_or(NrbfError::MissingMember { class, member })?;

    let items = match items_value {
        Value::Array(a) => a.as_slice(),
        Value::Null => {
            if size == 0 {
                return Ok(Vec::new());
            } else {
                return Err(NrbfError::InconsistentListSize {
                    class,
                    member,
                    size,
                    items: 0,
                });
            }
        }
        _ => {
            return Err(NrbfError::TypeMismatch {
                class,
                member,
                expected: "List<T>._items (Array)",
            });
        }
    };

    if size < 0 {
        return Err(NrbfError::InconsistentListSize {
            class,
            member,
            size,
            items: items.len(),
        });
    }
    let size_usize = size as usize;
    if size_usize > items.len() {
        return Err(NrbfError::InconsistentListSize {
            class,
            member,
            size,
            items: items.len(),
        });
    }

    let mut out = Vec::with_capacity(size_usize);
    for element in &items[..size_usize] {
        match extract_elem(element) {
            Some(v) => out.push(v),
            None => {
                return Err(NrbfError::TypeMismatch {
                    class,
                    member,
                    expected,
                });
            }
        }
    }
    Ok(out)
}

fn value_kind(value: &Value<'_>) -> &'static str {
    match value {
        Value::Object(_) => "Object",
        Value::Array(_) => "Array",
        Value::Boolean(_) => "Boolean",
        Value::Byte(_) => "Byte",
        Value::Char(_) => "Char",
        Value::Decimal(_) => "Decimal",
        Value::Double(_) => "Double",
        Value::Int16(_) => "Int16",
        Value::Int32(_) => "Int32",
        Value::Int64(_) => "Int64",
        Value::SByte(_) => "SByte",
        Value::Single(_) => "Single",
        Value::TimeSpan(_) => "TimeSpan",
        Value::DateTime(_) => "DateTime",
        Value::UInt16(_) => "UInt16",
        Value::UInt32(_) => "UInt32",
        Value::UInt64(_) => "UInt64",
        Value::String(_) => "String",
        Value::Null => "Null",
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::*;

    // --- `Beanfun.Records` happy path -------------------------------

    #[test]
    fn parse_records_all_null_lists() {
        // `Beanfun.Records` where every list field is the null
        // reference (WPF default-constructed, never initialised).
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

        let payload = parse_legacy_payload(&bytes).expect("parse");
        match payload {
            LegacyPayload::Records(r) => {
                assert_eq!(r, LegacyRecords::default());
            }
            other => panic!("expected LegacyPayload::Records, got {other:?}"),
        }
    }

    #[test]
    fn parse_records_two_accounts() {
        // Two accounts, all seven lists populated.
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
                    MemberSpec::StringList(&[Some("Alice-TW"), Some("Bob-HK")]),
                ),
                (
                    "passwdList",
                    MemberSpec::StringList(&[Some("cipher1"), Some("cipher2")]),
                ),
                (
                    "verifyList",
                    MemberSpec::StringList(&[Some(""), Some("v2")]),
                ),
                ("methodList", MemberSpec::I32List(&[0, 1])),
                ("autoLoginList", MemberSpec::BoolList(&[true, false])),
            ],
        );

        let payload = parse_legacy_payload(&bytes).expect("parse");
        let records = match payload {
            LegacyPayload::Records(r) => r,
            other => panic!("expected LegacyPayload::Records, got {other:?}"),
        };
        assert_eq!(records.region_list, vec!["TW", "HK"]);
        assert_eq!(records.account_list, vec!["alice", "bob"]);
        assert_eq!(records.account_name_list, vec!["Alice-TW", "Bob-HK"]);
        assert_eq!(records.passwd_list, vec!["cipher1", "cipher2"]);
        assert_eq!(records.verify_list, vec!["", "v2"]);
        assert_eq!(records.method_list, vec![0, 1]);
        assert_eq!(records.auto_login_list, vec![true, false]);
    }

    #[test]
    fn parse_records_empty_lists() {
        // All seven lists are empty (`_size == 0`, `_items.len() == 0`).
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                ("regionList", MemberSpec::StringList(&[])),
                ("accountList", MemberSpec::StringList(&[])),
                ("accountNameList", MemberSpec::StringList(&[])),
                ("passwdList", MemberSpec::StringList(&[])),
                ("verifyList", MemberSpec::StringList(&[])),
                ("methodList", MemberSpec::I32List(&[])),
                ("autoLoginList", MemberSpec::BoolList(&[])),
            ],
        );

        let payload = parse_legacy_payload(&bytes).expect("parse");
        assert_eq!(payload, LegacyPayload::Records(LegacyRecords::default()));
    }

    #[test]
    fn parse_records_string_list_with_null_element_maps_to_empty_string() {
        // WPF can serialise a populated List<string> that contains
        // null elements (rare but observed). They must map to `""`
        // matching WPF's `accRecInit` padding rule.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                ("regionList", MemberSpec::StringList(&[Some("TW"), None])),
                (
                    "accountList",
                    MemberSpec::StringList(&[Some("alice"), Some("bob")]),
                ),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );

        let payload = parse_legacy_payload(&bytes).expect("parse");
        let LegacyPayload::Records(records) = payload else {
            panic!("expected Records");
        };
        assert_eq!(records.region_list, vec!["TW", ""]);
    }

    // --- `_size` vs `_items.len()` semantics -----------------------

    #[test]
    fn parse_records_takes_first_size_elements_when_items_longer() {
        // `_items.len() == 3`, `_size == 2` — trailing slot is
        // capacity padding and must be ignored.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                (
                    "regionList",
                    MemberSpec::StringListWithSize {
                        items: &[Some("TW"), Some("HK"), Some("CAPACITY_PADDING")],
                        size: 2,
                    },
                ),
                ("accountList", MemberSpec::NullStringList),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );
        let payload = parse_legacy_payload(&bytes).expect("parse");
        let LegacyPayload::Records(records) = payload else {
            panic!("expected Records");
        };
        assert_eq!(records.region_list, vec!["TW", "HK"]);
    }

    #[test]
    fn parse_records_size_greater_than_items_returns_inconsistent() {
        // `_size > _items.len()` cannot be valid.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                (
                    "regionList",
                    MemberSpec::StringListWithSize {
                        items: &[Some("TW")],
                        size: 5,
                    },
                ),
                ("accountList", MemberSpec::NullStringList),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );
        let err = parse_legacy_payload(&bytes).expect_err("must error");
        match err {
            NrbfError::InconsistentListSize {
                class,
                member,
                size,
                items,
            } => {
                assert_eq!(class, CLASS_RECORDS);
                assert_eq!(member, "regionList");
                assert_eq!(size, 5);
                assert_eq!(items, 1);
            }
            other => panic!("expected InconsistentListSize, got {other:?}"),
        }
    }

    // --- Legacy `Beanfun.AccountRecords` shape ---------------------

    #[test]
    fn parse_account_records_six_fields() {
        let bytes = build_root_class(
            CLASS_ACCOUNT_RECORDS,
            &[
                ("regionList", MemberSpec::StringList(&[Some("TW")])),
                (
                    "accountList",
                    MemberSpec::StringList(&[Some("legacy-user")]),
                ),
                (
                    "passwdList",
                    MemberSpec::StringList(&[Some("legacy-cipher")]),
                ),
                ("verifyList", MemberSpec::StringList(&[Some("")])),
                ("methodList", MemberSpec::I32List(&[0])),
                ("autoLoginList", MemberSpec::BoolList(&[false])),
            ],
        );
        let payload = parse_legacy_payload(&bytes).expect("parse");
        match payload {
            LegacyPayload::AccountRecords(ar) => {
                assert_eq!(ar.region_list, vec!["TW"]);
                assert_eq!(ar.account_list, vec!["legacy-user"]);
                assert_eq!(ar.passwd_list, vec!["legacy-cipher"]);
                assert_eq!(ar.verify_list, vec![""]);
                assert_eq!(ar.method_list, vec![0]);
                assert_eq!(ar.auto_login_list, vec![false]);
            }
            other => panic!("expected LegacyPayload::AccountRecords, got {other:?}"),
        }
    }

    // --- Error paths -----------------------------------------------

    #[test]
    fn parse_unknown_class_returns_unsupported() {
        let bytes = build_root_class("Some.Other.Class", &[("foo", MemberSpec::NullStringList)]);
        let err = parse_legacy_payload(&bytes).expect_err("must error");
        match err {
            NrbfError::UnsupportedClass { name } => {
                assert_eq!(name, "Some.Other.Class");
            }
            other => panic!("expected UnsupportedClass, got {other:?}"),
        }
    }

    #[test]
    fn parse_malformed_header_returns_internal() {
        // Truncated right after the header byte — nrbf crate must
        // reject this before we ever see a `Value`.
        let err = parse_legacy_payload(&[0x00]).expect_err("must error");
        assert!(
            matches!(err, NrbfError::Internal(_)),
            "expected Internal, got {err:?}"
        );
    }

    #[test]
    fn parse_records_missing_member_returns_missing_member() {
        // Only 6 members on a Records root — `accountNameList` is
        // absent, which must trip the missing-member guard.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                ("regionList", MemberSpec::NullStringList),
                ("accountList", MemberSpec::NullStringList),
                // No accountNameList
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );
        let err = parse_legacy_payload(&bytes).expect_err("must error");
        match err {
            NrbfError::MissingMember { class, member } => {
                assert_eq!(class, CLASS_RECORDS);
                assert_eq!(member, "accountNameList");
            }
            other => panic!("expected MissingMember, got {other:?}"),
        }
    }

    #[test]
    fn parse_records_wrong_member_type_returns_type_mismatch() {
        // regionList field carries an Int32 instead of a List<String>.
        let bytes = build_root_class(
            CLASS_RECORDS,
            &[
                ("regionList", MemberSpec::Int32InsteadOfList(42)),
                ("accountList", MemberSpec::NullStringList),
                ("accountNameList", MemberSpec::NullStringList),
                ("passwdList", MemberSpec::NullStringList),
                ("verifyList", MemberSpec::NullStringList),
                ("methodList", MemberSpec::NullI32List),
                ("autoLoginList", MemberSpec::NullBoolList),
            ],
        );
        let err = parse_legacy_payload(&bytes).expect_err("must error");
        match err {
            NrbfError::TypeMismatch {
                class,
                member,
                expected,
            } => {
                assert_eq!(class, CLASS_RECORDS);
                assert_eq!(member, "regionList");
                assert_eq!(expected, "List<String>");
            }
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }
}

// ============================================================
// Test fixture — minimal NRBF byte-stream builder
// ============================================================
//
// Only emits the subset needed for `Beanfun.Records` /
// `Beanfun.AccountRecords` round-trip fixtures:
//
// - SerializedStreamHeader         (type 0)
// - SystemClassWithMembersAndTypes (type 4)  — nested `List<T>`
// - ClassWithMembersAndTypes       (type 5)  — Beanfun root class
// - BinaryObjectString             (type 6)  — element of `ArraySingleString`
// - MemberReference                (type 9)  — `_items` pointer inside `List<T>`
// - ObjectNull                     (type 10) — null member / null list element
// - MessageEnd                     (type 11)
// - BinaryLibrary                  (type 12) — declares `"Beanfun"` lib
// - ArraySinglePrimitive           (type 15) — backing array of `List<int>`/`List<bool>`
// - ArraySingleString              (type 17) — backing array of `List<string>`
//
// # Byte-stream invariants we rely on
//
// Inside each `List<T>` record, the `_items` field's value **must** be
// a `MemberReference (9)` pointing to a separately-emitted
// `ArraySingleString (17)` / `ArraySinglePrimitive (15)` record that
// immediately follows the enclosing `List<T>`. Emitting an
// `ArraySingleString` inline as the `_items` value is rejected by
// the upstream `nrbf` crate (the parser only accepts
// `BinaryObjectString` / `MemberReference` / `ObjectNull` for a
// member declared as `StringArray`).
//
// The reference array layout is the same pattern used by the
// crate's own `list_of_customers.rs` round-trip fixture and matches
// the MS-[MS-NRBF] grammar for `memberReference`.
//
// For `_size` / `_version` — declared as `Primitive(Int32)` — the
// value is emitted as `MemberPrimitiveUnTyped` (raw 4-byte LE),
// *not* `MemberPrimitiveTyped`, per §2.3.2.4.
//
// Not a general-purpose NRBF writer. Gated behind `cfg(test)` (for
// this crate's unit tests) + the `test-fixtures` cargo feature (for
// integration tests in `tests/`) so it never ships inside the
// production binary. `pub` visibility is required so integration
// tests in `tests/storage_legacy.rs` can reuse the byte builder
// verbatim — the NRBF layout invariants belong here, not duplicated
// at every call site.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixture {
    // Record type codes — MS-NRBF §2.1.2.1.
    const RT_SERIALIZED_STREAM_HEADER: u8 = 0;
    const RT_CLASS_WITH_MEMBERS_AND_TYPES: u8 = 5;
    const RT_SYSTEM_CLASS_WITH_MEMBERS_AND_TYPES: u8 = 4;
    const RT_BINARY_OBJECT_STRING: u8 = 6;
    const RT_MEMBER_REFERENCE: u8 = 9;
    const RT_OBJECT_NULL: u8 = 10;
    const RT_MESSAGE_END: u8 = 11;
    const RT_BINARY_LIBRARY: u8 = 12;
    const RT_ARRAY_SINGLE_PRIMITIVE: u8 = 15;
    const RT_ARRAY_SINGLE_STRING: u8 = 17;

    // BinaryTypeEnum — MS-NRBF §2.1.2.2.
    const BT_PRIMITIVE: u8 = 0;
    const BT_SYSTEM_CLASS: u8 = 3;
    const BT_STRING_ARRAY: u8 = 6;
    const BT_PRIMITIVE_ARRAY: u8 = 7;

    // PrimitiveTypeEnum — MS-NRBF §2.1.2.3.
    const PT_BOOLEAN: u8 = 1;
    const PT_INT32: u8 = 8;

    const LIBRARY_ID: i32 = 2;
    const ROOT_OBJECT_ID: i32 = 1;

    // Assembly-qualified generic names — the `Version` /
    // `PublicKeyToken` fields are free-form for our parser's purposes
    // (we only match on the bit before the backtick), but we keep
    // them close to what real WPF .NET Framework streams emit so the
    // fixtures double as documentation.
    const LIST_STRING_NAME: &str =
        "System.Collections.Generic.List`1[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
    const LIST_INT32_NAME: &str =
        "System.Collections.Generic.List`1[[System.Int32, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
    const LIST_BOOLEAN_NAME: &str =
        "System.Collections.Generic.List`1[[System.Boolean, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";

    /// Spec for a single member on a Beanfun root class. Each variant
    /// decides both the declared `BinaryTypeEnum` (for the root's
    /// MemberTypeInfo) and the follow-on member-value record(s).
    #[derive(Debug, Clone)]
    pub enum MemberSpec<'a> {
        /// `List<string>` field = null reference.
        NullStringList,
        /// `List<int>` field = null reference.
        NullI32List,
        /// `List<bool>` field = null reference.
        NullBoolList,
        /// `List<string>` populated with `items` and
        /// `_size == items.len()`.
        StringList(&'a [Option<&'a str>]),
        /// `List<string>` with an explicit `_size` that may differ
        /// from `items.len()` — drives `_size` vs `_items.len()` tests.
        StringListWithSize {
            /// Raw `_items` contents.
            items: &'a [Option<&'a str>],
            /// Claimed `List<T>._size`.
            size: i32,
        },
        /// `List<int>` populated with `items`.
        I32List(&'a [i32]),
        /// `List<bool>` populated with `items`.
        BoolList(&'a [bool]),
        /// Drives the `TypeMismatch` negative test — declares the
        /// member as Primitive(Int32) inline so the parser sees a
        /// bare `Value::Int32` where a `List<String>` was expected.
        Int32InsteadOfList(i32),
    }

    /// Build a full `.NET` NRBF byte-stream for a given root class
    /// name and ordered members. Emits the `SerializedStreamHeader`,
    /// `BinaryLibrary`, root `ClassWithMembersAndTypes`, each
    /// member's inline value (potentially a nested
    /// `SystemClassWithMembersAndTypes` + the referenced
    /// `ArraySingleString` / `ArraySinglePrimitive`), and finally
    /// `MessageEnd`.
    pub fn build_root_class(class_name: &str, members: &[(&str, MemberSpec<'_>)]) -> Vec<u8> {
        let mut out = Vec::new();

        write_serialized_stream_header(&mut out);
        write_binary_library(&mut out, LIBRARY_ID, "Beanfun");

        // Root ClassWithMembersAndTypes.
        out.push(RT_CLASS_WITH_MEMBERS_AND_TYPES);
        write_i32(&mut out, ROOT_OBJECT_ID);
        write_len_prefixed_string(&mut out, class_name);
        write_i32(&mut out, members.len() as i32);
        for (name, _) in members {
            write_len_prefixed_string(&mut out, name);
        }
        for (_, spec) in members {
            out.push(binary_type_enum(spec));
        }
        for (_, spec) in members {
            write_additional_info(&mut out, spec);
        }
        write_i32(&mut out, LIBRARY_ID);

        // Per-member values. Object IDs 2..= are allocated lazily as
        // the nested `List<T>` / `ArraySingle*` / `BinaryObjectString`
        // records are emitted.
        let mut next_id = ROOT_OBJECT_ID + 1;
        for (_, spec) in members {
            write_member_value(&mut out, spec, &mut next_id);
        }

        out.push(RT_MESSAGE_END);
        out
    }

    fn binary_type_enum(spec: &MemberSpec<'_>) -> u8 {
        match spec {
            MemberSpec::Int32InsteadOfList(_) => BT_PRIMITIVE,
            _ => BT_SYSTEM_CLASS,
        }
    }

    /// Emit the `AdditionalInfos` entry for this member's declared
    /// `BinaryTypeEnum`.
    fn write_additional_info(out: &mut Vec<u8>, spec: &MemberSpec<'_>) {
        match spec {
            MemberSpec::Int32InsteadOfList(_) => out.push(PT_INT32),
            MemberSpec::NullStringList
            | MemberSpec::StringList(_)
            | MemberSpec::StringListWithSize { .. } => {
                write_len_prefixed_string(out, LIST_STRING_NAME);
            }
            MemberSpec::NullI32List | MemberSpec::I32List(_) => {
                write_len_prefixed_string(out, LIST_INT32_NAME);
            }
            MemberSpec::NullBoolList | MemberSpec::BoolList(_) => {
                write_len_prefixed_string(out, LIST_BOOLEAN_NAME);
            }
        }
    }

    fn write_member_value(out: &mut Vec<u8>, spec: &MemberSpec<'_>, next_id: &mut i32) {
        match spec {
            MemberSpec::NullStringList | MemberSpec::NullI32List | MemberSpec::NullBoolList => {
                out.push(RT_OBJECT_NULL);
            }
            MemberSpec::StringList(items) => {
                write_list_of_strings(out, next_id, items, items.len() as i32);
            }
            MemberSpec::StringListWithSize { items, size } => {
                write_list_of_strings(out, next_id, items, *size);
            }
            MemberSpec::I32List(items) => {
                write_list_of_i32(out, next_id, items);
            }
            MemberSpec::BoolList(items) => {
                write_list_of_bool(out, next_id, items);
            }
            MemberSpec::Int32InsteadOfList(n) => {
                // Declared as `Primitive(Int32)` on the root class →
                // value is a bare 4-byte LE Int32 (MemberPrimitiveUnTyped),
                // not a full `MemberPrimitiveTyped` record.
                write_i32(out, *n);
            }
        }
    }

    fn write_list_of_strings(
        out: &mut Vec<u8>,
        next_id: &mut i32,
        items: &[Option<&str>],
        size: i32,
    ) {
        let list_id = *next_id;
        *next_id += 1;
        let array_id = *next_id;
        *next_id += 1;

        // SystemClassWithMembersAndTypes for List<string>.
        out.push(RT_SYSTEM_CLASS_WITH_MEMBERS_AND_TYPES);
        write_i32(out, list_id);
        write_len_prefixed_string(out, LIST_STRING_NAME);
        write_i32(out, 3);
        write_len_prefixed_string(out, "_items");
        write_len_prefixed_string(out, "_size");
        write_len_prefixed_string(out, "_version");
        out.push(BT_STRING_ARRAY);
        out.push(BT_PRIMITIVE);
        out.push(BT_PRIMITIVE);
        out.push(PT_INT32); // _size primitive type
        out.push(PT_INT32); // _version primitive type

        // Inline member values for List<string>:
        // _items → MemberReference to the ArraySingleString below.
        out.push(RT_MEMBER_REFERENCE);
        write_i32(out, array_id);
        // _size / _version → bare Int32 (MemberPrimitiveUnTyped).
        write_i32(out, size);
        write_i32(out, 0);

        // Referenced ArraySingleString payload.
        out.push(RT_ARRAY_SINGLE_STRING);
        write_i32(out, array_id);
        write_i32(out, items.len() as i32);
        for element in items {
            match element {
                Some(s) => {
                    let str_id = *next_id;
                    *next_id += 1;
                    out.push(RT_BINARY_OBJECT_STRING);
                    write_i32(out, str_id);
                    write_len_prefixed_string(out, s);
                }
                None => out.push(RT_OBJECT_NULL),
            }
        }
    }

    fn write_list_of_i32(out: &mut Vec<u8>, next_id: &mut i32, items: &[i32]) {
        let list_id = *next_id;
        *next_id += 1;
        let array_id = *next_id;
        *next_id += 1;

        out.push(RT_SYSTEM_CLASS_WITH_MEMBERS_AND_TYPES);
        write_i32(out, list_id);
        write_len_prefixed_string(out, LIST_INT32_NAME);
        write_i32(out, 3);
        write_len_prefixed_string(out, "_items");
        write_len_prefixed_string(out, "_size");
        write_len_prefixed_string(out, "_version");
        out.push(BT_PRIMITIVE_ARRAY);
        out.push(BT_PRIMITIVE);
        out.push(BT_PRIMITIVE);
        out.push(PT_INT32); // _items element primitive type
        out.push(PT_INT32); // _size primitive type
        out.push(PT_INT32); // _version primitive type

        out.push(RT_MEMBER_REFERENCE);
        write_i32(out, array_id);
        write_i32(out, items.len() as i32); // _size
        write_i32(out, 0); // _version

        out.push(RT_ARRAY_SINGLE_PRIMITIVE);
        write_i32(out, array_id);
        write_i32(out, items.len() as i32);
        out.push(PT_INT32);
        for n in items {
            write_i32(out, *n);
        }
    }

    fn write_list_of_bool(out: &mut Vec<u8>, next_id: &mut i32, items: &[bool]) {
        let list_id = *next_id;
        *next_id += 1;
        let array_id = *next_id;
        *next_id += 1;

        out.push(RT_SYSTEM_CLASS_WITH_MEMBERS_AND_TYPES);
        write_i32(out, list_id);
        write_len_prefixed_string(out, LIST_BOOLEAN_NAME);
        write_i32(out, 3);
        write_len_prefixed_string(out, "_items");
        write_len_prefixed_string(out, "_size");
        write_len_prefixed_string(out, "_version");
        out.push(BT_PRIMITIVE_ARRAY);
        out.push(BT_PRIMITIVE);
        out.push(BT_PRIMITIVE);
        out.push(PT_BOOLEAN); // _items element primitive type
        out.push(PT_INT32); // _size primitive type
        out.push(PT_INT32); // _version primitive type

        out.push(RT_MEMBER_REFERENCE);
        write_i32(out, array_id);
        write_i32(out, items.len() as i32);
        write_i32(out, 0);

        out.push(RT_ARRAY_SINGLE_PRIMITIVE);
        write_i32(out, array_id);
        write_i32(out, items.len() as i32);
        out.push(PT_BOOLEAN);
        for b in items {
            out.push(u8::from(*b));
        }
    }

    fn write_serialized_stream_header(out: &mut Vec<u8>) {
        out.push(RT_SERIALIZED_STREAM_HEADER);
        write_i32(out, ROOT_OBJECT_ID);
        write_i32(out, -1);
        write_i32(out, 1);
        write_i32(out, 0);
    }

    fn write_binary_library(out: &mut Vec<u8>, lib_id: i32, name: &str) {
        out.push(RT_BINARY_LIBRARY);
        write_i32(out, lib_id);
        write_len_prefixed_string(out, name);
    }

    fn write_i32(out: &mut Vec<u8>, n: i32) {
        out.extend_from_slice(&n.to_le_bytes());
    }

    /// LengthPrefixedString — MS-NRBF §2.1.1.6. 7-bit variable-length
    /// prefix followed by raw UTF-8 bytes.
    fn write_len_prefixed_string(out: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        let mut len = bytes.len();
        loop {
            let b = (len & 0x7F) as u8;
            len >>= 7;
            if len == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
        out.extend_from_slice(bytes);
    }
}
