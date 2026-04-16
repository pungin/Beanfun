//! Typed error enum for every Beanfun login / account call.
//!
//! Variant names are grouped by phase rather than by the literal WPF
//! `errmsg` string, but each variant includes a doc comment referencing the
//! original string so cross-referencing the legacy code base stays trivial.
//!
//! # Design
//!
//! - Transport-level failures (`reqwest`, `serde_json`) are wrapped with
//!   `#[from]` so `?` in service code stays clean.
//! - Logical failures (missing viewstate, captcha required, etc.) get their
//!   own variant so call sites can pattern-match without string compares.
//! - Deliberately **does not** implement `Clone` — an in-flight
//!   `reqwest::Error` may hold non-Clone inner state; consumers who need to
//!   forward the error elsewhere should stringify it.

use thiserror::Error;

use crate::core::parser::ParserError;
use crate::services::beanfun::login::totp_challenge::TotpChallenge;

/// Everything that can go wrong during a Beanfun login or session call.
#[derive(Debug, Error)]
pub enum LoginError {
    // ---------------------------------------------------------------------
    // Pre-login / session-key acquisition
    // ---------------------------------------------------------------------
    /// WPF `LoginNoSkey` / `LoginNoOTP1` — the portal entry page did not
    /// surface a `pSKey` (TW) or OTP1 span (HK).
    #[error("failed to obtain login session key from portal entry page")]
    MissingSessionKey,

    /// WPF `LoginNoResponse` — the portal entry page returned an empty body.
    #[error("empty response body from portal entry page")]
    EmptyResponse,

    // ---------------------------------------------------------------------
    // Form / antiforgery token parsing
    // ---------------------------------------------------------------------
    /// WPF `LoginNoToken` — `__RequestVerificationToken` hidden input
    /// missing from the login Index page.
    #[error("__RequestVerificationToken missing from login page")]
    MissingVerificationToken,

    /// WPF `LoginNoViewstate`.
    #[error("__VIEWSTATE missing from login page")]
    MissingViewState,

    /// WPF `LoginNoViewstateGenerator`.
    #[error("__VIEWSTATEGENERATOR missing from login page")]
    MissingViewStateGenerator,

    /// WPF `LoginNoEventvalidation`.
    #[error("__EVENTVALIDATION missing from login page")]
    MissingEventValidation,

    // ---------------------------------------------------------------------
    // AccountLogin outcome branches
    // ---------------------------------------------------------------------
    /// WPF `LoginAdvanceCheck` — server demands the user complete an in-page
    /// advance-verification step (captcha / email re-confirm). Carries the
    /// verification URL when the server supplies one.
    #[error("advance verification required")]
    AdvanceCheckRequired { url: Option<String> },

    /// WPF `need_totp` — the server response contained a `totpLoginBtn`
    /// form, meaning the account has TOTP 2FA enabled. Carries a
    /// [`TotpChallenge`] with the viewstate + URL the caller must
    /// forward into `login_totp` once it has the 6-digit code.
    ///
    /// Semantically this is a **continuation**, not an error —
    /// `login_hk_regular` can't produce a `Session` without the user
    /// supplying the OTP, so it surfaces through the `Err` channel of
    /// `Result<Session, LoginError>` to keep the caller's
    /// happy-path type signature consistent with TW Regular. The
    /// challenge is boxed to keep the `LoginError` variants compact.
    #[error("TOTP one-time password required")]
    TotpRequired(Box<TotpChallenge>),

    /// Server returned a freeform error `ResultMessage` — we surface the raw
    /// text verbatim so the UI can localise / display it.
    #[error("login rejected: {0}")]
    ServerMessage(String),

    // ---------------------------------------------------------------------
    // Final stage (SendLogin → return.aspx → bfWebToken)
    // ---------------------------------------------------------------------
    /// WPF `SendLoginNoFormData` — the SendLogin HTML page contained no
    /// `<input>` tags we could repost to `return.aspx`.
    #[error("SendLogin returned no form fields to forward")]
    SendLoginNoFormData,

    /// WPF `LoginNoAkey` / `AKeyParseFailed` — the HK / TOTP flow expected
    /// an `akey=...` query parameter on the redirect URL but did not find
    /// one.
    #[error("missing akey in response URL")]
    MissingAkey,

    /// WPF `LoginNoWebtoken` — `return.aspx` did not emit a `bfWebToken`
    /// cookie. Usually means an upstream step returned non-`200` and
    /// silently aborted.
    #[error("bfWebToken cookie missing from return.aspx response")]
    MissingWebToken,

    // ---------------------------------------------------------------------
    // QR code specific
    // ---------------------------------------------------------------------
    /// WPF `LoginIntResultError` — `InitLogin` JSON had `Result != 0` or was
    /// missing the expected fields.
    #[error("QR init-login returned non-zero Result")]
    QrInitResultError,

    /// WPF `LoginJsonParseFailed` — QR status polling got a non-JSON
    /// response.
    #[error("QR login status polling returned non-JSON payload")]
    QrJsonParseFailed,

    /// QR login is only supported in the TW region. WPF
    /// `MainWindow.loginMethodInit` (L1099-1114) explicitly disables the
    /// `btn_QRCode` button when `App.LoginRegion == "HK"`, and the entire
    /// `BeanfunClient` QR code path (`getQRCodeStrEncryptData`,
    /// `QRCodeCheckLoginStatus`, `QRCodeLogin`) hardcodes
    /// `https://login.beanfun.com/...` regardless of region. We surface a
    /// dedicated typed error so the orchestrator (and any future
    /// non-WPF UI) can refuse the call early instead of producing a
    /// confusing transport / cookie failure deeper in the flow.
    #[error("QR login is not supported in the HK region")]
    QrUnsupportedRegion,

    // ---------------------------------------------------------------------
    // Device-registration polling (CheckIsRegisteDevice / bfAPPAutoLogin)
    // ---------------------------------------------------------------------
    /// WPF `pollRequest(...)` branch on HK Regular (L273-281) and TOTP
    /// (L377-386) — the server rendered a `pollRequest("url","TOKEN","param")`
    /// script tag signalling that the user must authorise this device via
    /// an out-of-band channel (Beanfun mobile app / email). Semantically a
    /// **continuation**: the caller is expected to loop over
    /// `login_registered_device(client, login_token, ...)` until the user
    /// approves the request (`Ok(Some(session))`), rejects it
    /// ([`DeviceLoginRejected`](Self::DeviceLoginRejected)), or it expires
    /// ([`DeviceLoginTimeout`](Self::DeviceLoginTimeout)).
    ///
    /// WPF stashes the token on `this.LoginToken` and concatenates the
    /// url + param into `this.errmsg` for display only (L277-281 and
    /// L383-385). We preserve all three pieces in this variant so
    /// callers can drive the polling loop via `login_token` and log /
    /// show the url + param for diagnostics.
    #[error("device registration required; poll bfAPPAutoLogin.ashx with LT={login_token}")]
    DeviceRegistrationRequired {
        login_token: String,
        poll_url: String,
        param: String,
    },

    /// WPF `MainWindow.bfAPPAutoLogin_Tick` IntResult=`"-2"` (L2424-2427) —
    /// the polling loop returned a timeout status. The user did not
    /// approve or reject the device registration in the server-enforced
    /// window.
    #[error("device registration polling timed out")]
    DeviceLoginTimeout,

    /// WPF `MainWindow.bfAPPAutoLogin_Tick` IntResult=`"-3"` (L2420-2423) —
    /// the user (or some upstream policy) explicitly rejected the login
    /// request.
    #[error("device registration rejected")]
    DeviceLoginRejected,

    // ---------------------------------------------------------------------
    // Transport-level errors
    // ---------------------------------------------------------------------
    /// Wrapped `reqwest::Error` — network, TLS, connect, or body-read
    /// failure. Timeout failures surface here too (reqwest encodes timeout
    /// as a transport error).
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// Response body exceeded [`super::ClientConfig::max_body_size`]. The
    /// actual size reflects what we had buffered when we decided to abort.
    #[error("response body exceeded {limit} bytes (received at least {actual})")]
    BodyTooLarge { limit: usize, actual: usize },

    /// Wrapped `serde_json::Error` from a `serde_json::from_str` call site.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Parser errors from [`crate::core::parser`] bubble up here.
    #[error("HTML parse error: {0}")]
    Parser(#[from] ParserError),

    /// URL construction failed (almost always a programming error — we
    /// build URLs from static bases + `join`).
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Response body was not valid UTF-8. Beanfun serves everything as
    /// UTF-8 so this is effectively a bug or a hostile server; we surface
    /// it as a distinct variant so callers can retry if they want.
    #[error("response body was not valid UTF-8")]
    InvalidUtf8,

    // ---------------------------------------------------------------------
    // Catch-all
    // ---------------------------------------------------------------------
    /// WPF `LoginUnknown` — any condition we cannot attribute to one of the
    /// structured variants above.
    #[error("unexpected login error: {0}")]
    Unknown(String),
}
