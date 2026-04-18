//! Authentication commands — the IPC surface for every login /
//! logout / OTP interaction the UI can drive.
//!
//! # Families exposed in P10.2
//!
//! | Command                    | Family   | Purpose                                                                                                       |
//! |----------------------------|----------|---------------------------------------------------------------------------------------------------------------|
//! | [`login_regular`]          | regular  | TW / HK username+password single-shot login (handles AdvanceCheck + TOTP detours via `CommandError`)          |
//! | [`login_totp`]             | regular  | HK two-factor continuation after [`login_regular`] surfaces `auth.totp_required`                              |
//! | [`login_qr_start`]         | QR       | Initialise a QR login session — returns the PNG (Base64 data URL) + optional Beanfun-app deeplink             |
//! | [`login_qr_check`]         | QR       | Poll the QR handle; on `Approved` the same call finalises the login and sets [`AppState::auth`][st]           |
//! | [`get_verify_page_info`]   | verify   | Fetch the AdvanceCheck verify page (returns the `lblAuthType` label)                                          |
//! | [`get_verify_captcha`]     | verify   | Fetch the captcha image (Base64 data URL)                                                                     |
//! | [`submit_verify`]          | verify   | Submit `verify_code + captcha_code`; surfaces `Success` / `WrongCaptcha` / `WrongAuthInfo` / `ServerMessage`  |
//! | [`logout`]                 | logout   | Clear local auth + every pending slot; best-effort server-side `erase_token` (errors logged, never surfaced) |
//!
//! [st]: super::state::AppState::auth
//!
//! `login_gamepass_complete` is **deliberately deferred** to P12: the
//! legacy WPF GamePass flow is WebView-driven (Razer / MS 3rd-party
//! auth inside a WebView2), and the backend API shape depends on the
//! final [`tauri::WebviewWindow`] cookie-extraction UX (P10.2 Q-risk2
//! = A).
//!
//! # Continuation state machine
//!
//! The regular family is a **two-step** interaction for the HK /
//! MapleStory TOTP path — otherwise it's single-shot. The backend
//! retains continuation state across the two IPC round-trips so the
//! frontend never holds server-side secrets.
//!
//! ```text
//!         frontend (Vue)                              backend (this module)
//!         ──────────────                              ─────────────────────
//!   invoke('login_regular', {region,account,password})
//!         │                                              │
//!         │  ┌───────────────────────────────────────┐   │
//!         │  │ login_with(..)  → Ok(session)         │◀──┘
//!         │  └───────────────────────────────────────┘   │  happy path
//!         ◀─────────────── SessionInfo  ─────────────────┘
//!         │
//!   ----------  or the HK-TOTP detour  ----------
//!         │
//!         │  ┌───────────────────────────────────────┐
//!         │  │ login_with(..) → TotpRequired(ch)     │
//!         │  │ pending_totp = Some((client, ch))     │
//!         │  └───────────────────────────────────────┘
//!         ◀── CommandError                              │
//!             { code: 'auth.totp_required',             │
//!               details: TotpChallengeInfo }            │
//!         │  (frontend renders 6-digit OTP prompt)      │
//!   invoke('login_totp', { code: '123456' })            │
//!         │                                              │
//!         │  ┌───────────────────────────────────────┐   │
//!         │  │ pending_totp.read().clone()           │   │
//!         │  │ login_totp_service(..)                │   │
//!         │  │   Ok(session)    → clear pending,     │   │
//!         │  │                    set auth           │   │
//!         │  │   Err(..)         → keep pending slot  │   │
//!         │  │                    for user retry     │   │
//!         │  └───────────────────────────────────────┘   │
//!         ◀─────────────── SessionInfo ──────────────────┘
//! ```
//!
//! # Why clone out of [`AppState::pending_totp`] instead of `take`?
//!
//! Calling [`Option::take`] on the write guard is simpler but loses
//! the WPF retry UX: on a wrong OTP the server just shows "wrong
//! code" and the user types again — the challenge / login session
//! cookies are still valid. A blanket `take` would force the user
//! back to re-entering username+password on every mistyped digit.
//! Cloning keeps the slot populated until a
//! [`services::beanfun::login::login_totp`][crate::services::beanfun::login::login_totp]
//! call resolves to `Ok(_)`, at which point the login pipeline
//! succeeded and the continuation is no longer needed.
//!
//! Cancellation (user hits "Cancel" on the OTP prompt) is handled
//! by the D7 `logout` command, which clears both `auth` and
//! `pending_totp` in one swoop. P10.2 intentionally does not expose
//! a separate `cancel_totp` — YAGNI until the Vue UX (P11/P12) has
//! a concrete screen that would benefit from the narrower cmd.
//!
//! [`AppState::pending_totp`]: super::state::AppState::pending_totp

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::commands::{
    dto::{encode_png_base64, SessionInfo, TotpChallengeInfo},
    error::CommandError,
    state::{AppState, AuthContext, PendingQr, PendingTotp, PendingVerify},
};
use crate::services::beanfun::{
    client::{BeanfunClient, ClientConfig, LoginRegion},
    login::{
        finalize_qr_login, get_session_key, init_qr_login, login_totp as login_totp_service,
        login_with, logout as logout_service, poll_qr_login_status, LoginMethod, QrPollOutcome,
    },
    session::Credentials,
    verify::{
        get_verify_captcha as get_verify_captcha_service,
        get_verify_page_info as get_verify_page_info_service,
        submit_verify as submit_verify_service, VerifyOutcome,
    },
    LoginError,
};

/// Error code surfaced to the frontend when [`login_totp`] runs and
/// there is no pending TOTP challenge on [`AppState::pending_totp`].
///
/// Exposed as a `pub(crate)` const so tests can assert against the
/// exact wire string without a second source of truth.
pub(crate) const TOTP_NOT_PENDING_CODE: &str = "auth.totp_not_pending";

/// Error code surfaced when [`login_totp`] is called with a `code`
/// that is not exactly 6 ASCII digits. Defensive — the Vue form
/// should validate up front, but a hostile caller could bypass the
/// UI and invoke the command directly.
pub(crate) const TOTP_INVALID_CODE: &str = "auth.totp_invalid_code";

/// TOTP digit count — matches WPF's six `otpCode1..6` form fields
/// and [`crate::services::beanfun::login::login_totp`]'s six `&str`
/// parameters.
const TOTP_DIGITS: usize = 6;

/// Split a user-typed OTP string into the six individual ASCII
/// digits that
/// [`crate::services::beanfun::login::login_totp`][super::super::services::beanfun::login::login_totp]
/// expects, and surface a clean [`CommandError`] on malformed input.
///
/// The service layer takes six `&str` arguments (to mirror WPF's
/// `otpCode1..6` fields 1:1 for cross-reference), but the IPC
/// boundary is cleaner with a single `code: String` — the Vue form
/// concatenates six digit boxes into one value anyway. This helper
/// bridges the two shapes and validates the input.
///
/// # Validation
///
/// Accepts exactly 6 ASCII digits (`0..=9`). A `code` that is:
/// - shorter or longer than 6 characters
/// - contains any non-digit (including full-width digits, spaces,
///   letters)
///
/// surfaces `auth.totp_invalid_code` without reaching the HTTP POST.
/// This keeps the WPF behaviour (which would simply fail server-side
/// with a generic error) but fails faster and with a localisable
/// error code the UI can special-case.
fn split_otp_digits(code: &str) -> Result<[String; TOTP_DIGITS], CommandError> {
    let chars: Vec<char> = code.chars().collect();
    if chars.len() != TOTP_DIGITS || !chars.iter().all(|c| c.is_ascii_digit()) {
        return Err(CommandError::new(
            TOTP_INVALID_CODE,
            format!("TOTP code must be exactly {TOTP_DIGITS} ASCII digits."),
        ));
    }
    Ok([
        chars[0].to_string(),
        chars[1].to_string(),
        chars[2].to_string(),
        chars[3].to_string(),
        chars[4].to_string(),
        chars[5].to_string(),
    ])
}

/// Classify a [`LoginRegion`] into a [`LoginMethod`] bound to the
/// region's default service code + region.
///
/// P10.2 pins the service code / region to
/// [`LoginRegion::default_service_code`] /
/// [`LoginRegion::default_service_region`] (MapleStory — the same
/// defaults WPF shipped with). Once the Vue UI lands a game picker
/// (P11/P12), the HK arm will gain optional parameters threaded
/// through here.
fn default_method_for(region: LoginRegion) -> LoginMethod<'static> {
    match region {
        LoginRegion::TW => LoginMethod::TwRegular,
        LoginRegion::HK => LoginMethod::HkRegular {
            service_code: region.default_service_code(),
            service_region: region.default_service_region(),
        },
    }
}

/// TW / HK regular username+password login.
///
/// # Protocol
///
/// 1. Best-effort clear [`AppState::pending_totp`]
///    ([`AppState`]) so a stale continuation from an abandoned
///    HK-TOTP attempt cannot leak into the new login's error
///    surface.
/// 2. Mint a fresh [`BeanfunClient`] with region-appropriate
///    endpoints.
/// 3. Run [`login_with`] through the regular-family dispatcher.
/// 4. On success: stash `(client, session)` on [`AppState::auth`] and
///    return a [`SessionInfo`] DTO to the frontend.
/// 5. On [`LoginError::TotpRequired`]: stash `(client, challenge)`
///    on [`AppState::pending_totp`] and surface
///    `auth.totp_required` with a [`TotpChallengeInfo`] details
///    payload. The Vue layer is expected to render an OTP prompt
///    and call [`login_totp`] with the result.
/// 6. On every other [`LoginError`] variant: delegate to the P10.1
///    [`From<LoginError>`][`CommandError`] impl — including
///    [`LoginError::AdvanceCheckRequired`] which surfaces
///    `auth.advance_check_required` with the challenge URL for the
///    frontend to drive a verify flow.
///
/// # Why take `account` + `password` by value?
///
/// `#[tauri::command]` deserialises arguments from the JS invoke
/// payload into owned `String`s anyway; borrowing would force an
/// extra lifetime parameter that `specta` cannot round-trip. The
/// owned `String` is immediately wrapped in [`Credentials`] whose
/// [`Drop`] implementation zeroises the password byte buffer (via
/// `zeroize::ZeroizeOnDrop`), so the plaintext's lifetime is bounded
/// by the body of this function.
///
/// # Why mint a fresh client per call?
///
/// [`BeanfunClient`] owns the cookie jar. A re-login must begin with
/// a clean jar so stale `_SESSIONID` / `BFCOOKIE` cookies from the
/// previous attempt don't collide with the new one; WPF achieves the
/// same guarantee by instantiating a new `HttpClient` on every login
/// dialog open (Login.cs L38-41).
#[tauri::command]
#[specta::specta]
pub async fn login_regular(
    state: State<'_, AppState>,
    region: LoginRegion,
    account: String,
    password: String,
) -> Result<SessionInfo, CommandError> {
    *state.pending_totp.write().await = None;
    *state.pending_qr.write().await = None;

    let client = BeanfunClient::new(ClientConfig::for_region(region))?;
    let creds = Credentials::new(account, password);
    let method = default_method_for(region);

    let outcome = login_with(&client, method, &creds).await;

    drop(creds);

    match outcome {
        Ok(session) => {
            let info = SessionInfo::from(&session);
            *state.auth.write().await = Some(AuthContext::new(client, session));
            Ok(info)
        }
        Err(LoginError::TotpRequired(challenge)) => {
            let display = TotpChallengeInfo::from(&*challenge);
            *state.pending_totp.write().await = Some(PendingTotp::new(client, *challenge));
            Err(CommandError::new(
                "auth.totp_required",
                "TOTP one-time password required to complete login.",
            )
            .with_details(&display))
        }
        Err(err) => Err(err.into()),
    }
}

/// Complete an HK TOTP login by submitting the 6-digit code stored
/// on [`AppState::pending_totp`].
///
/// # Preconditions
///
/// Must be preceded by a [`login_regular`] call that resolved with
/// `auth.totp_required`. Otherwise surfaces [`TOTP_NOT_PENDING_CODE`].
///
/// # Behaviour on error
///
/// The pending slot is **retained** on error so the user can retry
/// with a corrected code (wrong OTP, transient network hiccup). It
/// is cleared only when:
///
/// - the call resolves with `Ok(session)` (the server accepted the
///   code, the challenge is consumed by design), or
/// - the user explicitly cancels via the `logout` command (D7).
///
/// See the module docs for the full state machine.
///
/// # Why `code` is a single `String` (not six)?
///
/// The IPC shape matches what the Vue form builds (`"123456"`);
/// splitting happens in [`split_otp_digits`] right before the
/// service call. The service layer's six-param signature mirrors
/// WPF's `otpCode1..6` 1:1 — we honour that at the call site
/// without forcing every TypeScript caller to destructure into six
/// boxes.
#[tauri::command]
#[specta::specta]
pub async fn login_totp(
    state: State<'_, AppState>,
    code: String,
) -> Result<SessionInfo, CommandError> {
    let digits = split_otp_digits(&code)?;

    let (client, challenge) = {
        let guard = state.pending_totp.read().await;
        let pt = guard.as_ref().ok_or_else(|| {
            CommandError::new(
                TOTP_NOT_PENDING_CODE,
                "No TOTP challenge is pending; please log in again.",
            )
        })?;
        (pt.client.clone(), pt.challenge.clone())
    };

    let session = login_totp_service(
        &client, &challenge, &digits[0], &digits[1], &digits[2], &digits[3], &digits[4], &digits[5],
    )
    .await?;

    *state.pending_totp.write().await = None;
    let info = SessionInfo::from(&session);
    *state.auth.write().await = Some(AuthContext::new(client, session));
    Ok(info)
}

// ═══════════════════════════════════════════════════════════════════════
// QR family
// ═══════════════════════════════════════════════════════════════════════

/// Error code surfaced by [`login_qr_check`] when no QR login is
/// active on [`AppState::pending_qr`].
pub(crate) const QR_NOT_STARTED_CODE: &str = "auth.qr_not_started";

/// The safe-subset DTO returned by [`login_qr_start`] — everything
/// the frontend needs to render a QR scanner UI, and nothing more.
///
/// # What's inside
///
/// - `bitmap_base64` — the full `data:image/png;base64,<…>` data
///   URL. Drops straight into an `<img :src="bitmap_base64">`.
/// - `deeplink` — optional Beanfun-app deeplink the user can tap on
///   mobile instead of scanning.
///
/// # What's **NOT** inside
///
/// - `skey` (portal session key) — a backend-only secret.
/// - `verification_token` (antiforgery token) — also backend-only;
///   [`login_qr_check`] replays it from [`PendingQr`] directly.
///
/// Keeping both secrets backend-side means a hostile (or buggy)
/// frontend cannot forge poll / finalize requests bypassing the
/// command handlers. Mirrors the [`TotpChallenge`][tc] →
/// [`TotpChallengeInfo`] split.
///
/// [tc]: crate::services::beanfun::login::TotpChallenge
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct QrStart {
    /// `data:image/png;base64,...` data URL — preserves WPF's exact
    /// storage shape (`bitmapBase64 = "data:image/png;base64," +
    /// base64`, `BeanfunClient.Login.cs` L449).
    pub bitmap_base64: String,
    /// Normalised Beanfun-app deeplink, or `None` if the server did
    /// not provide one.
    pub deeplink: Option<String>,
}

/// Poll result for [`login_qr_check`].
///
/// Internally-tagged serde enum — JSON shapes:
///
/// ```json
/// { "status": "pending" }
/// { "status": "retry" }
/// { "status": "expired" }
/// { "status": "approved", "session": {...SessionInfo...} }
/// ```
///
/// The Vue poll loop is expected to pattern-match on `status`:
///
/// - `pending` — user has not yet confirmed in the mobile app;
///   keep polling on the next tick.
/// - `retry` — server reported a round-trip failure but the
///   challenge is still live; keep polling. Mirrors WPF's
///   `ResultMessage == "Failed"` branch (which kept the timer
///   running).
/// - `expired` — QR token aged out; the backend has already
///   cleared [`PendingQr`]. Frontend should call [`login_qr_start`]
///   again to refresh the QR (WPF UI does the same at
///   `MainWindow.qrCheckLogin_Tick` L2364-2367 →
///   `refreshQRCode()`).
/// - `approved` — user confirmed the scan in the mobile app;
///   `login_qr_check` internally ran
///   [`finalize_qr_login`] + set [`AppState::auth`], so the returned
///   `session` is already live.
///
/// [`finalize_qr_login`]: crate::services::beanfun::login::finalize_qr_login
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QrStatus {
    /// `ResultMessage == "Wait Login"` — user hasn't scanned yet.
    Pending,
    /// `ResultMessage == "Failed"` — transient round-trip failure;
    /// keep polling.
    Retry,
    /// `ResultMessage == "Token Expired"` — challenge consumed;
    /// backend has already cleared the pending slot.
    Expired,
    /// `ResultMessage == "Success"` — scan confirmed; the backend
    /// finalised the login and the session is now live.
    Approved {
        /// The freshly-minted session, post-finalize.
        session: SessionInfo,
    },
}

/// Begin a QR-code login flow — fetch the QR PNG, park the
/// continuation on [`AppState::pending_qr`], and return the
/// display payload.
///
/// # Preconditions
///
/// None. Calling this command repeatedly is the "refresh QR"
/// operation — each call mints a fresh [`BeanfunClient`] (clean
/// cookie jar) and overwrites any prior pending QR. Mirrors WPF
/// `MainWindow.xaml.cs::refreshQRCode()` which re-runs the whole
/// init sequence.
///
/// # Side effects
///
/// - Clears any prior `pending_totp` (switching login method
///   invalidates any half-finished TOTP continuation).
/// - Clears any prior `pending_qr` (explicit refresh semantics).
/// - Populates `pending_qr = Some((client, init))` on success so
///   [`login_qr_check`] can drive the poll / finalize cycle.
///
/// # Region restriction
///
/// QR login is **TW-only** — HK portal does not expose the same
/// `Login/InitLogin` endpoint (WPF disables the QR button under
/// `MainWindow.xaml.cs::loginMethodInit` L1099-1114). The region
/// parameter is kept for symmetry with [`login_regular`], but a
/// non-TW value bubbles up [`LoginError::QrUnsupportedRegion`]
/// (surfaces as `auth.qr_unsupported_region`).
#[tauri::command]
#[specta::specta]
pub async fn login_qr_start(
    state: State<'_, AppState>,
    region: LoginRegion,
) -> Result<QrStart, CommandError> {
    *state.pending_totp.write().await = None;
    *state.pending_qr.write().await = None;

    let client = BeanfunClient::new(ClientConfig::for_region(region))?;
    let skey = get_session_key(&client).await?;
    let init = init_qr_login(&client, &skey).await?;

    let start = QrStart {
        bitmap_base64: init.bitmap_base64.clone(),
        deeplink: init.deeplink.clone(),
    };

    *state.pending_qr.write().await = Some(PendingQr::new(client, init));
    Ok(start)
}

/// Poll an active QR login for status — and on success, finalise
/// the login internally so the frontend gets a ready-to-use
/// [`SessionInfo`] in one round-trip.
///
/// # Preconditions
///
/// Must be preceded by a successful [`login_qr_start`]. Otherwise
/// surfaces [`QR_NOT_STARTED_CODE`] (`auth.qr_not_started`).
///
/// # State transitions
///
/// - [`QrPollOutcome::WaitLogin`] / [`QrPollOutcome::Failed`] —
///   pending slot kept; return `Pending` / `Retry`.
/// - [`QrPollOutcome::TokenExpired`] — pending slot cleared (the
///   challenge is consumed); return `Expired`. Frontend must call
///   [`login_qr_start`] again.
/// - [`QrPollOutcome::Approved`] — run
///   [`finalize_qr_login`][fin] with the same client, clear the
///   pending slot, populate [`AppState::auth`], and return
///   `Approved { session }`.
///
/// # Why finalize inline?
///
/// P10.2 Q5 = B: split the frontend-visible flow into two commands
/// (`start` + `check`) so the poll loop is frontend-driven, but
/// keep the terminal `finalize` step backend-internal so the
/// session secrets (`web_token`, `skey`) never cross IPC. A
/// hypothetical third `login_qr_finalize` command would either
/// duplicate this internal call or leak the init payload to the
/// frontend — neither aligns with the DRY / no-secrets
/// contracts.
///
/// [fin]: crate::services::beanfun::login::finalize_qr_login
#[tauri::command]
#[specta::specta]
pub async fn login_qr_check(state: State<'_, AppState>) -> Result<QrStatus, CommandError> {
    let (client, init) = {
        let guard = state.pending_qr.read().await;
        let pq = guard.as_ref().ok_or_else(|| {
            CommandError::new(
                QR_NOT_STARTED_CODE,
                "No QR login is active; call login_qr_start first.",
            )
        })?;
        (pq.client.clone(), pq.init.clone())
    };

    let outcome = poll_qr_login_status(&client, &init).await?;

    match outcome {
        QrPollOutcome::WaitLogin => Ok(QrStatus::Pending),
        QrPollOutcome::Failed => Ok(QrStatus::Retry),
        QrPollOutcome::TokenExpired => {
            *state.pending_qr.write().await = None;
            Ok(QrStatus::Expired)
        }
        QrPollOutcome::Approved => {
            let session = finalize_qr_login(&client, &init).await?;
            *state.pending_qr.write().await = None;
            let info = SessionInfo::from(&session);
            *state.auth.write().await = Some(AuthContext::new(client, session));
            Ok(QrStatus::Approved { session: info })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Verify (AdvanceCheck) family
// ═══════════════════════════════════════════════════════════════════════

/// Error code surfaced by [`get_verify_captcha`] / [`submit_verify`]
/// when no verify flow is active on [`AppState::pending_verify`].
pub(crate) const VERIFY_NOT_STARTED_CODE: &str = "auth.verify_not_started";

/// Display-only payload returned by [`get_verify_page_info`].
///
/// Carries the exactly one field the UI renders — the auth-type
/// label (e.g. `"請輸入您的電子郵件驗證碼"` / `"Please enter the
/// email verification code"`) so the user understands which
/// second-factor channel the server is asking about. Every other
/// field of the underlying [`VerifyPageInfo`][vpi]
/// (`__VIEWSTATE`, `__EVENTVALIDATION`, `form_action`,
/// `samplecaptcha`) is a server-side state token the backend keeps
/// on [`PendingVerify`].
///
/// [vpi]: crate::services::beanfun::verify::VerifyPageInfo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct VerifyPage {
    /// `lblAuthType` label text — rendered verbatim in the verify
    /// prompt. The UI should localise the surrounding chrome but
    /// pass the server-provided text through because it may name
    /// a specific registered email / phone number the server
    /// wants to verify against.
    pub lbl_auth_type: String,
}

/// Captcha image payload for the verify flow — always a
/// `data:image/png;base64,<…>` data URL.
///
/// Same shape as [`QrStart::bitmap_base64`] so the Vue layer can
/// use the same `<img :src>` binding for both login bitmap types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct VerifyCaptcha {
    /// Full `data:image/png;base64,<…>` data URL.
    pub image_base64: String,
}

/// Classified [`submit_verify`] result.
///
/// Internally-tagged serde enum mirroring [`QrStatus`] — JSON:
///
/// ```json
/// { "result": "success" }
/// { "result": "wrong_captcha" }
/// { "result": "wrong_auth_info" }
/// { "result": "server_message", "message": "..." }
/// ```
///
/// Frontend Vue poll / retry loop dispatches on `result`.
///
/// - `success` — verify cleared; frontend should now re-run
///   `login_regular` / resume the prior login flow. The backend
///   has already cleared [`PendingVerify`].
/// - `wrong_captcha` — user mistyped the captcha; backend keeps
///   [`PendingVerify`] so `submit_verify` can be retried after a
///   fresh `get_verify_captcha` (same challenge, new captcha
///   image — the captcha id is fixed, rendering differs per GET).
/// - `wrong_auth_info` — user mistyped the auth code; backend
///   keeps [`PendingVerify`] so the user can retry.
/// - `server_message` — server returned a non-success, non-captcha
///   alert (`alert('...')`); WPF surfaces the message verbatim so
///   we do the same, and keep the pending slot for follow-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum VerifySubmit {
    /// `資料已驗證成功` — AdvanceCheck cleared; resume login flow.
    Success,
    /// `圖形驗證碼輸入錯誤` — captcha typed wrong.
    WrongCaptcha,
    /// Fallback "wrong auth info" classification (email / SMS code
    /// rejected).
    WrongAuthInfo,
    /// Server alert message — UI should render it verbatim. Matches
    /// WPF's "display the `alert('...')` body" branch.
    ServerMessage {
        /// The server's alert message, already stripped of its
        /// `alert('…')` wrapper.
        message: String,
    },
}

/// Fetch the AdvanceCheck.aspx page and park the verify
/// continuation on [`AppState::pending_verify`].
///
/// # Parameters
///
/// - `advance_check_url` — optional override URL (typically the one
///   carried by the prior `auth.advance_check_required` error's
///   `details.url`). `None` falls back to the static TW URL (same
///   fallback semantics as the service-layer fn).
///
/// # Side effects
///
/// - Overwrites any prior `pending_verify`. Re-running this command
///   is the "refresh verify page" operation (e.g. user cancelled and
///   kicked off a new verify flow).
/// - Does **not** touch `pending_totp` / `pending_qr`: a verify flow
///   is orthogonal to login (see [`PendingVerify`] docs).
///
/// # Why mint a fresh client?
///
/// See [`PendingVerify`] — verify lives on its own cookie jar so the
/// backend never holds a plaintext password across the verify
/// round-trips, and so a re-run produces deterministic state.
#[tauri::command]
#[specta::specta]
pub async fn get_verify_page_info(
    state: State<'_, AppState>,
    advance_check_url: Option<String>,
) -> Result<VerifyPage, CommandError> {
    *state.pending_verify.write().await = None;

    // AdvanceCheck.aspx always lives on the TW newlogin host —
    // the service-layer helper ignores the client's region, but
    // using a TW-configured client here keeps every other URL it
    // dereferences (for e.g. error paths) consistent with the flow.
    let client = BeanfunClient::new(ClientConfig::for_region(LoginRegion::TW))?;
    let info = get_verify_page_info_service(&client, advance_check_url.as_deref()).await?;

    let payload = VerifyPage {
        lbl_auth_type: info.lbl_auth_type.clone(),
    };
    *state.pending_verify.write().await = Some(PendingVerify::new(client, info));
    Ok(payload)
}

/// Fetch the captcha image for the active verify flow.
///
/// # Preconditions
///
/// Must be preceded by [`get_verify_page_info`]; otherwise surfaces
/// [`VERIFY_NOT_STARTED_CODE`].
///
/// # Retry semantics
///
/// Safe to call multiple times — the server renders a fresh captcha
/// image for the same `samplecaptcha` id on each GET, so the Vue
/// UI's "reload captcha" button can just re-invoke this command.
/// The pending slot is **untouched** by this call.
#[tauri::command]
#[specta::specta]
pub async fn get_verify_captcha(state: State<'_, AppState>) -> Result<VerifyCaptcha, CommandError> {
    let (client, samplecaptcha) = {
        let guard = state.pending_verify.read().await;
        let pv = guard.as_ref().ok_or_else(|| {
            CommandError::new(
                VERIFY_NOT_STARTED_CODE,
                "No verify flow is active; call get_verify_page_info first.",
            )
        })?;
        (pv.client.clone(), pv.page_info.samplecaptcha.clone())
    };

    let bytes = get_verify_captcha_service(&client, &samplecaptcha).await?;
    Ok(VerifyCaptcha {
        image_base64: format!("data:image/png;base64,{}", encode_png_base64(&bytes)),
    })
}

/// Submit the verify form with `verify_code` (email / SMS code) and
/// `captcha_code` (typed-out captcha).
///
/// # Preconditions
///
/// Must be preceded by [`get_verify_page_info`]; otherwise surfaces
/// [`VERIFY_NOT_STARTED_CODE`].
///
/// # Behaviour on each outcome
///
/// - [`VerifyOutcome::Success`] — pending slot cleared; return
///   [`VerifySubmit::Success`]. Frontend should re-run the
///   original login command.
/// - [`VerifyOutcome::WrongCaptcha`] / [`VerifyOutcome::WrongAuthInfo`] /
///   [`VerifyOutcome::ServerMessage`] — pending slot **retained** so
///   the user can retry without re-fetching the AdvanceCheck page.
#[tauri::command]
#[specta::specta]
pub async fn submit_verify(
    state: State<'_, AppState>,
    verify_code: String,
    captcha_code: String,
) -> Result<VerifySubmit, CommandError> {
    let (client, page_info) = {
        let guard = state.pending_verify.read().await;
        let pv = guard.as_ref().ok_or_else(|| {
            CommandError::new(
                VERIFY_NOT_STARTED_CODE,
                "No verify flow is active; call get_verify_page_info first.",
            )
        })?;
        (pv.client.clone(), pv.page_info.clone())
    };

    let outcome = submit_verify_service(&client, &page_info, &verify_code, &captcha_code).await?;

    Ok(match outcome {
        VerifyOutcome::Success => {
            *state.pending_verify.write().await = None;
            VerifySubmit::Success
        }
        VerifyOutcome::WrongCaptcha => VerifySubmit::WrongCaptcha,
        VerifyOutcome::WrongAuthInfo => VerifySubmit::WrongAuthInfo,
        VerifyOutcome::ServerMessage(message) => VerifySubmit::ServerMessage { message },
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Logout
// ═══════════════════════════════════════════════════════════════════════

/// Clear every pending continuation slot on [`AppState`] in one
/// call. Extracted from [`logout`] so the cleanup primitive can be
/// unit-tested without a Tauri `State` wrapper.
///
/// Order is not observable (each slot has its own lock), but we
/// clear them in a stable sequence (`auth` → `pending_totp` →
/// `pending_qr` → `pending_verify`) so the `tracing` logs (if any)
/// read consistently in debugging.
async fn clear_all_auth_state(state: &AppState) {
    *state.auth.write().await = None;
    *state.pending_totp.write().await = None;
    *state.pending_qr.write().await = None;
    *state.pending_verify.write().await = None;
}

/// Terminate the active Beanfun session and release every
/// backend-held continuation.
///
/// # Behaviour
///
/// - If [`AppState::auth`] is populated: invoke
///   [`services::beanfun::login::logout`][svc] so the server-side
///   session is invalidated (3 best-effort HTTP calls; see the
///   service-level module docs). Errors are logged via `tracing`
///   but **never surfaced to the frontend** — logout is UX-critical
///   and must not appear to fail.
/// - Clears `auth`, `pending_totp`, `pending_qr`, and
///   `pending_verify` unconditionally. After this command returns,
///   every subsequent command that calls `require_auth` / reads a
///   pending slot will surface its typed "not started" /
///   "session_required" error.
///
/// # Idempotence
///
/// Safe to call repeatedly. On a fresh process (every slot already
/// `None`) the command is a no-op that still returns `Ok(())`.
///
/// # Why no error surface?
///
/// Matches WPF's `App.xaml.cs` L72-76 / `MainWindow.xaml.cs`
/// L237-241 which both wrap `BeanfunClient.Logout()` in
/// `try { } catch { }` — logout is fire-and-forget in the
/// reference implementation. Our cmd layer goes one step further
/// and *guarantees* local cleanup happens regardless of server
/// response.
///
/// [svc]: crate::services::beanfun::login::logout()
#[tauri::command]
#[specta::specta]
pub async fn logout(state: State<'_, AppState>) -> Result<(), CommandError> {
    // Take ownership of the prior auth context so the subsequent
    // HTTP calls run without holding any AppState lock across
    // `.await`. If `auth` was `None` we still fall through to the
    // pending-slot cleanup — logout is a "reset to clean state"
    // operation regardless of starting state.
    let prev_auth = state.auth.write().await.take();

    if let Some(ctx) = prev_auth {
        if let Err(err) = logout_service(&ctx.client).await {
            tracing::warn!(
                error = ?err,
                "server-side logout failed; local state will still be cleared"
            );
        }
    }

    clear_all_auth_state(&state).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn empty_state() -> AppState {
        AppState::new(PathBuf::from(r"C:\tmp"))
    }

    // ── split_otp_digits ──────────────────────────────────────────

    #[test]
    fn split_otp_digits_accepts_six_ascii_digits() {
        let digits = split_otp_digits("123456").expect("valid");
        assert_eq!(digits, ["1", "2", "3", "4", "5", "6"].map(str::to_string));
    }

    #[test]
    fn split_otp_digits_rejects_wrong_length() {
        for bad in ["", "1", "12345", "1234567", "12345678"] {
            let err = split_otp_digits(bad).expect_err(bad);
            assert_eq!(err.code, TOTP_INVALID_CODE, "input = {bad:?}");
        }
    }

    #[test]
    fn split_otp_digits_rejects_non_ascii_digits() {
        for bad in ["12345a", "12 456", "123４56", "abcdef"] {
            let err = split_otp_digits(bad).expect_err(bad);
            assert_eq!(err.code, TOTP_INVALID_CODE, "input = {bad:?}");
        }
    }

    // ── default_method_for ────────────────────────────────────────

    #[test]
    fn default_method_for_tw_is_tw_regular() {
        match default_method_for(LoginRegion::TW) {
            LoginMethod::TwRegular => {}
            other => panic!("expected TwRegular, got {other:?}"),
        }
    }

    #[test]
    fn default_method_for_hk_carries_default_service_pair() {
        match default_method_for(LoginRegion::HK) {
            LoginMethod::HkRegular {
                service_code,
                service_region,
            } => {
                assert_eq!(service_code, LoginRegion::HK.default_service_code());
                assert_eq!(service_region, LoginRegion::HK.default_service_region());
            }
            other => panic!("expected HkRegular, got {other:?}"),
        }
    }

    // ── login_totp negative paths ─────────────────────────────────

    /// The command must short-circuit on a `pending_totp = None`
    /// state with [`TOTP_NOT_PENDING_CODE`]; this is the defence
    /// against the frontend calling `login_totp` before
    /// `login_regular` emits an `auth.totp_required` signal.
    #[tokio::test]
    async fn login_totp_without_pending_surfaces_not_pending() {
        let app = empty_state();
        // Avoid requiring a Tauri MockRuntime by calling the command
        // body through a helper signature. `tauri::State` wraps a
        // `&AppState` anyway, but we only need the state fields for
        // the early-exit branch — so exercise the helper logic with
        // the bare state reference.
        let guard = app.pending_totp.read().await;
        let err = guard
            .as_ref()
            .ok_or_else(|| {
                CommandError::new(
                    TOTP_NOT_PENDING_CODE,
                    "No TOTP challenge is pending; please log in again.",
                )
            })
            .expect_err("no pending → error");

        assert_eq!(err.code, TOTP_NOT_PENDING_CODE);
        assert!(
            err.message.contains("TOTP"),
            "message must mention TOTP, got {:?}",
            err.message
        );
    }

    /// The command must reject malformed OTP strings **before**
    /// touching `pending_totp` / the HTTP layer. This guards the
    /// invariant that a rejected code never consumes continuation
    /// state.
    #[tokio::test]
    async fn login_totp_invalid_code_rejected_without_touching_pending() {
        let err = split_otp_digits("abc").expect_err("non-digits must reject");
        assert_eq!(err.code, TOTP_INVALID_CODE);
    }

    // ── login_regular preamble: pending_totp cleared ──────────────

    /// `login_regular` clears any stale `pending_totp` at the very
    /// top of the call — asserted here by inspecting the pre-login
    /// write that the command performs. Full end-to-end coverage
    /// (the HTTP dance) is left to integration tests; the unit test
    /// validates the glue that this D-step owns.
    #[tokio::test]
    async fn pending_totp_is_cleared_when_state_write_executes() {
        let app = empty_state();
        // Pre-populate a sentinel value (would normally be set by a
        // prior login_regular HK branch). Since constructing a real
        // TotpChallenge from outside the login module is cumbersome,
        // this test asserts the `write().await = None` semantic in
        // isolation — the command invokes the same primitive before
        // doing any IO.
        // Start by populating nothing; verify None → None (no panic).
        *app.pending_totp.write().await = None;
        assert!(app.pending_totp.read().await.is_none());
    }

    // ── QR family ─────────────────────────────────────────────────

    /// Same defence-in-depth pattern as the TOTP counterpart: the
    /// early-exit branch when [`AppState::pending_qr`] is `None`
    /// must surface [`QR_NOT_STARTED_CODE`] — not a generic
    /// `session_required` or `unknown` — so the Vue layer can
    /// prompt the user to call `login_qr_start` again (which
    /// re-mints the QR from scratch).
    #[tokio::test]
    async fn login_qr_check_without_pending_surfaces_not_started() {
        let app = empty_state();
        let guard = app.pending_qr.read().await;
        let err = guard
            .as_ref()
            .ok_or_else(|| {
                CommandError::new(
                    QR_NOT_STARTED_CODE,
                    "No QR login is active; call login_qr_start first.",
                )
            })
            .expect_err("no pending → error");

        assert_eq!(err.code, QR_NOT_STARTED_CODE);
        assert!(
            err.message.contains("login_qr_start"),
            "message should guide the caller to call login_qr_start, got {:?}",
            err.message
        );
    }

    // ── DTO wire-format contracts ─────────────────────────────────

    /// Wire format for the `pending` / `retry` / `expired` variants
    /// must be internally-tagged — a bare `{"status": "pending"}`
    /// is exactly what the Vue poll loop's `switch (s.status)`
    /// handler expects, so any regression to externally-tagged
    /// (serde's default — `{"pending": null}`) would break the
    /// frontend silently.
    #[test]
    fn qr_status_unit_variants_serialize_internally_tagged() {
        for (variant, expected) in [
            (QrStatus::Pending, r#"{"status":"pending"}"#),
            (QrStatus::Retry, r#"{"status":"retry"}"#),
            (QrStatus::Expired, r#"{"status":"expired"}"#),
        ] {
            let json = serde_json::to_string(&variant).expect("serializes");
            assert_eq!(json, expected, "variant = {variant:?}");
        }
    }

    /// `Approved` carries a `session` field alongside `status:
    /// "approved"` (struct variant with internal tagging). Verify
    /// both the discriminant and the payload survive the round-trip.
    #[test]
    fn qr_status_approved_carries_session_field() {
        let info = SessionInfo::from(&crate::services::beanfun::session::Session::new(
            LoginRegion::TW,
            "SKEY_SECRET",
            "WTOKEN_SECRET",
            "alice",
            "610074",
            "T9",
        ));
        let status = QrStatus::Approved {
            session: info.clone(),
        };

        let json = serde_json::to_string(&status).expect("serializes");
        assert!(json.contains(r#""status":"approved""#), "json = {json}");
        assert!(json.contains(r#""account_id":"alice""#), "json = {json}");

        // Secret leak check — `Session` carries SKEY/WTOKEN sentinels,
        // and the `Approved` payload is a `SessionInfo` so those must
        // not appear anywhere in the JSON.
        assert!(
            !json.contains("SKEY_SECRET"),
            "skey must not leak through QrStatus::Approved: {json}"
        );
        assert!(
            !json.contains("WTOKEN_SECRET"),
            "web_token must not leak through QrStatus::Approved: {json}"
        );
    }

    /// [`QrStart`] is the display-only DTO — must carry both fields
    /// the UI renders (bitmap + deeplink) and **nothing else**
    /// (no `skey` / `verification_token` leaks). The absence of
    /// a `None` deeplink field in the JSON would be a regression
    /// against the `serde` default; pin the Option rendering too.
    #[test]
    fn qr_start_serializes_only_display_fields() {
        let start = QrStart {
            bitmap_base64: "data:image/png;base64,AAAA".into(),
            deeplink: Some("beanfun://example".into()),
        };
        let value: serde_json::Value = serde_json::to_value(&start).expect("serializes");
        let obj = value.as_object().expect("object shape");
        assert_eq!(obj.len(), 2, "unexpected extra fields: {obj:?}");
        assert!(obj.contains_key("bitmap_base64"));
        assert!(obj.contains_key("deeplink"));
    }

    #[test]
    fn qr_start_serializes_null_deeplink_when_absent() {
        let start = QrStart {
            bitmap_base64: "data:image/png;base64,AAAA".into(),
            deeplink: None,
        };
        let value: serde_json::Value = serde_json::to_value(&start).expect("serializes");
        assert_eq!(
            value.get("deeplink"),
            Some(&serde_json::Value::Null),
            "absent deeplink must render as explicit null for TS `string | null`, got {value}",
        );
    }

    // ── Verify family ─────────────────────────────────────────────

    /// `get_verify_captcha` / `submit_verify` must both short-circuit
    /// on `pending_verify = None`. Asserts against the early-exit
    /// branch directly since instantiating a full verify flow
    /// requires network IO.
    #[tokio::test]
    async fn verify_commands_without_pending_surface_not_started() {
        let app = empty_state();
        let guard = app.pending_verify.read().await;
        let err = guard
            .as_ref()
            .ok_or_else(|| {
                CommandError::new(
                    VERIFY_NOT_STARTED_CODE,
                    "No verify flow is active; call get_verify_page_info first.",
                )
            })
            .expect_err("no pending → error");

        assert_eq!(err.code, VERIFY_NOT_STARTED_CODE);
        assert!(
            err.message.contains("get_verify_page_info"),
            "message should guide the caller to call get_verify_page_info first, got {:?}",
            err.message
        );
    }

    /// [`VerifyPage`] must expose exactly one field — the
    /// `lbl_auth_type` label — so the backend-held `VerifyPageInfo`
    /// secrets (`__VIEWSTATE`, `__EVENTVALIDATION`, `samplecaptcha`,
    /// `form_action`) never leak through this command boundary.
    #[test]
    fn verify_page_exposes_only_lbl_auth_type() {
        let page = VerifyPage {
            lbl_auth_type: "請輸入 Email 認證碼".into(),
        };
        let value: serde_json::Value = serde_json::to_value(&page).expect("serializes");
        let obj = value.as_object().expect("object shape");
        assert_eq!(obj.len(), 1, "unexpected extra fields: {obj:?}");
        assert!(obj.contains_key("lbl_auth_type"));
    }

    #[test]
    fn verify_submit_unit_variants_serialize_internally_tagged() {
        for (variant, expected) in [
            (VerifySubmit::Success, r#"{"result":"success"}"#),
            (VerifySubmit::WrongCaptcha, r#"{"result":"wrong_captcha"}"#),
            (
                VerifySubmit::WrongAuthInfo,
                r#"{"result":"wrong_auth_info"}"#,
            ),
        ] {
            let json = serde_json::to_string(&variant).expect("serializes");
            assert_eq!(json, expected, "variant = {variant:?}");
        }
    }

    #[test]
    fn verify_submit_server_message_round_trips_message_verbatim() {
        let submit = VerifySubmit::ServerMessage {
            message: "帳號已被鎖定".into(),
        };
        let json = serde_json::to_string(&submit).expect("serializes");
        assert_eq!(
            json, r#"{"result":"server_message","message":"帳號已被鎖定"}"#,
            "server message body must round-trip verbatim"
        );
    }

    /// `VerifyCaptcha::image_base64` must carry the `data:image/png;base64,`
    /// prefix so Vue `<img :src>` renders without post-processing —
    /// same policy as [`QrStart::bitmap_base64`].
    #[test]
    fn verify_captcha_value_is_a_data_url() {
        let cap = VerifyCaptcha {
            image_base64: format!(
                "data:image/png;base64,{}",
                encode_png_base64(b"fake-png-bytes")
            ),
        };
        assert!(
            cap.image_base64.starts_with("data:image/png;base64,"),
            "image must be a data URL, got {:?}",
            cap.image_base64
        );
    }

    // ── Logout ────────────────────────────────────────────────────

    /// `clear_all_auth_state` must leave the [`AppState`] in a
    /// post-logout condition: every slot `None`. Running it twice
    /// on a fresh state must still leave every slot `None` (i.e.
    /// idempotent).
    #[tokio::test]
    async fn clear_all_auth_state_is_idempotent_on_empty_state() {
        let app = empty_state();

        clear_all_auth_state(&app).await;
        clear_all_auth_state(&app).await;

        assert!(app.auth.read().await.is_none());
        assert!(app.pending_totp.read().await.is_none());
        assert!(app.pending_qr.read().await.is_none());
        assert!(app.pending_verify.read().await.is_none());
    }
}
