//! Login flows for the Beanfun service.
//!
//! Every submodule here is an `async` function (or small family of
//! functions) that drives one discrete HTTP round-trip of the WPF login
//! sequence. The flows are composed by higher-level orchestrators (added
//! in later chunks) that call each step in order, handling branching
//! (TOTP required / advance-check required / QR-code polling) as typed
//! [`super::LoginError`] variants.
//!
//! # Why split per step?
//!
//! The WPF source inlines the entire flow inside one giant `try` block per
//! method (`TwRegularLogin`, `HkRegularLogin`, `QRCodeLogin`). That makes
//! unit-testing any individual step difficult without mocking the whole
//! sequence. Splitting each HTTP call into its own function lets us:
//!
//! - Test each step against a tiny wiremock expectation.
//! - Reuse shared steps (e.g. `get_session_key`) across all three login
//!   methods without duplicating code.
//! - Surface per-step errors via the typed [`super::LoginError`] enum.

pub mod account_login;
pub mod check_account_type;
pub mod completed;
pub mod hk_error;
pub mod hk_regular;
pub mod index;
pub mod registered_device;
pub mod return_aspx;
pub mod send_login;
pub mod session_key;
pub mod totp;
pub mod totp_challenge;
pub mod tw_regular;

pub use account_login::account_login;
pub use check_account_type::check_account_type;
pub use completed::login_completed;
pub use hk_error::{extract_hk_error_signal, HkErrorSignal};
pub use hk_regular::login_hk_regular;
pub use index::{get_login_index, LoginIndex};
pub use registered_device::login_registered_device;
pub use return_aspx::post_return_aspx;
pub use send_login::send_login;
pub use session_key::get_session_key;
pub use totp::login_totp;
pub use totp_challenge::TotpChallenge;
pub use tw_regular::login_tw_regular;

use crate::services::beanfun::LoginError;

// -----------------------------------------------------------------------------
// Shared request helpers
// -----------------------------------------------------------------------------

/// Fail with [`LoginError::Unknown`] when `resp` is not a 2xx. Keeps
/// every login step's "non-success shortcut" to one line so the error
/// text stays consistent across the flow.
///
/// Does **not** handle 3xx as success — the one step that needs that
/// (`return.aspx`) inspects the status itself.
pub(crate) fn ensure_success(resp: &reqwest::Response, step: &str) -> Result<(), LoginError> {
    if !resp.status().is_success() {
        return Err(LoginError::Unknown(format!(
            "{step} returned HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Apply the exact header set that WPF's `SetJsonHeaders` installs before
/// any JSON-bodied login call (CheckAccountType, AccountLogin).
///
/// Factored out here because both call sites send the **same** four
/// headers; keeping a single helper means a future protocol tweak only
/// needs to be applied in one place.
///
/// Note: `Content-Type: application/json; charset=utf-8` is **not** set
/// explicitly — reqwest's `.json(&body)` adds it automatically, which
/// byte-matches what WPF's `Headers[HttpRequestHeader.ContentType]`
/// assignment emits.
pub(crate) fn apply_json_headers(
    rb: reqwest::RequestBuilder,
    verification_token: &str,
    referer: &str,
) -> reqwest::RequestBuilder {
    rb.header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
        .header(reqwest::header::REFERER, referer)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("RequestVerificationToken", verification_token)
}
