//! QR-code login **poll** step — single-shot
//! `POST /QRLogin/CheckLoginStatus` that returns whether the user has
//! scanned / confirmed / cancelled the QR code yet.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Login.cs::QRCodeCheckLoginStatus`
//! (L609-665) is a single round-trip the WinForms timer
//! (`MainWindow.qrCheckLogin_Tick`, L2340-2368) fires once per
//! second:
//!
//! ```csharp
//! SetBaseHeaders(true, "application/json, text/plain, */*",
//!                "https://login.beanfun.com/Login/Index?pSKey={skey}");
//! Headers.Set("Origin", "https://login.beanfun.com");
//! Headers.Set("RequestVerificationToken", qrcodeclass.requestVerificationToken);
//! NameValueCollection payload = new NameValueCollection(); // empty
//! string response = UploadString(
//!     "https://login.beanfun.com/QRLogin/CheckLoginStatus",
//!     payload,
//! );
//! ```
//!
//! `SetBaseHeaders` clears every header first (L917) before writing
//! the four it cares about, so — unlike [`super::qr_init`] — this
//! call does **not** send `X-Requested-With`. We mirror that
//! exactly: any extra header here would be observable to the server
//! and risk diverging from WPF's wire shape.
//!
//! # Single-shot polling
//!
//! Same contract as `login/registered_device.rs` — the function
//! performs exactly one round-trip and returns a typed
//! [`QrPollOutcome`]. The caller (Tauri command, eventual
//! orchestrator) owns the polling loop and decides cadence,
//! cancellation, and what to do with each outcome (continue /
//! refresh QR / kick off `qr_finalize`). Keeping the loop
//! out-of-tree means the function stays trivially testable against
//! one wiremock response.
//!
//! # Outcome mapping
//!
//! WPF `QRCodeCheckLoginStatus` switches on the `ResultMessage`
//! string and folds the four known values into three int return
//! codes (L640-653):
//!
//! | `ResultMessage`   | WPF int | Our [`QrPollOutcome`]                   |
//! |-------------------|---------|-----------------------------------------|
//! | `"Failed"`        |   `0`   | [`QrPollOutcome::Failed`]               |
//! | `"Wait Login"`    |   `0`   | [`QrPollOutcome::WaitLogin`]            |
//! | `"Token Expired"` |  `-2`   | [`QrPollOutcome::TokenExpired`]         |
//! | `"Success"`       |   `1`   | [`QrPollOutcome::Approved`]             |
//! | other / missing   |  `-1`   | `Err(`[`LoginError::ServerMessage`]`)`  |
//!
//! WPF conflates `"Failed"` and `"Wait Login"` into a single
//! "keep polling" int code. We deliberately keep the WPF-string
//! distinction so the UI can show different copy if it ever wants
//! to (e.g. "Server hiccup, retrying" vs "Waiting for confirmation"),
//! per the user's chunk 3.4 design decision (option B / no
//! conflation).
//!
//! # `Approved` carries no payload
//!
//! WPF's `Success` branch returns int 1 and **never** reads anything
//! from `ResultData` (L647-648). The downstream `do_Login` →
//! `QRCodeLogin` step pulls everything it needs (`skey`,
//! `requestVerificationToken`) from the original `QRCodeClass` — i.e.
//! our [`QrLoginInit`]. So [`QrPollOutcome::Approved`] is a unit
//! variant; the caller already has `&QrLoginInit` in scope to drive
//! `qr_finalize`.
//!
//! # Error mapping
//!
//! - JSON parse failure → [`LoginError::QrJsonParseFailed`] — WPF
//!   `errmsg = "LoginJsonParseFailed"; return -1` (L634-638).
//! - Unknown `ResultMessage` (or absent field) →
//!   [`LoginError::ServerMessage`] carrying the raw response body —
//!   WPF `errmsg = response; return -1` (L649-652).
//! - HTTP transport failure → [`LoginError::Http`] (auto via `?`) —
//!   WPF caught the WebException, formatted it, and returned -1
//!   (L655-661); our typed wrapping is strictly safer for downstream
//!   pattern matching.
//! - HTTP non-2xx → [`LoginError::Unknown`] via the shared
//!   `ensure_success` helper.
//! - HK region → [`LoginError::QrUnsupportedRegion`] before any HTTP
//!   traffic, mirroring [`super::qr_init`] and the WPF UI guard at
//!   `MainWindow.loginMethodInit` L1099-1114.

use reqwest::header;
use serde::Deserialize;

use super::qr_init::QrLoginInit;
use super::{ensure_success, truncate_chars, BODY_LOG_PREVIEW_CHARS};
use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion};

/// One round-trip's worth of state from `QRLogin/CheckLoginStatus`.
///
/// All variants are unit — none of them carries data. WPF's
/// `Success` branch never reads `ResultData` (L647-648), and the
/// downstream `qr_finalize` step pulls `skey` /
/// `verification_token` from the [`QrLoginInit`] the caller is
/// already holding. Keeping outcomes payload-free means the caller
/// can `match` without destructuring and the enum stays
/// [`Copy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrPollOutcome {
    /// `ResultMessage == "Failed"` — WPF int return code `0` (keep
    /// polling). Server-side reports the round-trip failed but the
    /// session is still live; caller should poll again on the next
    /// tick.
    Failed,

    /// `ResultMessage == "Wait Login"` — WPF int return code `0`
    /// (keep polling). User has scanned the QR but not yet confirmed
    /// in the mobile app; caller should poll again on the next tick.
    WaitLogin,

    /// `ResultMessage == "Token Expired"` — WPF int return code
    /// `-2`. The QR token has aged out; caller should refresh the
    /// QR (run [`super::init_qr_login`] again). UI side this is
    /// `MainWindow.qrCheckLogin_Tick` L2364-2367 →
    /// `refreshQRCode()`.
    TokenExpired,

    /// `ResultMessage == "Success"` — WPF int return code `1`. User
    /// confirmed login in the mobile app; caller should now run
    /// `qr_finalize::finalize_qr_login` (chunk 3.4.3) to close out
    /// the flow.
    Approved,
}

/// Run one `QRLogin/CheckLoginStatus` round-trip and classify the
/// result.
///
/// See module docs for the mapping table and error contract. The
/// caller owns the polling loop.
///
/// `init` carries everything this step needs: `skey` (for the
/// `Referer` URL), `verification_token` (for the
/// `RequestVerificationToken` header). Caller passes `&init` so the
/// same value can drive a follow-up `qr_finalize` call without
/// re-cloning.
pub async fn poll_qr_login_status(
    client: &BeanfunClient,
    init: &QrLoginInit,
) -> Result<QrPollOutcome, LoginError> {
    if client.config().region != LoginRegion::TW {
        return Err(LoginError::QrUnsupportedRegion);
    }

    let url = client.login_url("QRLogin/CheckLoginStatus")?;
    let referer_url = client.login_url_with_skey("Login/Index", &init.skey)?;
    // `Url::origin().ascii_serialization()` yields `scheme://host[:port]`
    // with no trailing slash and no path — byte-equal to WPF's
    // hardcoded `"https://login.beanfun.com"` literal at L620 when the
    // login_base is the production URL, and yields the equivalent
    // mock origin in tests.
    let origin = client
        .config()
        .endpoints
        .login_base
        .origin()
        .ascii_serialization();

    // Header set mirrors WPF L615-621 exactly: Accept + Referer (via
    // `SetBaseHeaders`), then Origin + RequestVerificationToken
    // (via `Headers.Set`). `SetBaseHeaders` clears all headers first
    // (L917), so `X-Requested-With` is intentionally absent — adding
    // it here would diverge from the production wire shape.
    //
    // Body is empty (`NameValueCollection payload = new ...` with no
    // entries). WPF's `WebClient.UploadString` emits two headers
    // automatically for that empty payload which we have to
    // reconstruct manually around `.body("")`:
    //
    // - `Content-Type: application/x-www-form-urlencoded` — reqwest
    //   only auto-sets it for `.form(&T)` / `.json(&T)`, not for
    //   raw `.body(...)`.
    // - `Content-Length: 0` — reqwest/hyper treats `.body("")` as
    //   a no-length streaming body and emits
    //   `Transfer-Encoding: chunked` (or neither framing header)
    //   instead of `Content-Length: 0`. Beanfun's
    //   `QRLogin/CheckLoginStatus` endpoint is strict HTTP/1.1 and
    //   rejects both alternatives with `HTTP 411 Length Required`
    //   (observed live 2026-04-18). An explicit zero-length header
    //   restores byte-equal wire parity with WPF's `UploadString`.
    let resp = client
        .http()
        .post(url)
        .header(header::ACCEPT, "application/json, text/plain, */*")
        .header(header::REFERER, referer_url.as_str())
        .header("Origin", origin)
        .header("RequestVerificationToken", &init.verification_token)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .await?;

    ensure_success(&resp, "QRLogin/CheckLoginStatus")?;
    let body = client.bounded_text(resp).await?;

    // WPF L630-638 uses a try/catch around `JObject.Parse`. We
    // collapse the underlying `serde_json::Error` into our
    // dedicated `QrJsonParseFailed` variant to keep the WPF
    // `errmsg = "LoginJsonParseFailed"` mapping intact.
    //
    // Diagnostic (same pattern as `session_key.rs` regex-miss and
    // `send_login.rs` empty-scrape paths from the D3 observability
    // commit): log a bounded `body_preview` so a server-side wire
    // shape regression (e.g. `ResultMessage` field returned as an
    // integer à la P3 parity fix) is identifiable from one tracing
    // line instead of a silent frontend banner. The error code
    // itself (`auth.qr_json_parse_failed`) doesn't change.
    let parsed: PollResponse = serde_json::from_str(&body).map_err(|e| {
        tracing::warn!(
            step = "qr_poll",
            error = %e,
            body_preview = %truncate_chars(&body, BODY_LOG_PREVIEW_CHARS),
            "QR poll response JSON parse failed",
        );
        LoginError::QrJsonParseFailed
    })?;

    // WPF L640-652 dispatch table. Missing `ResultMessage` field
    // casts to null in C# (`(string)jsonData["ResultMessage"]`) and
    // therefore matches none of the literal branches → falls into
    // the `else` arm at L649-652 → `errmsg = response`. We mirror
    // that fall-through with the catch-all `_` arm.
    match parsed.result_message.as_deref() {
        Some("Failed") => Ok(QrPollOutcome::Failed),
        Some("Wait Login") => Ok(QrPollOutcome::WaitLogin),
        Some("Token Expired") => Ok(QrPollOutcome::TokenExpired),
        Some("Success") => Ok(QrPollOutcome::Approved),
        other => {
            // Unknown / absent `ResultMessage` → mirror WPF's
            // `errmsg = response; return -1` fall-through. Log the
            // preview **before** moving `body` into the error so a
            // future beanfun release that adds a new status (e.g.
            // `"RateLimited"`) surfaces in the logs as a clear
            // regression signal rather than just a user-visible
            // error banner.
            tracing::warn!(
                step = "qr_poll",
                result_message = ?other,
                body_preview = %truncate_chars(&body, BODY_LOG_PREVIEW_CHARS),
                "QR poll returned unknown ResultMessage",
            );
            Err(LoginError::ServerMessage(body))
        }
    }
}

// -----------------------------------------------------------------------------
// JSON shape — private to this module
// -----------------------------------------------------------------------------

/// Sliver of the `QRLogin/CheckLoginStatus` JSON body we actually
/// read. We only need `ResultMessage` — WPF's `Success` branch
/// never touches `ResultData` (L647-648), and surfacing whatever
/// extra fields the server happens to send today would lock us
/// into a wire shape the next backend release might break.
#[derive(Debug, Deserialize)]
struct PollResponse {
    /// The status string. Boxed in `Option` so the
    /// "field absent" case folds cleanly into the same
    /// catch-all arm as "field present but unknown value", matching
    /// WPF's `(string)jsonData["ResultMessage"]` null fall-through.
    #[serde(rename = "ResultMessage")]
    result_message: Option<String>,
}

// -----------------------------------------------------------------------------
// Unit tests — pure serde shape only. The full HTTP orchestration
// + dispatch table lives in `tests/qr_poll.rs`.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_response_parses_with_extra_ignored_fields() {
        // The real server includes a `ResultData` object alongside
        // `ResultMessage`; serde's default skip-unknown-fields
        // behaviour means we accept the extra payload without
        // requiring a model for it.
        let body = r#"{
            "ResultMessage": "Success",
            "ResultData": { "SessionKey": "ignored", "Status": 0 }
        }"#;
        let parsed: PollResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(parsed.result_message.as_deref(), Some("Success"));
    }

    #[test]
    fn poll_response_field_name_is_capital_camel_case() {
        // Locks the spelling — a future serde rename to
        // `result_message` (snake_case) would silently break the
        // dispatch table.
        let body = r#"{"resultMessage":"Success"}"#;
        let parsed: PollResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(
            parsed.result_message.is_none(),
            "lower-case key must NOT match — server uses CapitalCamelCase"
        );
    }

    #[test]
    fn poll_response_treats_missing_field_as_none() {
        let body = r#"{"OtherField":42}"#;
        let parsed: PollResponse = serde_json::from_str(body).expect("valid JSON");
        assert!(parsed.result_message.is_none());
    }
}
