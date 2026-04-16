//! Step 3 of the TW Regular flow: `POST Login/AccountLogin`.
//!
//! This is where credentials are actually validated. The server
//! multiplexes three outcomes on a pair of string codes in the JSON
//! response:
//!
//! | `ResultCode` | `Result` | Meaning                                        |
//! |:------------:|:--------:|:-----------------------------------------------|
//! | `"1"`        | `"0"`    | success – proceed to `SendLogin`               |
//! | `"1"`        | `"1"`    | advance-check needed (captcha / email), no URL |
//! | `"2"`        | *any*    | advance-check needed with external URL         |
//! | anything else| *any*    | `ResultMessage` surfaced verbatim to the user  |
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L80-113.
//!
//! # Sensitive data note
//!
//! The password is serialised into the JSON body via serde, which
//! allocates a `String` / `Vec<u8>` that neither we nor `reqwest` can
//! zero on drop. Zeroising the **request body** is a larger design
//! change (custom serde adapter + intercepting reqwest's internal
//! buffer) that we explicitly defer to a later pass; the zeroise-on-drop
//! of [`Credentials`](crate::services::beanfun::Credentials) itself
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
fn classify_outcome(
    result_code: &str,
    result: &str,
    result_message: String,
) -> Result<(), LoginError> {
    match (result_code, result) {
        ("1", "0") => Ok(()),
        ("1", "1") => Err(LoginError::AdvanceCheckRequired { url: None }),
        ("2", _) => {
            // WPF L101-104: only keep the message as a URL when it
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
    fn success_returns_ok() {
        assert!(classify_outcome("1", "0", String::new()).is_ok());
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
