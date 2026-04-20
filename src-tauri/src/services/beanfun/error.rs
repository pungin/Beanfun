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

    /// GamePass login is only supported in the TW region. WPF
    /// `MainWindow.loginMethodInit` (L1099-1114) hides the
    /// `btn_GamePass` button when `App.LoginRegion == "HK"` (same
    /// guard that disables `btn_QRCode`), and the GamePass WebView
    /// hardcodes the TW `login.beanfun.com/GP/GPLoginInfo.aspx` host.
    /// We surface a dedicated typed error so `login_gamepass_start`
    /// can refuse the call early before opening any window or
    /// minting an HTTP client, mirroring the QR sibling above.
    #[error("GamePass login is not supported in the HK region")]
    GamepassUnsupportedRegion,

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
    // OTP retrieval (`BeanfunClient.OTP.cs::GetOTP`, P4.2)
    // ---------------------------------------------------------------------
    /// WPF `OTPNoLongPollingKey:{response}` (L39-40) — step 1
    /// (`game_start_step2.aspx`) returned a body that did **not** contain
    /// the expected `GetResultByLongPolling&key=...` substring.
    ///
    /// We carry a bounded `snippet` of the body for diagnostics
    /// (matching WPF's behaviour of dumping the whole response into
    /// `errmsg`) without holding the full body indefinitely.
    #[error("OTP step 1 missing long-polling key (snippet: {snippet:?})")]
    OtpMissingLongPollingKey { snippet: String },

    /// WPF `OTPNoUnkData` (L50-51) — step 1 (TW only) failed to extract
    /// the `MyAccountData.ServiceAccountCreateTime + "key=value";`
    /// fragment that becomes a per-account form field on step 3
    /// (`record_service_start.ashx`). HK does not parse this field.
    #[error("OTP step 1 missing TW per-account form fragment")]
    OtpMissingUnkData,

    /// WPF `OTPNoCreateTime` (L61-62) — the caller passed a
    /// `ServiceAccount` whose `screatetime` was `None` *and* the
    /// fallback regex (`ServiceAccountCreateTime: "..."`) on step 1's
    /// response also failed to match. WPF mutates the input account
    /// here; we keep the input immutable and surface this typed error
    /// instead.
    #[error("OTP step 1 missing service-account create time (fallback also failed)")]
    OtpMissingCreateTime,

    /// WPF `OTPNoSecretCode` (L73-74) — step 2 (`get_cookies.ashx`)
    /// returned a body without the `var m_strSecretCode = '...';`
    /// fragment that step 5 needs.
    #[error("OTP step 2 missing m_strSecretCode")]
    OtpMissingSecretCode,

    /// WPF `OTPNoResponse` (L105-112) — step 5
    /// (`get_webstart_otp.ashx`) returned an empty body **or** a body
    /// that did not split into at least 2 segments by `;`. Both
    /// branches surface here because they are semantically the same
    /// outcome ("server gave us nothing parseable").
    #[error("OTP step 5 returned empty or unparseable response")]
    OtpEmptyResponse,

    /// WPF `GetOtpError\r\n{message}` (L117-124) — step 5 returned
    /// `parts[0] != "1"`, signalling that the server rejected the
    /// request (typically maintenance, account lock, or service
    /// unavailable). Carries the raw server message verbatim so the
    /// UI can display / localise as needed; matching the P4.1
    /// `AmountLimitNotice` convention, the service layer does **not**
    /// prepend the localised "Get OTP failed" prefix.
    #[error("OTP step 5 server rejected: {message}")]
    OtpServerRejected { message: String },

    /// WPF `DecryptOTPError` (L136) — step 6 (`WCDESComp.DecryStrHex`)
    /// returned `null`. In our Rust port [`crate::core::wcdes::decrypt_hex`]
    /// surfaces typed [`crate::core::wcdes::WcdesError`] values for
    /// the underlying cause (invalid key length, invalid hex,
    /// non-block-aligned ciphertext); we collapse them all into this
    /// single variant with the underlying error's `Display` text in
    /// the `cause` field for diagnostics, matching WPF's
    /// "give up and report decryption failure" posture.
    #[error("OTP step 6 decryption failed: {cause}")]
    OtpDecryptionFailed { cause: String },

    // ---------------------------------------------------------------------
    // Advance-check verify (`BeanfunClient.Verify.cs`, P4.3)
    // ---------------------------------------------------------------------
    /// WPF `VerifyNoViewstate` (`MainWindow.xaml.cs::reLoadVerifyPage`
    /// L761) — the AdvanceCheck.aspx HTML did not contain a
    /// `__VIEWSTATE` hidden field. Either the server returned an
    /// unexpected page, or our regex no longer matches the current
    /// markup.
    #[error("verify page missing __VIEWSTATE")]
    VerifyMissingViewState,

    /// WPF `VerifyNoEventvalidation` (`reLoadVerifyPage` L776) — the
    /// AdvanceCheck.aspx HTML did not contain an `__EVENTVALIDATION`
    /// hidden field. Note `__VIEWSTATEGENERATOR` is **not** required
    /// (WPF stores it only when present, L766-770) so it doesn't get
    /// its own variant.
    #[error("verify page missing __EVENTVALIDATION")]
    VerifyMissingEventValidation,

    /// WPF `VerifyNoSamplecaptcha` (`reLoadVerifyPage` L784) — the
    /// AdvanceCheck.aspx HTML did not contain a `LBD_VCID_*` captcha
    /// id field. The captcha image URL embeds this id as the `t=`
    /// query parameter.
    #[error("verify page missing LBD_VCID_* captcha id")]
    VerifyMissingSampleCaptcha,

    /// WPF `VerifyNoLblAuthType` (`reLoadVerifyPage` L792) — the
    /// AdvanceCheck.aspx HTML did not contain the `lblAuthType`
    /// label. WPF surfaces this label inside the verify dialog so
    /// the user knows whether they're being asked for an email or
    /// SMS code.
    #[error("verify page missing lblAuthType label")]
    VerifyMissingLblAuthType,

    /// WPF `getVerifyCaptcha` L48-52 (`buffer == null || buffer.Length
    /// < 500`) — the captcha image endpoint returned a body too small
    /// to be a real PNG. WPF returns `null` and the verify dialog
    /// renders no image; we surface a typed error so callers can
    /// distinguish "rate-limited / blocked" from "decode failure".
    /// `actual` carries the byte count for diagnostics.
    #[error("verify captcha image too small to be valid (got {actual} bytes, < 500)")]
    VerifyCaptchaImageTooSmall { actual: usize },

    // ---------------------------------------------------------------------
    // WebForms account-management (`BeanfunClient.Account.cs`
    // `UnconnectedGame_*`, P4.4)
    // ---------------------------------------------------------------------
    //
    // The five `*Missing*` variants below mirror the five distinct
    // `errmsg = "LoginNo*"` strings WPF raises while parsing the
    // accounts-management WebForms HTML
    // (`UnconnectedGame_InitAccountPayload` /
    // `_InitAddAccountPayload` / `_AddAccountCheck` /
    // `_AddAccountCheckNickName` / `_ChangePassword`). They share the
    // same Naming pattern as the verify chunk's `Verify*Missing*`
    // variants for grep-friendliness; consolidating both groups under
    // a generic `MissingHiddenField { context, name }` is left for a
    // potential P10 cross-cutting refactor.
    /// WPF `LoginNoViewstate` raised inside any of the
    /// accounts-management `UnconnectedGame_*` flows
    /// (`Account.cs` L191 / L240 / L326 / L397 / L507 / L561) —
    /// the WebForms HTML page returned by `auth.aspx` /
    /// `01.aspx` / `01Accounts.aspx` / `02.aspx` / `03.aspx` did
    /// not contain a `__VIEWSTATE` hidden input.
    #[error("accounts-management page missing __VIEWSTATE")]
    AccountMgmtMissingViewState,

    /// WPF `LoginNoViewstategenerator` (`Account.cs` L198 / L247
    /// / L333 / L404 / L514 / L568) — the WebForms HTML did not
    /// contain a `__VIEWSTATEGENERATOR` hidden input.
    #[error("accounts-management page missing __VIEWSTATEGENERATOR")]
    AccountMgmtMissingViewStateGenerator,

    /// WPF `LoginNoEventvalidation` (`Account.cs` L253 / L340 /
    /// L411 / L521 / L575) — the WebForms HTML did not contain an
    /// `__EVENTVALIDATION` hidden input. Note that
    /// `UnconnectedGame_InitAccountPayload` (`auth.aspx` GET, L191
    /// / L198) does **not** check this field; only the post-`02.aspx`
    /// / `01Accounts.aspx` / `03.aspx` parses do.
    #[error("accounts-management page missing __EVENTVALIDATION")]
    AccountMgmtMissingEventValidation,

    /// WPF `LoginNoGameName` (`Account.cs` L269) — the
    /// `UnconnectedGame_InitAddAccountPayload` POST response did not
    /// contain the `<span id="lblGameName">…</span>` element that
    /// the AddAccount UI shows. Surfaces only from
    /// [`super::unconnected_game_init_add_account_payload`].
    #[error("accounts-management init-add-account page missing lblGameName")]
    AccountMgmtMissingGameName,

    /// WPF `LoginNoAccountLen` (`Account.cs` L277) — the
    /// `UnconnectedGame_InitAddAccountPayload` POST response did not
    /// contain the `<span id="lblAccountLen">…</span>` element that
    /// drives the AddAccount UI's per-game length range
    /// (e.g. `"6 - 12"`). Surfaces only from
    /// [`super::unconnected_game_init_add_account_payload`].
    #[error("accounts-management init-add-account page missing lblAccountLen")]
    AccountMgmtMissingAccountLen,

    // ---------------------------------------------------------------------
    // Game listing (`MainWindow.xaml.cs::reLoadGameInfo`, P12.3 D1)
    // ---------------------------------------------------------------------
    /// `MainWindow.xaml.cs::reLoadGameInfo` L707-708 — the
    /// `game_zone/` HTML response did not contain the
    /// `Services.ServiceList = …;` JS literal that drives the
    /// GameList dialog.
    ///
    /// WPF silently swallows this case (its only guard is the
    /// outer `if (reg.IsMatch(res))` with no `else` branch); the
    /// Rust port surfaces a typed error so the GameList dialog
    /// can show a retry banner instead of an empty grid. See
    /// [`crate::services::beanfun::games`] module docs for the
    /// "WPF deviation" rationale (strictly-additive UX
    /// improvement; no behaviour regression for well-formed
    /// responses).
    #[error("game_zone/ response missing `Services.ServiceList = …;` literal")]
    GameListServiceListMissing,

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
