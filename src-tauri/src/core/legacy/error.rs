//! Typed error enum for the legacy NRBF parser.
//!
//! Scopes to P6 chunk 6.1 only — this module stays pure and carries
//! no I/O or migration concern. The application layer in
//! `services::storage::legacy` (chunk 6.2, not yet implemented) will
//! wrap these variants inside a sibling `LegacyMigrateError` together
//! with [`StorageError`][storage-err], keeping the core / services
//! error surfaces decoupled.
//!
//! [storage-err]: crate::services::storage::StorageError
//!
//! # Design
//!
//! - [`NrbfError::Internal`] wraps the upstream `nrbf::Error` *as a
//!   stringified message* instead of `#[from]`-ing the concrete type.
//!   Upstream's `Error<'i>` is lifetime-bound to the input slice, so
//!   carrying it by value across an owned error would force every
//!   caller into a borrowed lifetime; a plain `String` is sufficient
//!   for logs / UI and lets the error outlive the input buffer.
//! - [`NrbfError::UnsupportedClass`] deliberately distinguishes
//!   "wrong class at the root" from "couldn't parse" — callers may
//!   want to treat it as "definitely not a legacy Users.dat, skip
//!   migration" while still logging the class name for diagnostics.
//! - [`NrbfError::MissingMember`] / [`NrbfError::TypeMismatch`] use
//!   `&'static str` for class + member names to avoid a heap
//!   allocation per error at the (admittedly rare) failure path.
//! - [`NrbfError::InconsistentListSize`] carries concrete numbers so
//!   support bug reports can reproduce the malformed stream without a
//!   binary dump.

use thiserror::Error;

/// Typed failure surface for [`crate::core::legacy::parse_legacy_payload`].
#[derive(Debug, Error)]
pub enum NrbfError {
    /// Upstream `nrbf` crate rejected the byte stream before we got a
    /// chance to inspect the root [`nrbf::Value`]. Carries the
    /// upstream error formatted for logs; the original lifetime-bound
    /// value is dropped.
    #[error("NRBF parse failure: {0}")]
    Internal(String),

    /// Root object's class name was neither `Beanfun.Records` nor
    /// `Beanfun.AccountRecords`. Typically means the file is *not* a
    /// legacy Users.dat (e.g. user manually dropped something else in
    /// `%APPDATA%\Beanfun\Users.dat`) and migration must not
    /// synthesise a bogus records list.
    #[error("unsupported NRBF root class: {name}")]
    UnsupportedClass {
        /// Class name as reported by the root
        /// [`nrbf::value::Object::class`].
        name: String,
    },

    /// Root object is missing a required field. WPF never serialised
    /// a `Beanfun.Records` / `AccountRecords` without all its list
    /// fields, so this indicates a truncated / mismatched stream
    /// rather than an old-version shape.
    #[error("NRBF class {class}: missing required member {member}")]
    MissingMember {
        /// WPF class name (`"Beanfun.Records"` or
        /// `"Beanfun.AccountRecords"`).
        class: &'static str,
        /// WPF field name (camelCase, e.g. `"accountList"`).
        member: &'static str,
    },

    /// A member was present but the carried [`nrbf::Value`] did not
    /// match the expected shape (e.g. `accountList` was an `Int32`
    /// instead of a `List<String>` / `Null`).
    #[error("NRBF class {class}: member {member} type mismatch (expected {expected})")]
    TypeMismatch {
        /// WPF class name.
        class: &'static str,
        /// WPF field name.
        member: &'static str,
        /// Human-readable description of the expected shape (e.g.
        /// `"List<String>"`).
        expected: &'static str,
    },

    /// `List<T>._size` was inconsistent with `_items.len()` — `size`
    /// should always be `<= items`. WPF uses `_size` as the
    /// authoritative element count (trailing slots in `_items` are
    /// capacity) so `size > items` indicates a malformed stream.
    #[error(
        "NRBF class {class}: member {member} has _size={size} but _items length={items} (size must be <= items)"
    )]
    InconsistentListSize {
        /// WPF class name.
        class: &'static str,
        /// WPF field name.
        member: &'static str,
        /// Reported `List<T>._size`.
        size: i32,
        /// Actual `List<T>._items.len()`.
        items: usize,
    },
}
