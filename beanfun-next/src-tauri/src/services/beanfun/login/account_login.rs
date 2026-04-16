//! Step 3 of the TW Regular flow: `POST Login/AccountLogin`.
//!
//! This is where credentials are actually validated. The server
//! multiplexes its outcome on a pair of string codes in the JSON
//! response; the truth table below mirrors WPF's `if/else` chain
//! **exactly** — note the asymmetry on `ResultCode == "1"` where only
//! `Result == "1"` triggers advance-check and *any other* `Result`
//! (including `""`, `null`, `"0"`, `"2"`, …) is treated as success:
//!
//! | `ResultCode` | `Result`        | Meaning                                       |
//! |:------------:|:----------------|:----------------------------------------------|
//! | `"1"`        | `"1"`           | advance-check needed (captcha / email)        |
//! | `"1"`        | anything else   | success – proceed to `SendLogin`              |
//! | `"2"`        | *any*           | advance-check needed, URL in `ResultMessage`  |
//! | anything else| *any*           | `ResultMessage` surfaced verbatim to the user |
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L101-192. The lenient "non-`1` is success" rule is deliberate in
//! WPF — preserving it keeps us forward-compatible with any future
//! success code the server decides to coin.
//!
//! # Sensitive data note
//!
//! The password is serialised into the JSON body via serde, which
//! allocates a `String` / `Vec<u8>` that neither we nor `reqwest` can
//! zero on drop. Zeroising the **request body** is a larger design
//! change (custom serde adapter + intercepting reqwest's internal
//! buffer) that we explicitly defer to a later pass; the zeroise-on-drop
//! of [`Credentials`] itself
//! still prevents the plaintext from lingering on the caller's stack
//! after the call returns.

use serde::{Deserialize, Serialize};

use super::{apply_json_headers, ensure_success};
use crate::services::beanfun::{BeanfunClient, Credentials, LoginError};

#[derive(Serialize)]
struct AccountLoginRequest<'a> {
    #[serde(rename = "Account")]
    account: &'a str,
    #[serde(rename = "Pasw")]
    password: &'a str,
    /// Always `false` — WPF hard-codes it. `IsMobile=true` is used by
    /// the mobile app flavour of the API and is out of scope here.
    #[serde(rename = "IsMobile")]
    is_mobile: bool,
    #[serde(rename = "Captcha")]
    captcha: &'a str,
    #[serde(rename = "__RequestVerificationToken")]
    verification_token: &'a str,
}

#[derive(Deserialize)]
struct AccountLoginResponse {
    #[serde(rename = "ResultCode")]
    result_code: Option<String>,
    #[serde(rename = "Result")]
    result: Option<String>,
    #[serde(rename = "ResultMessage")]
    result_message: Option<String>,
}

/// POST credentials to `AccountLogin` and return `Ok(())` on success.
///
/// Maps the four server-side outcomes to:
/// - `Ok(())` for the happy path,
/// - [`LoginError::AdvanceCheckRequired`] (with or without an external
///   verification URL) for the two advance-check branches,
/// - [`LoginError::ServerMessage`] for anything else, with the exact
///   `ResultMessage` surfaced so the UI can show the Chinese text as-is.
pub async fn account_login(
    client: &BeanfunClient,
    skey: &str,
    creds: &Credentials,
    captcha: &str,
    verification_token: &str,
    index_url: &str,
) -> Result<(), LoginError> {
    let url = client.login_url_with_skey("Login/AccountLogin", skey)?;
    let body = AccountLoginRequest {
        account: &creds.account,
        password: &creds.password,
        is_mobile: false,
        captcha,
        verification_token,
    };

    let rb = apply_json_headers(client.http().post(url), verification_token, index_url);
    let resp = rb.json(&body).send().await?;

    ensure_success(&resp, "AccountLogin")?;
    let text = client.bounded_text(resp).await?;
    let parsed: AccountLoginResponse = serde_json::from_str(&text)?;

    classify_outcome(
        parsed.result_code.as_deref().unwrap_or_default(),
        parsed.result.as_deref().unwrap_or_default(),
        parsed.result_message.unwrap_or_default(),
    )
}

/// Pure mapping from response fields to a `Result`. Kept in its own
/// function so the table above can be unit-tested without spinning up a
/// mock HTTP server.
///
/// The arm ordering is load-bearing: `("1", "1")` must match **before**
/// the `("1", _)` success arm, otherwise we'd short-circuit advance-check
/// into success.
fn classify_outcome(
    result_code: &str,
    result: &str,
    result_message: String,
) -> Result<(), LoginError> {
    match (result_code, result) {
        // WPF L103-107: only the literal string `"1"` triggers advance
        // check inside the `ResultCode == "1"` branch.
        ("1", "1") => Err(LoginError::AdvanceCheckRequired { url: None }),
        // WPF L101-180: every other `Result` value (`"0"`, `""`, `"2"`,
        // missing field, …) is success. The wildcard here is deliberate
        // and mirrors the reference implementation — narrowing it would
        // reject valid responses the server could start returning.
        ("1", _) => Ok(()),
        ("2", _) => {
            // WPF L183-189: only keep the message as a URL when it
            // starts with "http", otherwise fall through to advance
            // check without URL. Anything else is silently dropped —
            // preserving that quirk keeps logs clean of half-baked URLs.
            let url = result_message.starts_with("http").then_some(result_message);
            Err(LoginError::AdvanceCheckRequired { url })
        }
        _ => Err(LoginError::ServerMessage(result_message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    #[test]
    fn result_code_1_result_0_returns_ok() {
        assert!(classify_outcome("1", "0", String::new()).is_ok());
    }

    #[test]
    fn result_code_1_empty_result_returns_ok() {
        // WPF treats missing/empty `Result` as success when
        // `ResultCode == "1"`; we must match that leniency so we don't
        // spuriously reject a future server variant.
        assert!(classify_outcome("1", "", String::new()).is_ok());
    }

    #[test]
    fn result_code_1_unknown_result_returns_ok() {
        // Future-proofing: any non-`"1"` Result under ResultCode=1 is
        // success per WPF. A new `"2"` code should NOT surface as
        // ServerMessage.
        assert!(classify_outcome("1", "2", "ignored".into()).is_ok());
    }

    #[test]
    fn result_code_1_result_1_means_advance_check_without_url() {
        assert_matches!(
            classify_outcome("1", "1", "ignored".into()),
            Err(LoginError::AdvanceCheckRequired { url: None })
        );
    }

    #[test]
    fn result_code_2_with_http_url_is_preserved() {
        assert_matches!(
            classify_outcome("2", "", "https://verify.example/c".into()),
            Err(LoginError::AdvanceCheckRequired { url: Some(u) }) if u == "https://verify.example/c"
        );
    }

    #[test]
    fn result_code_2_with_non_http_message_drops_the_url() {
        assert_matches!(
            classify_outcome("2", "", "nope".into()),
            Err(LoginError::AdvanceCheckRequired { url: None })
        );
    }

    #[test]
    fn any_other_result_code_surfaces_server_message() {
        assert_matches!(
            classify_outcome("-1", "", "帳號或密碼錯誤".into()),
            Err(LoginError::ServerMessage(m)) if m == "帳號或密碼錯誤"
        );
    }
}
