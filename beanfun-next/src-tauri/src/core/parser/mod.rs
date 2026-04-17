//! HTML/URL parsers for the beanfun web surface.
//!
//! Every parser here is:
//! - **Pure** — no I/O, no async. Takes a `&str`, returns a plain value /
//!   `Result`.
//! - **1:1 aligned with the legacy WPF regex-based extractors** (see module
//!   docs on each individual parser). The HTML we consume comes from ASP.NET
//!   WebForms pages whose hidden-field layout is extremely stable across
//!   releases — the C# reference has used these exact regexes for 10+ years,
//!   so we intentionally stay with regex rather than pulling in a full DOM
//!   parser (which would change normalization semantics and add risk).
//! - **Defensive by default** — missing required fields surface as typed
//!   [`ParserError`] variants instead of panicking.
//!
//! Each submodule targets a single response / string kind (SRP) and reuses a
//! shared error enum so consumers only need to match a single type.

pub mod account;
pub mod akey;
pub mod form;
pub mod token;
pub mod viewstate;

use regex::Regex;
use thiserror::Error;

/// Return the first capture group of the first regex match in `input`, owned
/// as a `String`. Returns `None` when the pattern fails to match at all.
///
/// Every regex-based parser under `core::parser` follows the same
/// "first match, group 1, or nothing" shape (the C# reference uses
/// `regex.Match(...).Groups[1].Value`), so centralising the chain here keeps
/// call sites to a single line and locks the semantics in one place.
pub(crate) fn capture_first(re: &Regex, input: &str) -> Option<String> {
    re.captures(input)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
}

/// Errors surfaced by the parsers under [`crate::core::parser`].
///
/// All variants are non-fatal at the parser level: the caller decides whether
/// a missing field should abort the containing flow (`errmsg = "LoginNoXxx"`
/// in the legacy WPF code) or trigger a retry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParserError {
    /// `__VIEWSTATE` hidden input absent — WPF signals this as
    /// `errmsg = "LoginNoViewstate"`.
    #[error("missing __VIEWSTATE hidden field in HTML")]
    MissingViewState,

    /// `akey=...` query parameter / fragment not found on the redirect URL —
    /// WPF signals this as `errmsg = "LoginNoAkey"` / `"AKeyParseFailed"`.
    #[error("missing akey in URL or text payload")]
    MissingAkey,

    /// `__RequestVerificationToken` hidden input absent — WPF simply leaves
    /// the anti-forgery token blank when this happens, but we surface it so
    /// callers can decide.
    #[error("missing __RequestVerificationToken hidden field in HTML")]
    MissingRequestVerificationToken,
}

/// Convenience alias: every parser in this module uses [`ParserError`].
pub type Result<T> = std::result::Result<T, ParserError>;

// Re-export the most frequently used public items so services/commands can
// `use crate::core::parser::{extract_viewstate, ViewStateForm, …}` without
// reaching into submodules individually.
pub use account::{
    extract_account_limit_notice, extract_service_account_create_time, extract_service_accounts,
    ServiceAccountRow,
};
pub use akey::extract_akey;
pub use form::{extract_hidden_inputs, HiddenInput};
pub use token::extract_verification_token;
pub use viewstate::{extract_viewstate, ViewStateForm};
