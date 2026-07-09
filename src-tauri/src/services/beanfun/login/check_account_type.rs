//! Step 2 of the TW Regular flow: `POST Login/CheckAccountType`.
//!
//! The server uses this call to decide whether a captcha challenge is
//! required for the following [`account_login`](super::account_login())
//! step. On the non-reCAPTCHA path we forward whatever captcha token the
//! server returns (empty string when not required) verbatim into the next
//! payload — same as WPF `TwRegularLogin` L57-78.
//!
//! # reCAPTCHA (issue #313 / #315 / #318 — token-replay)
//!
//! As of 2026-06 the server can gate this POST behind a Google reCAPTCHA
//! Enterprise challenge. Rather than probing `Login/InitLogin` up front,
//! we follow an **empty-first** strategy (task spec §1): send an empty
//! `Captcha` token and inspect the response. When the server signals that
//! a token is required — `ResultData.IsRecaptcha == true` or a
//! "機器人"/"我不是機器人" message — we surface
//! [`CheckAccountOutcome::RecaptchaRequired`] so the caller can solve the
//! widget on beanfun's own origin and **retry this same step** with the
//! solved token (which is passed back in via `captcha_token`).

use serde::{Deserialize, Serialize};

use super::{
    apply_json_headers, deserialize_jtoken_to_string, ensure_success, message_demands_recaptcha,
    parse_step_json,
};
use crate::services::beanfun::{BeanfunClient, LoginError};

/// JSON body of the `CheckAccountType` POST.
///
/// Mirrors the JObject WPF builds at L58-63. Borrows all fields so we can
/// construct the struct without cloning the caller's strings.
#[derive(Serialize)]
struct CheckAccountTypeRequest<'a> {
    #[serde(rename = "Account")]
    account: &'a str,
    /// The reCAPTCHA token to replay. Empty on the first (empty-first)
    /// attempt; a solved-on-origin token on a reCAPTCHA retry.
    #[serde(rename = "Captcha")]
    captcha: &'a str,
    #[serde(rename = "__RequestVerificationToken")]
    verification_token: &'a str,
}

/// Relevant subset of the JSON response. The server returns a larger
/// envelope (ResultCode / Message / etc.); we read `ResultData.Captcha`
/// (server-provided passthrough token) and the reCAPTCHA signals.
#[derive(Deserialize)]
struct CheckAccountTypeResponse {
    #[serde(
        rename = "Message",
        default,
        deserialize_with = "deserialize_jtoken_to_string"
    )]
    message: Option<String>,
    #[serde(rename = "ResultData")]
    result_data: Option<CheckAccountTypeData>,
}

#[derive(Deserialize, Default)]
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
    /// `true` when the server requires a reCAPTCHA token on this call
    /// for the current attempt. Absent → `false` (unchanged behaviour).
    #[serde(rename = "IsRecaptcha", default)]
    is_recaptcha: bool,
}

/// Result of a `CheckAccountType` POST.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckAccountOutcome {
    /// The step succeeded; the (possibly empty) server-provided captcha
    /// token is forwarded verbatim into the following `AccountLogin`.
    Proceed { server_captcha: String },
    /// The server demands a reCAPTCHA token. The caller solves the widget
    /// on beanfun's origin and retries this step with the solved token.
    RecaptchaRequired,
}

/// POST the CheckAccountType request.
///
/// `captcha_token` is the reCAPTCHA token to replay — pass `""` on the
/// empty-first attempt, or a solved-on-origin token when retrying after a
/// [`CheckAccountOutcome::RecaptchaRequired`].
///
/// `index_url` is used as `Referer`; supply the [`super::LoginIndex::index_url`]
/// produced by the preceding [`super::get_login_index`] call verbatim.
pub async fn check_account_type(
    client: &BeanfunClient,
    skey: &str,
    account: &str,
    captcha_token: &str,
    verification_token: &str,
    index_url: &str,
) -> Result<CheckAccountOutcome, LoginError> {
    let url = client.login_url_with_skey("Login/CheckAccountType", skey)?;
    let body = CheckAccountTypeRequest {
        account,
        captcha: captcha_token,
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
        return Ok(CheckAccountOutcome::Proceed {
            server_captcha: String::new(),
        });
    }

    let parsed: CheckAccountTypeResponse = parse_step_json(&text, "CheckAccountType")?;

    let message = parsed.message.clone().unwrap_or_default();
    let (is_recaptcha, server_captcha) = match parsed.result_data {
        Some(d) => (d.is_recaptcha, d.captcha.unwrap_or_default()),
        None => (false, String::new()),
    };

    // Diagnostic parity with `AccountLogin.Verdict` (#313/#315/#318). The
    // `IsRecaptcha` flag is logged but deliberately NOT acted on — only the
    // robot message gates reCAPTCHA (see `classify_check_response`).
    tracing::info!(
        step = "CheckAccountType.Verdict",
        is_recaptcha = is_recaptcha,
        captcha_sent = !captcha_token.is_empty(),
        message = %message,
        "CheckAccountType server response classified"
    );

    Ok(classify_check_response(&message, server_captcha))
}

/// Pure mapping from the parsed response fields to a [`CheckAccountOutcome`].
/// Kept separate so the reCAPTCHA-detection table is unit-testable without
/// a mock HTTP server.
///
/// Escalates to a reCAPTCHA solve **only** when the server asks for the
/// "我不是機器人" check in the message. The bare `IsRecaptcha` flag is not a
/// trigger — live traffic sets it even on advance-check responses that log
/// in fine without a token (see `account_login` for the same rule and the
/// #313/#315/#318 rationale).
fn classify_check_response(message: &str, server_captcha: String) -> CheckAccountOutcome {
    if message_demands_recaptcha(message) {
        CheckAccountOutcome::RecaptchaRequired
    } else {
        CheckAccountOutcome::Proceed { server_captcha }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> CheckAccountOutcome {
        let r: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        let message = r.message.clone().unwrap_or_default();
        let server_captcha = r.result_data.and_then(|d| d.captcha).unwrap_or_default();
        classify_check_response(&message, server_captcha)
    }

    /// Reach into the private DTO to assert the JToken coercion
    /// actually fires for this call site — belt-and-braces on top of
    /// the helper-level unit tests in `login/mod.rs`.
    #[test]
    fn captcha_integer_response_parses_via_jtoken_coercion() {
        assert_eq!(
            parse(r#"{"ResultData":{"Captcha":0}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: "0".to_owned()
            }
        );
    }

    #[test]
    fn captcha_string_response_still_parses() {
        assert_eq!(
            parse(r#"{"ResultData":{"Captcha":"TOKEN"}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: "TOKEN".to_owned()
            }
        );
    }

    #[test]
    fn captcha_null_response_yields_empty_proceed() {
        assert_eq!(
            parse(r#"{"ResultData":{"Captcha":null}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: String::new()
            }
        );
    }

    #[test]
    fn missing_result_data_yields_empty_proceed() {
        assert_eq!(
            parse(r#"{"ResultCode":"1"}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: String::new()
            }
        );
    }

    #[test]
    fn bare_is_recaptcha_flag_does_not_demand() {
        // The flag alone (no robot message) must NOT escalate — it is set
        // even on advance-check responses that log in without a token.
        assert_eq!(
            parse(r#"{"ResultData":{"IsRecaptcha":true}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: String::new()
            }
        );
    }

    #[test]
    fn robot_message_demands_recaptcha() {
        // The 機器人 message is the only thing that escalates.
        assert_eq!(
            parse(r#"{"Message":"請點選「我不是機器人」","ResultData":{"Captcha":""}}"#),
            CheckAccountOutcome::RecaptchaRequired
        );
    }

    #[test]
    fn flag_with_non_robot_message_proceeds() {
        // IsRecaptcha set but the message isn't a robot prompt → proceed
        // instead of opening the widget (#313/#315/#318 loop guard).
        assert_eq!(
            parse(r#"{"Message":"資料驗證錯誤","ResultData":{"IsRecaptcha":true,"Captcha":"X"}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: "X".to_owned()
            }
        );
    }
}
