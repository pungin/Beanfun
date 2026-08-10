//! Shared step: `GET Login/SendLogin`.
//!
//! After the credential / QR-handshake step succeeds the server hands
//! back an HTML page whose `<form>` holds all the opaque session
//! tokens the portal expects when the browser POSTs over to
//! `beanfun_block/bflogin/return.aspx`. WPF scrapes every non-submit
//! `<input>` from that form — we do the same via
//! [`extract_hidden_inputs`].
//!
//! WPF references:
//! - TW Regular flow: `BeanfunClient.Login.cs::TwRegularLogin` L114-146
//!   (Accept = `text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8`).
//! - QR flow: `BeanfunClient.Login.cs::QRCodeLogin` L543-580
//!   (Accept = `text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8`).
//!
//! Same endpoint, same response shape, same error mapping — only the
//! Accept string differs between the two callers. We surface that as
//! an explicit `accept` parameter so the WPF wire shape stays
//! byte-identical at every callsite without the helper having to
//! guess which flow invoked it.
//!
//! # Why parameterise instead of hard-coding the union of both?
//!
//! Servers in practice don't read past the leading `text/html` token,
//! so either string would "work" for either caller. We still mirror
//! WPF exactly because the rule we hold ourselves to is byte-level
//! parity with the reference implementation — a future fingerprint
//! check or proxy normalisation could observe the difference.

use reqwest::header;

use super::{ensure_success, truncate_chars, BODY_LOG_PREVIEW_CHARS};
use crate::core::parser::{extract_hidden_inputs, HiddenInput};
use crate::services::beanfun::{BeanfunClient, LoginError};

/// GET the SendLogin page and return its hidden form payload.
///
/// `accept` is the exact `Accept` header string the calling flow
/// sends — TW Regular and QR pass different values; see module docs
/// for the WPF references.
///
/// Returns [`LoginError::SendLoginNoFormData`] when the scrape finds
/// zero usable inputs (empty body, error page, or unexpected markup).
/// That mirrors the `errmsg = "SendLoginNoFormData"` branch at WPF L140.
pub async fn send_login(
    client: &BeanfunClient,
    index_url: &str,
    accept: &str,
) -> Result<Vec<HiddenInput>, LoginError> {
    let url = client.login_url("Login/SendLogin")?;

    let resp = client
        .http()
        .get(url)
        .header(header::ACCEPT, accept)
        .header(header::REFERER, index_url)
        .send()
        .await?;

    ensure_success(&resp, "SendLogin")?;
    let body = client.bounded_text(resp).await?;
    let inputs = extract_hidden_inputs(&body);
    if inputs.is_empty() {
        // Diagnostic: scraping zero hidden inputs usually means one of
        // (a) upstream credential step silently failed and the server
        // handed us an error / redirect page, (b) Beanfun served an
        // anti-bot interstitial, or (c) the form markup shape changed.
        // The bounded body preview gives the operator enough context
        // to tell them apart without blowing up log volume. WPF's
        // equivalent (`errmsg = "SendLoginNoFormData"`) is silent
        // about the body, so this is a parity-superset.
        tracing::warn!(
            step = "SendLogin",
            body_preview = %crate::core::redact::scrub(truncate_chars(&body, BODY_LOG_PREVIEW_CHARS)),
            "SendLogin scrape returned 0 hidden inputs"
        );
        return Err(LoginError::SendLoginNoFormData);
    }
    Ok(inputs)
}
