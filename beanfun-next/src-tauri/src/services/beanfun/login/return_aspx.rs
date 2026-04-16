//! Step 5 of the TW Regular flow: `POST …/beanfun_block/bflogin/return.aspx`.
//!
//! This is the final redirect-hop that authenticates the portal session
//! and — critically — drops a `bfWebToken` cookie that every subsequent
//! service call relies on.
//!
//! Two peculiarities worth calling out:
//!
//! 1. **No redirects.** The server replies with 302 → we need to read
//!    the `Set-Cookie` header *on that 302 response* before any auto
//!    redirect consumes it. We use
//!    [`BeanfunClient::http_no_redirect`](crate::services::beanfun::BeanfunClient::http_no_redirect)
//!    for this reason.
//! 2. **Cookie-header scrape, not cookie-jar read.** reqwest's cookie
//!    store would have the value too, but reading the raw header is the
//!    exact behaviour of WPF (L160-165) and is independent of the jar's
//!    domain/path matching. When in doubt, match the reference.
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L148-176.

use std::sync::OnceLock;

use regex::Regex;
use reqwest::header;

use crate::core::parser::HiddenInput;
use crate::services::beanfun::{BeanfunClient, LoginError};

/// POST the SendLogin-scraped form to `return.aspx` and pull the
/// `bfWebToken` value out of the `Set-Cookie` header.
///
/// Accepts any HTTP 2xx / 3xx — reqwest's no-redirect client surfaces
/// the 302 as-is. 4xx / 5xx are surfaced as [`LoginError::Unknown`].
pub async fn post_return_aspx(
    client: &BeanfunClient,
    form: &[HiddenInput],
) -> Result<String, LoginError> {
    let url = client.portal_url("beanfun_block/bflogin/return.aspx")?;
    // WPF sends Referer = login_base + "/". Our Url::as_str() already
    // carries the trailing slash (url::Url canonicalises that for us).
    let login_base = client.config().endpoints.login_base.as_str().to_owned();

    let resp = client
        .http_no_redirect()
        .post(url)
        .header(header::REFERER, login_base)
        .form(form)
        .send()
        .await?;

    let status = resp.status();
    if !(status.is_success() || status.is_redirection()) {
        return Err(LoginError::Unknown(format!(
            "return.aspx returned HTTP {status}"
        )));
    }

    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|hv| hv.to_str().ok())
        .find_map(scan_bfwebtoken)
        .ok_or(LoginError::MissingWebToken)
}

/// Extract the `bfWebToken=…` value from a single `Set-Cookie` header.
/// Returns `None` when the cookie isn't present. A single header can
/// only carry one cookie, so iterating through all headers and taking
/// the first match is correct.
fn scan_bfwebtoken(cookie_header: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // `[^;]+` stops at the attribute delimiter, matching WPF L163
        // (`bfWebToken=([^;]+)`).
        //
        // **Intentional divergence from WPF**: the `(?i)` flag makes
        // the cookie-name match case-insensitive. WPF is case-sensitive,
        // which is a latent bug — a middlebox or a future server change
        // that emits `BFWebToken=…` would silently miss the token and
        // surface `LoginNoWebtoken`. Our leniency strictly widens the
        // accept set (every cookie WPF would capture, we capture too)
        // and is exercised by the `case_insensitive_cookie_name` unit
        // test below.
        Regex::new(r"(?i)bfWebToken=([^;]+)").expect("bfWebToken regex must compile")
    });
    re.captures(cookie_header)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_token_from_minimal_cookie() {
        assert_eq!(
            scan_bfwebtoken("bfWebToken=abc123").as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn scans_token_with_standard_attributes() {
        let header =
            "bfWebToken=xyz789; Path=/; Domain=.beanfun.com; HttpOnly; Secure; SameSite=Lax";
        assert_eq!(scan_bfwebtoken(header).as_deref(), Some("xyz789"));
    }

    #[test]
    fn case_insensitive_cookie_name() {
        assert_eq!(
            scan_bfwebtoken("BFWEBTOKEN=casecheck; Path=/").as_deref(),
            Some("casecheck")
        );
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(scan_bfwebtoken("OtherCookie=foo; Path=/"), None);
        assert_eq!(scan_bfwebtoken(""), None);
    }
}
