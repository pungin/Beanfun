//! Obtain the portal session key (`pSKey` on TW, OTP1 span on HK).
//!
//! Both regions kick off the same way: hit
//! `…/beanfun_block/bflogin/default.aspx?service=999999_T0` on the portal
//! host. The ways they surface the key differ:
//!
//! | Region | Where the key lives                                                   |
//! |--------|-----------------------------------------------------------------------|
//! | TW     | Final redirected URL query parameter, matched by `[sp][Ss]?[Kk]ey=(…)` |
//! | HK     | `<span id="ctl00_ContentPlaceHolder1_lblOtp1">…</span>` in the body    |
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::GetSessionkey`
//! (L702-743).

use std::sync::OnceLock;

use regex::Regex;

use super::{truncate_chars, BODY_LOG_PREVIEW_CHARS};
use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion};

/// Region-aware session-key retrieval. Delegates to the TW or HK helper
/// based on `client.config().region`.
pub async fn get_session_key(client: &BeanfunClient) -> Result<String, LoginError> {
    match client.config().region {
        LoginRegion::TW => get_session_key_tw(client).await,
        LoginRegion::HK => get_session_key_hk(client).await,
    }
}

/// TW: follow the default.aspx redirect and scrape `pSKey` from the final
/// URL. Uses the default (redirect-following) HTTP client.
async fn get_session_key_tw(client: &BeanfunClient) -> Result<String, LoginError> {
    let url = portal_default_url(client)?;
    let resp = client.http().get(url).send().await?;

    // We only need the final URL, but the body must be drained so the
    // underlying connection is released back to the pool cleanly. The
    // bounded read also defends against a hostile server streaming a huge
    // body into the redirected response.
    let final_url = resp.url().clone();
    let _ = client.bounded_text(resp).await?;

    session_key_from_url(final_url.as_str()).ok_or_else(|| {
        // Diagnostic: when the portal-default redirect chain ends on a
        // URL whose query doesn't carry `[sp][Ss]?[Kk]ey=…` we want
        // the operator to see what URL we actually landed on. The URL
        // query may contain user-agent-derived identifiers but not
        // credentials; logging the full URL is safe in this context
        // and is the minimum context needed to tell "Beanfun changed
        // its redirect target" apart from a transient network glitch.
        tracing::warn!(
            step = "GetSessionKey",
            region = ?LoginRegion::TW,
            final_url = %final_url,
            "session key regex did not match the redirected URL"
        );
        LoginError::MissingSessionKey
    })
}

/// HK: the default.aspx response body itself contains an OTP1 span whose
/// text **is** the session key. No redirect chasing required.
async fn get_session_key_hk(client: &BeanfunClient) -> Result<String, LoginError> {
    let url = portal_default_url(client)?;
    let resp = client.http().get(url).send().await?;
    let body = client.bounded_text(resp).await?;

    if body.is_empty() {
        return Err(LoginError::EmptyResponse);
    }

    session_key_from_hk_body(&body).ok_or_else(|| {
        // Diagnostic: when the HK OTP1 span regex doesn't match we log
        // a bounded body preview so the operator can tell apart
        // (a) an anti-bot / rate-limit interstitial, (b) an error
        // page, and (c) a new markup shape where the span id changed.
        // Preview is bounded to the shared `BODY_LOG_PREVIEW_CHARS`
        // limit and cut at a UTF-8 boundary by `truncate_chars`.
        tracing::warn!(
            step = "GetSessionKey",
            region = ?LoginRegion::HK,
            body_preview = %truncate_chars(&body, BODY_LOG_PREVIEW_CHARS),
            "OTP1 span regex did not match the response body"
        );
        LoginError::MissingSessionKey
    })
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Build the `beanfun_block/bflogin/default.aspx?service=999999_T0` URL on
/// the portal host. Factored out so TW / HK share the exact same path.
fn portal_default_url(client: &BeanfunClient) -> Result<url::Url, LoginError> {
    client
        .config()
        .endpoints
        .portal_base
        .join("beanfun_block/bflogin/default.aspx?service=999999_T0")
        .map_err(|e| LoginError::InvalidUrl(format!("default.aspx URL join failed: {e}")))
}

/// Extract the session key value from a TW portal redirect URL.
///
/// Identical to the WPF pattern `[sp][Ss]?[Kk]ey=([^&]+)` — matches
/// `pSKey=…` (the real shape), `sKey=…`, `ssKey=…`, etc., and stops at
/// `&` or end-of-string.
fn session_key_from_url(url: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"[sp][Ss]?[Kk]ey=([^&]+)").expect("TW skey regex must compile")
    });
    re.captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

/// Extract the session key value from an HK portal response body.
///
/// Identical to the WPF pattern
/// `<span id="ctl00_ContentPlaceHolder1_lblOtp1">(.*)</span>`.
fn session_key_from_hk_body(body: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"<span id="ctl00_ContentPlaceHolder1_lblOtp1">(.*)</span>"#)
            .expect("HK OTP1 span regex must compile")
    });
    re.captures(body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Unit tests here are **pure** — they cover the regex helpers only.
    //! End-to-end wiremock tests live in `tests/session_key.rs` so they
    //! can share a common runtime setup across chunks.

    use super::*;

    #[test]
    fn tw_regex_matches_capital_pskey() {
        let url =
            "https://tw.newlogin.beanfun.com/login/id-pass.aspx?service=999999_T0&pSKey=ABCDEF123";
        assert_eq!(
            session_key_from_url(url).as_deref(),
            Some("ABCDEF123"),
            "production URLs use capital `pSKey`; regex must match"
        );
    }

    #[test]
    fn tw_regex_stops_at_ampersand() {
        let url = "https://host/path?pSKey=TOKEN&next=foo";
        assert_eq!(session_key_from_url(url).as_deref(), Some("TOKEN"));
    }

    #[test]
    fn tw_regex_accepts_lowercase_variants() {
        // The WPF regex `[sp][Ss]?[Kk]ey` is permissive by design so we
        // lock in parity.
        assert_eq!(
            session_key_from_url("?skey=LOW").as_deref(),
            Some("LOW"),
            "lowercase skey"
        );
        assert_eq!(
            session_key_from_url("?pKey=PK").as_deref(),
            Some("PK"),
            "pKey with no middle S"
        );
    }

    #[test]
    fn tw_regex_returns_none_when_absent() {
        assert_eq!(
            session_key_from_url("https://host/no-key-here"),
            None,
            "absent key yields None so caller can raise MissingSessionKey"
        );
    }

    #[test]
    fn hk_regex_extracts_inner_span_text() {
        let body = r#"<html><body><span id="ctl00_ContentPlaceHolder1_lblOtp1">HK_OTP1_VAL</span></body></html>"#;
        assert_eq!(
            session_key_from_hk_body(body).as_deref(),
            Some("HK_OTP1_VAL")
        );
    }

    #[test]
    fn hk_regex_returns_none_when_span_absent() {
        let body = "<html><body>no otp1 span here</body></html>";
        assert_eq!(session_key_from_hk_body(body), None);
    }
}
