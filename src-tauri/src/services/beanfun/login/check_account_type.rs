//! Step 2 of the TW Regular flow: `POST Login/CheckAccountType`.
//!
//! The server uses this call to decide whether a captcha challenge is
//! required for the following [`account_login`](super::account_login())
//! step. We forward whatever captcha token the server returns (empty
//! string when not required) verbatim into the next payload.
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L57-78.

use serde::{Deserialize, Serialize};

use super::{apply_json_headers, deserialize_jtoken_to_string, ensure_success, parse_step_json};
use crate::services::beanfun::{BeanfunClient, LoginError};

/// JSON body of the `CheckAccountType` POST.
///
/// Mirrors the JObject WPF builds at L58-63. Borrows all fields so we can
/// construct the struct without cloning the caller's strings.
#[derive(Serialize)]
struct CheckAccountTypeRequest<'a> {
    #[serde(rename = "Account")]
    account: &'a str,
    /// Always empty on this call — WPF hard-codes `""` here, and the
    /// server only populates a captcha token on the response side.
    #[serde(rename = "Captcha")]
    captcha: &'a str,
    #[serde(rename = "__RequestVerificationToken")]
    verification_token: &'a str,
}

/// Relevant subset of the JSON response. The server returns a larger
/// envelope (ResultCode / Message / etc.) but only `ResultData.Captcha`
/// is consumed by WPF, so we ignore the rest.
#[derive(Deserialize)]
struct CheckAccountTypeResponse {
    #[serde(rename = "ResultData")]
    result_data: Option<CheckAccountTypeData>,
}

#[derive(Deserialize)]
struct CheckAccountTypeData {
    /// WPF reads this via `JToken.ToString() ?? ""` (L77) — the
    /// server has been observed to send this as either a string or
    /// a (zero-valued) integer, so we use the shared JToken-style
    /// coercion helper. See [`deserialize_jtoken_to_string`] for the
    /// full rationale.
    #[serde(
        rename = "Captcha",
        default,
        deserialize_with = "deserialize_jtoken_to_string"
    )]
    captcha: Option<String>,
}

/// POST the CheckAccountType request and return the captcha token (empty
/// string when the server did not require one).
///
/// `index_url` is used as `Referer`; supply the [`super::LoginIndex::index_url`]
/// produced by the preceding [`super::get_login_index`] call verbatim.
pub async fn check_account_type(
    client: &BeanfunClient,
    skey: &str,
    account: &str,
    verification_token: &str,
    index_url: &str,
) -> Result<String, LoginError> {
    let url = client.login_url_with_skey("Login/CheckAccountType", skey)?;
    let body = CheckAccountTypeRequest {
        account,
        captcha: "",
        verification_token,
    };

    let rb = apply_json_headers(client.http().post(url), verification_token, index_url);
    let resp = rb.json(&body).send().await?;

    ensure_success(&resp, "CheckAccountType")?;
    let text = client.bounded_text(resp).await?;

    // WPF sniff: if the body does not begin with `{`, treat it as "no
    // captcha" rather than a JSON parse error. This defends against the
    // server returning an HTML error page — rare, but we stay compatible
    // with the legacy client which simply falls through to `captchaToken
    // = ""` in that case.
    if !text.trim_start().starts_with('{') {
        return Ok(String::new());
    }

    let parsed: CheckAccountTypeResponse = parse_step_json(&text, "CheckAccountType")?;
    Ok(parsed
        .result_data
        .and_then(|d| d.captcha)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reach into the private DTO to assert the JToken coercion
    /// actually fires for this call site — belt-and-braces on top of
    /// the helper-level unit tests in `login/mod.rs`.
    #[test]
    fn captcha_integer_response_parses_via_jtoken_coercion() {
        let body = r#"{"ResultData":{"Captcha":0}}"#;
        let parsed: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(
            parsed.result_data.and_then(|d| d.captcha).as_deref(),
            Some("0")
        );
    }

    #[test]
    fn captcha_string_response_still_parses() {
        let body = r#"{"ResultData":{"Captcha":"TOKEN"}}"#;
        let parsed: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(
            parsed.result_data.and_then(|d| d.captcha).as_deref(),
            Some("TOKEN")
        );
    }

    #[test]
    fn captcha_null_response_yields_none() {
        let body = r#"{"ResultData":{"Captcha":null}}"#;
        let parsed: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.result_data.and_then(|d| d.captcha).is_none());
    }

    #[test]
    fn missing_result_data_yields_none() {
        let body = r#"{"ResultCode":"1"}"#;
        let parsed: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.result_data.is_none());
    }
}
