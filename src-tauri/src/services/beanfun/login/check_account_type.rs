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
//! `Captcha` token and inspect the response.
//!
//! A reCAPTCHA demand only counts when the step **actually failed**
//! (`ResultCode != 1`). beanfun echoes `ResultData.IsRecaptcha == true` as
//! a *session-level advisory* (mirroring `InitLogin`'s risk verdict) even on
//! a `ResultCode == 1` success — its own login page never reads that field
//! on `CheckAccountType`, it only checks `ResultCode`. Popping a widget on
//! that advisory is the "useless pre-password reCAPTCHA": the account
//! check already succeeded, so we forward the server captcha and proceed.
//! MapleLink guards this the same way with its `result_code != 1` check.
//!
//! When the step *fails* and the server signals a token is required —
//! `ResultData.IsRecaptcha == true` or a "機器人"/"我不是機器人" message —
//! we surface [`CheckAccountOutcome::RecaptchaRequired`] so the caller can
//! solve the widget on beanfun's own origin and **retry this same step**
//! with the solved token (which is passed back in via `captcha_token`).

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
    /// beanfun's success/failure code. `"1"` == success (the account check
    /// passed); anything else is a failure. Read via the shared JToken-style
    /// coercion because the server sends it as either an integer or a string.
    /// Absent → treated as success (matches MapleLink's `.unwrap_or(1)`), so a
    /// response-shape change never spuriously pops a widget.
    #[serde(
        rename = "ResultCode",
        default,
        deserialize_with = "deserialize_jtoken_to_string"
    )]
    result_code: Option<String>,
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

    // `Content-Type: application/json; charset=utf-8` to match beanfun's own
    // login page + MapleLink (reqwest's `.json()` omits the charset — a
    // bot-tell). See `account_login` for the full rationale.
    let body_bytes = serde_json::to_vec(&body).expect("CheckAccountTypeRequest serializes");
    let rb = apply_json_headers(client.http().post(url), verification_token, index_url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )
        .body(body_bytes);
    let resp = rb.send().await?;

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

    let succeeded = result_code_is_success(parsed.result_code.as_deref());
    let is_recaptcha = parsed
        .result_data
        .as_ref()
        .map(|d| d.is_recaptcha)
        .unwrap_or(false);

    // Diagnostic: the exact server verdict for this step. Parity with
    // `AccountLogin.Verdict` — it's the only way to see whether beanfun is
    // genuinely gating the *account* step behind reCAPTCHA (a risk verdict on
    // our request fingerprint) vs. sailing through like MapleLink on the same
    // IP. `account` is non-secret; the captcha token itself is never logged.
    tracing::info!(
        step = "CheckAccountType.Verdict",
        account_id = %account,
        result_code = parsed.result_code.as_deref().unwrap_or(""),
        succeeded,
        is_recaptcha,
        captcha_sent = !captcha_token.is_empty(),
        message = %truncate_for_log(parsed.message.as_deref().unwrap_or("")),
        "CheckAccountType server response classified"
    );

    Ok(classify_check_response(
        succeeded,
        is_recaptcha,
        parsed.message.as_deref().unwrap_or_default(),
        captcha_token.is_empty(),
        parsed
            .result_data
            .and_then(|d| d.captcha)
            .unwrap_or_default(),
    ))
}

/// Borrow at most ~120 chars of a server message for a log line (won't split
/// a CJK codepoint). Mirrors the helper in [`super::account_login`].
fn truncate_for_log(s: &str) -> &str {
    match s.char_indices().nth(120) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Whether `CheckAccountType` accepted the account check. `"1"` is beanfun's
/// success code; a missing code defaults to success (MapleLink's
/// `.unwrap_or(1)` semantics) so a response-shape change degrades to "proceed"
/// rather than a spurious reCAPTCHA pop.
fn result_code_is_success(result_code: Option<&str>) -> bool {
    match result_code {
        Some(code) => code == "1",
        None => true,
    }
}

/// Pure mapping from the parsed response fields to a [`CheckAccountOutcome`].
/// Kept separate so the reCAPTCHA-detection table is unit-testable without
/// a mock HTTP server.
///
/// A reCAPTCHA solve is only demanded when the step **failed** (`!succeeded`)
/// *and* the server either asks for the "我不是機器人" check or sets
/// `IsRecaptcha` on an empty-first probe (`captcha_empty`). On a `ResultCode`
/// success the server accepted the account check — a lingering `IsRecaptcha`
/// advisory there is the useless pre-password reCAPTCHA, so we forward
/// the server captcha and proceed. A flag lingering alongside a real error
/// after a token was already replayed likewise surfaces as the error instead
/// of looping (see `account_login` for the same rule / the #313/#315/#318
/// rationale).
fn classify_check_response(
    succeeded: bool,
    is_recaptcha: bool,
    message: &str,
    captcha_empty: bool,
    server_captcha: String,
) -> CheckAccountOutcome {
    if !succeeded && (message_demands_recaptcha(message) || (is_recaptcha && captcha_empty)) {
        CheckAccountOutcome::RecaptchaRequired
    } else {
        CheckAccountOutcome::Proceed { server_captcha }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> CheckAccountOutcome {
        // Empty-first probe (no token yet) — the common escalation path.
        parse_with(body, true)
    }

    fn parse_with(body: &str, captcha_empty: bool) -> CheckAccountOutcome {
        let r: CheckAccountTypeResponse = serde_json::from_str(body).expect("valid JSON");
        classify_check_response(
            result_code_is_success(r.result_code.as_deref()),
            r.result_data
                .as_ref()
                .map(|d| d.is_recaptcha)
                .unwrap_or(false),
            r.message.as_deref().unwrap_or_default(),
            captcha_empty,
            r.result_data.and_then(|d| d.captcha).unwrap_or_default(),
        )
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
    fn is_recaptcha_flag_on_failure_demands_recaptcha() {
        // The genuine gate: the account check FAILED (ResultCode 0) and the
        // server flagged IsRecaptcha on our empty-first probe.
        assert_eq!(
            parse(r#"{"ResultCode":0,"ResultData":{"IsRecaptcha":true}}"#),
            CheckAccountOutcome::RecaptchaRequired
        );
    }

    #[test]
    fn is_recaptcha_flag_on_success_is_the_useless_pre_password_widget() {
        // ResultCode 1 means the account check succeeded. A lingering
        // IsRecaptcha advisory must NOT pop a widget — we forward the (here
        // empty) server captcha and proceed straight to AccountLogin.
        assert_eq!(
            parse(r#"{"ResultCode":1,"ResultData":{"IsRecaptcha":true,"Captcha":"srv"}}"#),
            CheckAccountOutcome::Proceed {
                server_captcha: "srv".to_owned()
            }
        );
    }

    #[test]
    fn robot_message_on_failure_demands_recaptcha() {
        // Even without the flag, a 機器人 message on a failed check escalates.
        assert_eq!(
            parse(r#"{"ResultCode":0,"Message":"請點選「我不是機器人」","ResultData":{"Captcha":""}}"#),
            CheckAccountOutcome::RecaptchaRequired
        );
    }

    #[test]
    fn lingering_flag_after_token_does_not_loop() {
        // IsRecaptcha still set but a token was already replayed
        // (captcha_empty=false) and the message isn't a robot prompt →
        // proceed instead of re-opening the widget (#313/#315/#318).
        assert_eq!(
            parse_with(
                r#"{"ResultCode":0,"Message":"資料驗證錯誤","ResultData":{"IsRecaptcha":true,"Captcha":"X"}}"#,
                false,
            ),
            CheckAccountOutcome::Proceed {
                server_captcha: "X".to_owned()
            }
        );
    }
}
