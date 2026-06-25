//! TW Regular reCAPTCHA-detection step — `GET Login/InitLogin`.
//!
//! As of 2026-06-25 the `login.beanfun.com/Login/Index` SPA gates the
//! account/password sub-form behind a **Google reCAPTCHA Enterprise v2
//! checkbox** whenever the server decides the client looks suspicious
//! (IP reputation / connection fingerprint). The SPA learns this from
//! `GET /Login/InitLogin`, whose JSON `ResultData.IsRecaptcha` flag
//! tells it whether to render the "I'm not a robot" widget and require
//! a token on the subsequent `CheckAccountType` / `AccountLogin` POSTs.
//!
//! Our headless `reqwest` flow cannot solve a reCAPTCHA v2 challenge
//! (it needs a real browser, a human click, and a token bound to
//! beanfun's own domain). So the TW Regular orchestrator calls this
//! step right after `get_login_index` to find out whether reCAPTCHA is
//! required; when it is, the flow bails to an interactive WebView login
//! instead of attempting the headless POSTs (which would be rejected).
//!
//! # Why a separate step rather than reusing `qr_init`'s InitLogin call
//!
//! [`super::init_qr_login`] already performs a `GET Login/InitLogin`,
//! but its parsing is tightly coupled to the QR sub-flow (it requires
//! `Result == 0` and a non-empty `ResultData.QRImage`, raising
//! [`LoginError::QrInitResultError`] otherwise). QR login is **not**
//! affected by reCAPTCHA — it authenticates by app-scan, with no
//! password sub-form — so we deliberately leave `init_qr_login`
//! untouched and pay for one small duplicated GET here rather than
//! risk regressing a working flow.
//!
//! # Request shape
//!
//! Mirrors what the real SPA (and `init_qr_login`) sends: a JSON GET to
//! `Login/InitLogin?pSKey={skey}` with `Accept: application/json`,
//! `Referer: {Login/Index URL}`, `X-Requested-With: XMLHttpRequest`,
//! and an `Origin` of the login host.

use reqwest::header;
use serde::Deserialize;

use crate::services::beanfun::{BeanfunClient, LoginError};

/// Subset of the `InitLogin` JSON envelope we care about for
/// reCAPTCHA detection. Every other field the server returns
/// (`QRImage`, `IsHK`, `RecaptchaV2PublicKey`, …) is ignored — the
/// public reCAPTCHA site key is rendered by beanfun's own page in the
/// WebView leg, so the backend never needs it.
#[derive(Debug, Deserialize)]
struct InitLoginResponse {
    #[serde(rename = "ResultData")]
    result_data: Option<InitLoginResultData>,
}

#[derive(Debug, Deserialize, Default)]
struct InitLoginResultData {
    /// `true` when the server requires a reCAPTCHA token on the
    /// following `CheckAccountType` / `AccountLogin` calls. Defaults to
    /// `false` when the field is absent so a response-shape change
    /// degrades to the existing headless flow rather than a hard error.
    #[serde(rename = "IsRecaptcha", default)]
    is_recaptcha: bool,
}

/// Ask the server whether the account/password login requires a
/// reCAPTCHA challenge for this attempt.
///
/// Returns `Ok(true)` **only** when the response positively reports
/// `ResultData.IsRecaptcha == true`. **Every** other outcome —
/// transport failure, non-2xx status, oversized / unreadable body,
/// missing field, missing `ResultData`, non-JSON body — degrades to
/// `Ok(false)`.
///
/// # Why fully best-effort (never `Err` on a runtime condition)
///
/// reCAPTCHA detection is an **additive pre-check**, not a login step:
/// `false` simply routes the caller through the unchanged headless flow
/// (`check_account_type` → `account_login`). Bubbling a transport /
/// status error up from here would make a transient `InitLogin` blip
/// fail the *entire* login — strictly worse than today, where the flow
/// has no `InitLogin` call at all. If the network is genuinely down the
/// subsequent headless steps surface the real error anyway, so swallowing
/// it here loses no diagnostic signal. Only a confirmed
/// `IsRecaptcha == true` diverts to the interactive WebView path.
///
/// The signature stays `Result<bool, _>` (rather than `bool`) only so
/// the `?` on the URL builder — a programming error, not a runtime
/// condition — can propagate; all runtime paths return `Ok`.
///
/// `index_url` is the `Login/Index?pSKey=…` URL produced by
/// [`super::get_login_index`]; it is sent verbatim as the `Referer`.
pub async fn check_recaptcha_required(
    client: &BeanfunClient,
    skey: &str,
    index_url: &str,
) -> Result<bool, LoginError> {
    let init_url = client.login_url_with_skey("Login/InitLogin", skey)?;
    let origin = client
        .config()
        .endpoints
        .login_base
        .origin()
        .ascii_serialization();

    let resp = match client
        .http()
        .get(init_url)
        .header(header::ACCEPT, "application/json, text/plain, */*")
        .header(header::REFERER, index_url)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Origin", origin)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(error) => {
            tracing::warn!(
                step = "InitLogin.Recaptcha",
                error = %error,
                "InitLogin request failed; assuming reCAPTCHA not required",
            );
            return Ok(false);
        }
    };

    if !resp.status().is_success() {
        tracing::warn!(
            step = "InitLogin.Recaptcha",
            status = %resp.status(),
            "InitLogin returned non-success; assuming reCAPTCHA not required",
        );
        return Ok(false);
    }

    let body = match client.bounded_text(resp).await {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(
                step = "InitLogin.Recaptcha",
                error = %error,
                "InitLogin body read failed; assuming reCAPTCHA not required",
            );
            return Ok(false);
        }
    };

    match serde_json::from_str::<InitLoginResponse>(&body) {
        Ok(parsed) => Ok(parsed.result_data.map(|d| d.is_recaptcha).unwrap_or(false)),
        Err(error) => {
            // Not a hard failure: fall through to the headless flow.
            tracing::warn!(
                step = "InitLogin.Recaptcha",
                error = %error,
                "InitLogin body was not parseable JSON; assuming reCAPTCHA not required",
            );
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_is_recaptcha_true() {
        let body = r#"{"ResultData":{"IsRecaptcha":true,"RecaptchaV2PublicKey":"6Lx"}}"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.result_data.expect("ResultData present").is_recaptcha);
    }

    #[test]
    fn parses_is_recaptcha_false() {
        let body = r#"{"ResultData":{"IsRecaptcha":false}}"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(!parsed.result_data.expect("ResultData present").is_recaptcha);
    }

    #[test]
    fn missing_is_recaptcha_field_defaults_to_false() {
        // QR-shaped ResultData with no IsRecaptcha key → default false.
        let body = r#"{"ResultData":{"QRImage":"iVBOR","IsHK":false}}"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(!parsed.result_data.expect("ResultData present").is_recaptcha);
    }

    #[test]
    fn missing_result_data_is_none() {
        let body = r#"{"Result":0,"ResultCode":1}"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.result_data.is_none());
    }
}
