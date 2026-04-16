//! Step 1 of the TW Regular flow: `GET Login/Index?pSKey=…`.
//!
//! Fetches the login page, scrapes the `__RequestVerificationToken`
//! hidden input, and returns both the token and the canonical index URL.
//! The URL is surfaced so every follow-up call can set it as
//! `Referer` without rebuilding the string.
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L40-54.

use reqwest::header;
use url::Url;

use super::ensure_success;
use crate::core::parser::extract_verification_token;
use crate::services::beanfun::{BeanfunClient, LoginError};

/// Result of a successful `GET Login/Index` — the antiforgery token plus
/// the fully qualified URL of the page (used as `Referer` downstream).
#[derive(Debug, Clone)]
pub struct LoginIndex {
    /// `__RequestVerificationToken` value from the returned HTML.
    pub verification_token: String,
    /// URL of the Index page, including the `?pSKey=…` query. Re-used as
    /// the `Referer` header by [`super::check_account_type`] and
    /// [`super::account_login`].
    pub index_url: Url,
}

/// Fetch the login Index page and extract the antiforgery token.
///
/// Returns:
/// - [`LoginError::MissingVerificationToken`] when the HTML does not
///   expose a `__RequestVerificationToken` input (mapped from the
///   underlying [`crate::core::parser::ParserError`] so the caller only
///   needs to pattern-match on `LoginError`).
/// - [`LoginError::Http`] on any transport failure.
pub async fn get_login_index(client: &BeanfunClient, skey: &str) -> Result<LoginIndex, LoginError> {
    let index_url = client.login_url_with_skey("Login/Index", skey)?;

    let resp = client
        .http()
        .get(index_url.clone())
        .header(header::ACCEPT, "text/html")
        .send()
        .await?;

    ensure_success(&resp, "Login/Index")?;
    let body = client.bounded_text(resp).await?;

    // Remap the parser-level "missing token" into the higher-level
    // `LoginError::MissingVerificationToken` so call sites don't need to
    // special-case a nested `Parser(…)` variant for this very common
    // early-fail.
    let verification_token =
        extract_verification_token(&body).map_err(|_| LoginError::MissingVerificationToken)?;

    Ok(LoginIndex {
        verification_token,
        index_url,
    })
}
