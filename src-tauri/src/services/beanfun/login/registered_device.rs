//! Orchestrator for the **device-registration polling** step —
//! a single POST to `bfAPPAutoLogin.ashx` that asks the server
//! whether the mobile-app user has approved / rejected an
//! out-of-band device authorisation request.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Login.cs::CheckIsRegisteDevice`
//! (L667-700) plus `MainWindow.xaml.cs::bfAPPAutoLogin_Tick`
//! (L2400-2441). WPF wires the two together via a
//! `DispatcherTimer`: the window ticks at a fixed cadence and each
//! tick invokes `CheckIsRegisteDevice` once, dispatching on the
//! `IntResult` returned by the JSON body:
//!
//! | WPF `IntResult` | WPF action (MainWindow L2418-2439)     | Our mapping                                     |
//! |-----------------|----------------------------------------|-------------------------------------------------|
//! | `"-3"`          | `errexit("MsgBeanfunRejectLogin")`      | `Err(DeviceLoginRejected)`                      |
//! | `"-2"`          | `NavigateLoginPage()` (timeout reset)   | `Err(DeviceLoginTimeout)`                       |
//! | `"-1"`          | `errexit(StrReslut)` (opaque message)   | `Err(ServerMessage(str_reslut))`                |
//! | `"0"`           | `return;` — keep polling                | `Ok(None)`                                      |
//! | `"1"`           | "尚未授權本次登入" — keep polling        | `Ok(None)`                                      |
//! | `"2"`           | `loginWorker_RunWorkerCompleted(...)`    | `Ok(Some(session))` via `login_completed`       |
//! | anything else   | unreachable in WPF's switch             | `Err(Unknown(...))`                             |
//!
//! # Single-shot API (callers own the loop)
//!
//! We expose **one** async call that performs **one** HTTP
//! round-trip. The caller — typically the login orchestrator or the
//! UI tick handler — drives the polling loop itself, choosing the
//! cadence and back-off policy. This mirrors the WPF architecture
//! (the timer lives on `MainWindow`, not inside `BeanfunClient`) and
//! keeps the orchestrator free of hard-coded timing assumptions, so
//! integration tests can exercise individual response branches
//! without mocking a clock.
//!
//! # `IntResult=="2"` — internal `login_completed` call
//!
//! WPF's `CheckIsRegisteDevice` (L683-697) does **three** things on
//! success (`IntResult=="2"`):
//!
//! 1. Fires a side-effect `DownloadString("…/login/" + StrReslut)` —
//!    the response body is discarded but the request primes the
//!    cookie jar with whatever the server sets along the way
//!    (L685-687).
//! 2. Regex-extracts `akey=(.*)` against `StrReslut` itself (L688-694
//!    — note: not against the body fetched above; the string `StrReslut`
//!    already carries the akey).
//! 3. Calls `LoginCompleted(akey, service_code, service_region)` —
//!    the shared "login tail" that posts to `return.aspx` and reads
//!    `bfWebToken` off the redirect (L696).
//!
//! We mirror all three: one GET for the cookie side-effect, akey
//! extraction via the shared [`extract_akey`] regex, and a call to
//! [`login_completed`] with the caller-supplied session key /
//! account id / service metadata. Producing a `Session` inside this
//! module (rather than returning an `(akey, _)` tuple for the
//! caller to finalise) preserves the WPF contract that "IntResult==2
//! means the login is finished" — callers can treat
//! `Ok(Some(session))` as "you are logged in", nothing more to do.
//!
//! # URL choice — `tw.newlogin.beanfun.com` for both regions
//!
//! WPF hardcodes `https://tw.newlogin.beanfun.com/login/bfAPPAutoLogin.ashx`
//! regardless of `App.LoginRegion` (L675-676). Our
//! [`Endpoints::newlogin_base`](super::super::client::Endpoints)
//! for HK is deliberately set to the same TW host for exactly this
//! call. Routing the URL through `newlogin_base` (instead of a
//! module-local hardcode) keeps the wiremock-injectable test path
//! intact.
//!
//! # `IntResult=="2"` + `AKeyParseFailed` (WPF L688-693)
//!
//! If `StrReslut` on a "2" branch does not match the `akey=(.*)`
//! regex, WPF sets `this.errmsg = "AKeyParseFailed"` and `return
//! null;`. The `MainWindow.bfAPPAutoLogin_Tick` handler guards its
//! switch behind `resultJson == null || resultJson["IntResult"] ==
//! null` (L2413-2414), so a null return causes the tick to
//! **silently continue polling** — WPF never propagates the
//! `AKeyParseFailed` message to the user on this code path.
//!
//! We preserve that observable behaviour: on akey-parse failure we
//! return `Ok(None)` so the caller's polling loop naturally retries.
//! The user's directive was "結果能對齊舊實作才優化"; surfacing a
//! hard error here would diverge from WPF's silent-retry behaviour.
//! A tracing-level log (added by the caller, not by this module) is
//! sufficient to flag the anomaly during diagnostics.

use reqwest::header;
use serde::Deserialize;

use crate::core::parser::{extract_akey, ParserError};
use crate::services::beanfun::{
    login::{completed::login_completed, ensure_success},
    BeanfunClient, LoginError, Session,
};

/// One `CheckIsRegisteDevice` round-trip. Returns:
///
/// - `Ok(Some(session))` — server approved (`IntResult=="2"`) and
///   we successfully ran [`login_completed`] to mint the final
///   `bfWebToken`.
/// - `Ok(None)` — server is still waiting for the user to act
///   (`IntResult=="0"` or `"1"`). Caller should sleep and poll
///   again. Also returned when `IntResult=="2"` but the `StrReslut`
///   carries no parseable `akey=…` (WPF's silent-retry path,
///   see module docs).
/// - `Err(LoginError::DeviceLoginRejected)` — `IntResult=="-3"`.
/// - `Err(LoginError::DeviceLoginTimeout)` — `IntResult=="-2"`.
/// - `Err(LoginError::ServerMessage(StrReslut))` — `IntResult=="-1"`,
///   the opaque fatal-error branch from WPF L2428-2430.
/// - `Err(LoginError::Unknown(...))` — any `IntResult` value the
///   WPF switch does not enumerate (including missing JSON fields).
/// - Any [`LoginError`] that [`login_completed`] or the HTTP
///   transport layer can surface bubbles up unchanged.
///
/// # Parameters
///
/// - `client` — same [`BeanfunClient`] that produced the
///   [`LoginError::DeviceRegistrationRequired`] continuation. The
///   cookie jar carries the ASP.NET session that the server binds
///   the poll response to; a different client would be a
///   different session.
/// - `login_token` — the `login_token` field captured from
///   [`LoginError::DeviceRegistrationRequired`]. Sent on the wire
///   as the form field `LT`.
/// - `session_key` — the `pSKey` the parent login flow obtained
///   from `get_session_key`. Forwarded to [`login_completed`] so
///   the final `Session.skey` matches the HK / TOTP producer.
/// - `account_id` — the user-facing login id, propagated onto
///   `Session.account_id` for UI purposes.
/// - `service_code` / `service_region` — MapleStory service
///   metadata (same contract as everywhere else —
///   see `login_hk_regular` / `login_totp` module docs).
pub async fn login_registered_device(
    client: &BeanfunClient,
    login_token: &str,
    session_key: &str,
    account_id: &str,
    service_code: &str,
    service_region: &str,
) -> Result<Option<Session>, LoginError> {
    debug_assert!(
        !login_token.is_empty(),
        "login_registered_device requires a non-empty login_token"
    );

    let body = poll_bf_app_auto_login(client, login_token).await?;

    // WPF L679-681 — `json == null || json["IntResult"] == null ||
    // json["StrReslut"] == null` short-circuits to `return null`.
    // We surface that as a structured error rather than folding it
    // into the "keep polling" path: a malformed JSON response is a
    // contract breach that deserves a distinct diagnostic.
    let parsed: PollResponse = serde_json::from_str(&body)?;
    let int_result = parsed
        .int_result
        .ok_or_else(|| LoginError::Unknown("bfAPPAutoLogin response missing IntResult".into()))?;
    let str_reslut = parsed
        .str_reslut
        .ok_or_else(|| LoginError::Unknown("bfAPPAutoLogin response missing StrReslut".into()))?;

    match int_result.as_str() {
        // Success — run the login-completion tail. The "2" branch
        // may internally return Ok(None) when StrReslut lacks an
        // akey, matching WPF's silent-retry behaviour (see module
        // docs).
        "2" => {
            finalise_registered_device_login(
                client,
                session_key,
                account_id,
                &str_reslut,
                service_code,
                service_region,
            )
            .await
        }
        // WPF "0" (waiting) and "1" ("尚未授權") both mean "user
        // has not yet acted — keep polling". We collapse them onto
        // Ok(None) because the distinction does not matter to the
        // caller's polling loop.
        "0" | "1" => Ok(None),
        // WPF L2428-2430 — `-1` is an opaque fatal-error branch
        // whose message is carried in StrReslut. Surface verbatim
        // so the UI can display whatever the server sent.
        "-1" => Err(LoginError::ServerMessage(str_reslut)),
        // WPF L2424-2427 — `-2` is a server-enforced timeout.
        "-2" => Err(LoginError::DeviceLoginTimeout),
        // WPF L2420-2423 — `-3` means the user (or policy) rejected
        // the device registration request.
        "-3" => Err(LoginError::DeviceLoginRejected),
        // WPF's switch leaves this unreachable — any other value is
        // a server contract violation.
        other => Err(LoginError::Unknown(format!(
            "bfAPPAutoLogin unexpected IntResult={other}"
        ))),
    }
}

// -----------------------------------------------------------------------------
// Helpers — private to this module
// -----------------------------------------------------------------------------

/// Deserialisation shape for the `bfAPPAutoLogin.ashx` JSON
/// response. The server's typo (`StrReslut` instead of `StrResult`)
/// is preserved verbatim — it is what the real server sends (see
/// WPF `BeanfunClient.Login.cs` L680-697 and `MainWindow.xaml.cs`
/// L2429) and renaming it would break the wire contract.
#[derive(Debug, Deserialize)]
struct PollResponse {
    #[serde(rename = "IntResult")]
    int_result: Option<String>,
    #[serde(rename = "StrReslut")]
    str_reslut: Option<String>,
}

/// POST `LT={login_token}` to `newlogin_base/login/bfAPPAutoLogin.ashx`
/// and return the response body.
///
/// WPF's `UploadString` uses the default `application/x-www-form-urlencoded`
/// Content-Type that `reqwest::RequestBuilder::form(...)` also
/// produces, so no explicit header is needed. WPF likewise does not
/// call `SetBaseHeaders` on this call, so we intentionally do **not**
/// send a `Referer` either — the `bfAPPAutoLogin.ashx` handler has
/// been running in production for years against WPF's bare POST and
/// any added header is a divergence that risks server-side allowlist
/// surprises. Other calls in this tree (e.g. TW Regular) do send
/// Referer where WPF also does; we keep parity on a per-call basis.
async fn poll_bf_app_auto_login(
    client: &BeanfunClient,
    login_token: &str,
) -> Result<String, LoginError> {
    let url = client
        .config()
        .endpoints
        .newlogin_base
        .join("login/bfAPPAutoLogin.ashx")
        .map_err(|e| LoginError::InvalidUrl(format!("bfAPPAutoLogin URL: {e}")))?;

    let form = [("LT", login_token)];
    let resp = client
        .http()
        .post(url)
        // Accept all the usual JSON variants reqwest might negotiate
        // against — mirrors what WPF's `UploadString` gets served by
        // default.
        .header(header::ACCEPT, "*/*")
        .form(&form)
        .send()
        .await?;

    ensure_success(&resp, "bfAPPAutoLogin POST")?;
    client.bounded_text(resp).await
}

/// Run the "IntResult == 2" tail: side-effect GET on
/// `{newlogin_base}/login/{StrReslut}`, extract `akey`, hand off to
/// [`login_completed`].
///
/// Returns `Ok(None)` on akey-parse failure to preserve WPF's
/// silent-retry semantics (see module docs); any other failure
/// bubbles up as-is.
async fn finalise_registered_device_login(
    client: &BeanfunClient,
    session_key: &str,
    account_id: &str,
    str_reslut: &str,
    service_code: &str,
    service_region: &str,
) -> Result<Option<Session>, LoginError> {
    // WPF L685-687 — `DownloadString("https://tw.newlogin.beanfun.com/login/" + StrReslut)`
    // as a pure side effect. The body is discarded but the request
    // (and any Set-Cookie headers along its redirect chain) updates
    // the shared cookie jar before the `return.aspx` POST that
    // follows in `login_completed`.
    //
    // We use string concat rather than `Url::join` because WPF
    // concatenates the strings literally; `Url::join` treats a
    // relative argument starting with `/` as an absolute path
    // replacement, which would strip the `/login/` prefix on paths
    // like `/Mlogin/…` — a divergence from WPF. Concat preserves
    // the exact byte sequence WPF sends.
    let ack_url_str = format!(
        "{}login/{}",
        client.config().endpoints.newlogin_base,
        str_reslut
    );
    let ack_url = url::Url::parse(&ack_url_str).map_err(|e| {
        LoginError::InvalidUrl(format!("bfAPPAutoLogin ack URL `{ack_url_str}`: {e}"))
    })?;

    let resp = client.http().get(ack_url).send().await?;
    ensure_success(&resp, "bfAPPAutoLogin ack GET")?;
    // Drain the body to honour the body-size cap; we discard the
    // text just like WPF discards `string test = …`.
    let _ = client.bounded_text(resp).await?;

    // WPF L688-694 — regex `akey=(.*)` against `StrReslut`. Our
    // `extract_akey` uses the exact same regex, so the match
    // semantics are 1:1.
    let akey = match extract_akey(str_reslut) {
        Ok(a) => a,
        // WPF sets `errmsg = "AKeyParseFailed"` and returns null,
        // which `bfAPPAutoLogin_Tick` silently retries — see
        // module docs for the rationale. Ok(None) preserves that.
        Err(ParserError::MissingAkey) => return Ok(None),
        Err(other) => return Err(LoginError::Parser(other)),
    };

    let session = login_completed(
        client,
        session_key,
        &akey,
        account_id,
        service_code,
        service_region,
    )
    .await?;
    Ok(Some(session))
}

// -----------------------------------------------------------------------------
// Unit tests
// -----------------------------------------------------------------------------
//
// Full wire-shape coverage lives in `tests/registered_device.rs`. We
// keep two narrow unit tests here for the pure helpers so the
// module's own invariants do not depend on wiremock being wired up.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_response_parses_expected_shape() {
        let body = r#"{"IntResult":"2","StrReslut":"Mlogin/MLoginSuccess.aspx?akey=TOK"}"#;
        let parsed: PollResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(parsed.int_result.as_deref(), Some("2"));
        assert_eq!(
            parsed.str_reslut.as_deref(),
            Some("Mlogin/MLoginSuccess.aspx?akey=TOK")
        );
    }

    #[test]
    fn poll_response_tolerates_missing_fields() {
        // Missing IntResult and StrReslut are valid JSON but map to
        // `None` — the caller short-circuits to `LoginError::Unknown`.
        // This regression guards against accidentally making either
        // field required-in-serde (which would turn a missing-field
        // response into a confusing `LoginError::Json(...)`).
        let body = r#"{}"#;
        let parsed: PollResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.int_result.is_none());
        assert!(parsed.str_reslut.is_none());
    }
}
