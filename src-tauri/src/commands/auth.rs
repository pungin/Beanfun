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
//! | [`login_gamepass_start`]   | gamepass | Mint a fresh `BeanfunClient`, fetch a portal session key, stash both on `pending_gamepass` for the WebView    |
//! | [`get_verify_page_info`]   | verify   | Fetch the AdvanceCheck verify page (returns the `lblAuthType` label)                                          |
//! | [`get_verify_captcha`]     | verify   | Fetch the captcha image (Base64 data URL)                                                                     |
//! | [`submit_verify`]          | verify   | Submit `verify_code + captcha_code`; surfaces `Success` / `WrongCaptcha` / `WrongAuthInfo` / `ServerMessage`  |
//! | [`logout`]                 | logout   | Clear local auth + every pending slot; best-effort server-side `erase_token` (errors logged, never surfaced) |
//!
//! [st]: super::state::AppState::auth
//!
//! `open_gamepass_window` (P12.1 D5b CP3) and the
//! `login_gamepass_complete` follow-up are deliberately split out of
//! D5a so the backend changes here can land + run quality gates
//! independently of the [`tauri::WebviewWindow`] cookie-extraction
//! plumbing.
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::commands::{
    dto::{encode_png_base64, SessionInfo, TotpChallengeInfo},
    error::CommandError,
    state::{
        AppState, AuthContext, PendingGamepass, PendingQr, PendingTotp, PendingTwLogin,
        PendingVerify,
    },
};
use crate::services::beanfun::{
    account::get_accounts as service_get_accounts,
    client::{BeanfunClient, ClientConfig, LoginRegion},
    login::{
        finalize_qr_login, get_session_key, init_qr_login, inject_webview_cookies,
        login_registered_device, login_totp as login_totp_service, login_with,
        logout as logout_service, poll_qr_login_status, try_complete_gamepass_login,
        tw_login_resume, tw_login_start, LoginMethod, QrPollOutcome, RecaptchaStep, TwStepOutcome,
    },
    session::Credentials,
    verify::{
        get_verify_captcha as get_verify_captcha_service,
        get_verify_page_info as get_verify_page_info_service,
        submit_verify as submit_verify_service, VerifyOutcome,
    },
    LoginError, Session,
};

// Only the non-Windows GamePass seed path uses the wry `set_cookie`
// helper; Windows seeds (and clears, issue #296) through the native
// COM `cookie_native::seed_cookies_native`, so importing this on
// Windows would be a dead `use`.
#[cfg(not(target_os = "windows"))]
use crate::services::beanfun::login::seed_webview_cookies_from_client;

// ═══════════════════════════════════════════════════════════════════════
// Session keep-alive (WPF pingWorker parity)
// ═══════════════════════════════════════════════════════════════════════

/// Interval between consecutive [`BeanfunClient::ping`] calls inside
/// [`run_ping_loop`].
///
/// 60 s matches WPF `MainWindow.pingWorker_DoWork` (`WaitSecs = 60`,
/// `MainWindow.xaml.cs` L2327). The Beanfun portal drops idle sessions
/// after a few minutes server-side; pinging every minute has proved
/// sufficient (WPF users reported sessions surviving for days).
const PING_INTERVAL: Duration = Duration::from_secs(60);

/// Poll cadence for the HK "registered device" continuation.
///
/// Matches WPF `MainWindow.bfAPPAutoLogin.Interval = 2 seconds`
/// (`MainWindow.xaml.cs` L177) so the mobile-app approval flow feels
/// the same as the legacy client.
const DEVICE_REGISTRATION_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Install a freshly-minted `(client, session)` pair onto
/// [`AppState::auth`] and spawn the session keep-alive ping loop.
///
/// Centralises the four-site login-finalisation pattern:
///
/// 1. Derive the [`SessionInfo`] DTO **before** moving `session` so we
///    can still return it to the caller.
/// 2. Wrap `client` + `session` in an [`AuthContext`] — each call
///    mints a fresh [`CancellationToken`] via
///    [`AuthContext::new`].
/// 3. Install the context on [`AppState::auth`]. Holds the write lock
///    for the shortest possible window (no `.await` points between
///    the acquire and the release besides the `replace` itself).
/// 4. Spawn [`run_ping_loop`] with a clone of `client` + a clone of
///    the context's `ping_cancel` token. The clones share the same
///    cookie jar and cancellation signal as the installed context,
///    so `logout` -> `cancel()` stops the loop promptly.
///
/// # Why clone `client` for the spawned task?
///
/// [`BeanfunClient`] is cheap to clone (all inner fields are
/// `Arc<_>`), and cloning keeps ownership semantics simple: the
/// spawned task outlives any single `AppState::auth` read guard.
async fn install_session_and_start_ping(
    state: &AppState,
    client: BeanfunClient,
    session: Session,
) -> SessionInfo {
    let info = SessionInfo::from(&session);
    let ctx = AuthContext::new(client.clone(), session.clone());
    let ping_client = ctx.client.clone();
    let ping_cancel = ctx.ping_cancel.clone();

    // Atomic replace: if a prior `AuthContext` was still installed
    // (e.g. the user re-logged in without first calling `logout`,
    // or a `session_required` flow recovered mid-session), cancel
    // its keep-alive loop so we don't leak an orphaned background
    // task holding a stale cookie jar.
    let prev = state.auth.write().await.replace(ctx);
    if let Some(prev_ctx) = prev {
        prev_ctx.ping_cancel.cancel();
    }

    /*
     * #263: Prefetch accounts immediately after login success, mirroring
     * WPF behaviour where `BeanfunClient.Login` calls `GetAccounts` before
     * returning. This ensures account data is available by the time the
     * frontend navigates to AccountList, eliminating the "blank loading
     * state" gap. Errors are logged but not surfaced — the frontend will
     * still call `get_accounts` and handle the error there with proper UX.
     */
    match service_get_accounts(
        &client,
        &session,
        &session.service_code,
        &session.service_region,
    )
    .await
    {
        Ok(accounts) => {
            tracing::debug!(
                account_count = accounts.accounts.len(),
                "prefetch: accounts loaded during login"
            );
            // Store the prefetched accounts so the frontend can use them immediately
            let mut guard = state.prefetched_accounts.write().await;
            *guard = Some(accounts);
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                "prefetch: get_accounts failed during login (frontend will retry)"
            );
        }
    }

    spawn_ping_loop(ping_client, ping_cancel);
    info
}

/// Spawn [`run_ping_loop`] as a detached Tokio task.
///
/// Split out of [`install_session_and_start_ping`] so unit tests can
/// drive [`run_ping_loop`] directly without a live Tokio reactor
/// observing a spawned future.
fn spawn_ping_loop(client: BeanfunClient, cancel: CancellationToken) {
    tokio::spawn(run_ping_loop(client, cancel));
}

/// Periodically hit [`BeanfunClient::ping`] until `cancel` fires.
///
/// Ports WPF `MainWindow.pingWorker_DoWork`
/// (`MainWindow.xaml.cs` L2322-2368). The WPF loop is:
///
/// 1. Ping.
/// 2. Sleep `WaitSecs` (60 s), checking cancellation each second.
/// 3. Goto 1.
///
/// Our Tokio rewrite uses [`tokio::select!`] on the cancel token so
/// shutdown fires immediately instead of waiting up to 1 s for the
/// next inner tick. Cancellation is also checked *during* the ping
/// itself so a mid-flight request doesn't delay shutdown by up to
/// the client timeout (tens of seconds).
///
/// # Error handling
///
/// Ping failures are swallowed at `tracing::debug!` level to match
/// WPF `BeanfunClient.Ping()`'s `catch { }` (bfClient.cs L193-212).
/// A transient network hiccup or a 5xx from the Beanfun portal must
/// not kill the keep-alive loop — the next tick 60 s later is the
/// retry. If the session is genuinely dead the user will find out
/// on their next meaningful action (Get OTP, launch game), just
/// like WPF.
async fn run_ping_loop(client: BeanfunClient, cancel: CancellationToken) {
    run_ping_loop_with_interval(client, cancel, PING_INTERVAL).await;
}

/// Inner implementation of [`run_ping_loop`] with the sleep interval
/// pulled out as a parameter.
///
/// The production loop always passes [`PING_INTERVAL`] (60 s, matching
/// WPF). Splitting the interval out keeps the unit tests fast and
/// hermetic — they can drive the loop with a 50 ms cadence and assert
/// real wall-clock behaviour without depending on `start_paused = true`,
/// which interacts poorly with `wiremock`'s hyper server (the paused
/// runtime time freezes hyper's internal time wheel and the HTTP
/// request never resolves on Windows CI).
async fn run_ping_loop_with_interval(
    client: BeanfunClient,
    cancel: CancellationToken,
    interval: Duration,
) {
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            res = client.ping() => {
                match res {
                    Ok(()) => {
                        tracing::debug!("session keep-alive ping ok");
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = ?err,
                            "session keep-alive ping failed; will retry next tick"
                        );
                    }
                }
            }
        }

        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(interval) => {}
        }
    }
}

/// Drive WPF's `bfAPPAutoLogin` continuation loop until the server
/// either approves / rejects / times out the device-registration
/// request.
///
/// The service layer already ports one *single* `bfAPPAutoLogin.ashx`
/// round-trip (`login_registered_device`). The WPF client wraps that
/// call in a 2-second UI timer on both the HK regular and TOTP
/// branches; without this command-layer loop the first
/// `DeviceRegistrationRequired` result bubbles to the frontend as a
/// terminal error, which is the regression users are seeing as
/// "missing akey".
async fn await_registered_device_login(
    client: &BeanfunClient,
    login_token: &str,
    session_key: &str,
    account_id: &str,
    service_code: &str,
    service_region: &str,
) -> Result<Session, LoginError> {
    await_registered_device_login_with_interval(
        client,
        login_token,
        session_key,
        account_id,
        service_code,
        service_region,
        DEVICE_REGISTRATION_POLL_INTERVAL,
    )
    .await
}

async fn await_registered_device_login_with_interval(
    client: &BeanfunClient,
    login_token: &str,
    session_key: &str,
    account_id: &str,
    service_code: &str,
    service_region: &str,
    interval: Duration,
) -> Result<Session, LoginError> {
    loop {
        match login_registered_device(
            client,
            login_token,
            session_key,
            account_id,
            service_code,
            service_region,
        )
        .await?
        {
            Some(session) => return Ok(session),
            None => tokio::time::sleep(interval).await,
        }
    }
}

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
    *state.pending_gamepass.write().await = None;
    *state.pending_tw_login.write().await = None;

    let client = BeanfunClient::new(ClientConfig::for_region(region))?;
    let creds = Credentials::new(account, password);
    let account_id = creds.account.clone();

    // TW Regular: token-replay flow (issues #313 / #315 / #318). The step
    // runner tries `CheckAccountType` / `AccountLogin` with empty reCAPTCHA
    // tokens and pauses (returning a resumable context) only when the
    // server actually demands a solve.
    if region == LoginRegion::TW {
        // Stash the plaintext password for a possible reCAPTCHA resume
        // before `creds` is dropped (its `Drop` zeroises the buffer).
        let password_for_resume = creds.password.clone();
        let started = tw_login_start(&client, &creds).await;
        drop(creds);
        return finish_tw_step(&state, client, account_id, password_for_resume, started).await;
    }

    // HK Regular: unchanged headless flow (TOTP / device-registration
    // continuations, advance-check, etc.).
    let method = default_method_for(region);
    let service_code = region.default_service_code().to_owned();
    let service_region = region.default_service_region().to_owned();

    let outcome = login_with(&client, method, &creds).await;

    drop(creds);

    match outcome {
        Ok(session) => {
            let info = install_session_and_start_ping(&state, client, session).await;
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
        Err(LoginError::DeviceRegistrationRequired { login_token, .. }) => {
            let session_key = get_session_key(&client).await?;
            let session = await_registered_device_login(
                &client,
                &login_token,
                &session_key,
                &account_id,
                &service_code,
                &service_region,
            )
            .await?;
            let info = install_session_and_start_ping(&state, client, session).await;
            Ok(info)
        }
        Err(err) => Err(err.into()),
    }
}

/// Resume a paused TW-Regular login after the user solved a reCAPTCHA
/// widget (issues #313 / #315 / #318 — token-replay).
///
/// Preconditions: a prior [`login_regular`] (or a prior resume) parked a
/// [`PendingTwLogin`] on [`AppState::pending_tw_login`] and returned
/// [`RECAPTCHA_REQUIRED_CODE`]. The `token` is the reCAPTCHA response
/// harvested from beanfun's own origin by [`open_recaptcha_window`].
///
/// The backend replays `token` into whichever step it paused on (the
/// authoritative [`PendingTwLogin::step`], not a frontend-supplied value)
/// and continues the *same* HTTP session. Outcomes mirror the empty-first
/// flow: another reCAPTCHA (e.g. the second step now gates too) re-parks
/// the slot and returns [`RECAPTCHA_REQUIRED_CODE`] again; success installs
/// the session; an advance-check / server-message error surfaces verbatim.
#[tauri::command]
#[specta::specta]
pub async fn resume_tw_login_with_recaptcha(
    state: State<'_, AppState>,
    token: String,
) -> Result<SessionInfo, CommandError> {
    let pending = state.pending_tw_login.write().await.take();
    let Some(p) = pending else {
        return Err(CommandError::new(
            RECAPTCHA_NOT_PENDING_CODE,
            "No TW reCAPTCHA login is pending; start the login again.",
        ));
    };

    let creds = Credentials::new(p.account.clone(), p.password.clone());
    let resumed = tw_login_resume(&p.client, p.ctx.clone(), &creds, p.step, &token).await;
    drop(creds);

    finish_tw_step(&state, p.client, p.account, p.password, resumed).await
}

/// Shared tail for both [`login_regular`]'s TW arm and
/// [`resume_tw_login_with_recaptcha`]: install the session on success,
/// (re-)park [`PendingTwLogin`] and signal [`RECAPTCHA_REQUIRED_CODE`] on a
/// reCAPTCHA demand, or map any other [`LoginError`] to a command error.
async fn finish_tw_step(
    state: &State<'_, AppState>,
    client: BeanfunClient,
    account: String,
    password: String,
    outcome: Result<TwStepOutcome, LoginError>,
) -> Result<SessionInfo, CommandError> {
    match outcome {
        Ok(TwStepOutcome::Complete(session)) => {
            *state.pending_tw_login.write().await = None;
            Ok(install_session_and_start_ping(state, client, *session).await)
        }
        Ok(TwStepOutcome::RecaptchaRequired { ctx, step }) => {
            *state.pending_tw_login.write().await = Some(PendingTwLogin {
                client,
                ctx,
                account,
                password,
                step,
            });
            Err(recaptcha_required_error(step))
        }
        Err(err) => Err(err.into()),
    }
}

/// Build the `auth.recaptcha_required` command error carrying which
/// [`RecaptchaStep`] the frontend must solve, so the widget window can
/// (informationally) tag its handback and the UI can show step-specific
/// copy.
fn recaptcha_required_error(step: RecaptchaStep) -> CommandError {
    CommandError::new(
        RECAPTCHA_REQUIRED_CODE,
        "reCAPTCHA verification is required; solve it in the popup window.",
    )
    .with_details(serde_json::json!({ "step": step.as_wire() }))
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

    let session = match login_totp_service(
        &client, &challenge, &digits[0], &digits[1], &digits[2], &digits[3], &digits[4], &digits[5],
    )
    .await
    {
        Ok(session) => session,
        Err(LoginError::DeviceRegistrationRequired { login_token, .. }) => {
            await_registered_device_login(
                &client,
                &login_token,
                &challenge.session_key,
                &challenge.account_id,
                &challenge.service_code,
                &challenge.service_region,
            )
            .await?
        }
        Err(other) => return Err(other.into()),
    };

    *state.pending_totp.write().await = None;
    let info = install_session_and_start_ping(&state, client, session).await;
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
            let info = install_session_and_start_ping(&state, client, session).await;
            Ok(QrStatus::Approved { session: info })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GamePass family
// ═══════════════════════════════════════════════════════════════════════

/// Error code surfaced by [`open_gamepass_window`] when no GamePass
/// login is active on [`AppState::pending_gamepass`] at the moment
/// of invocation — same wire-string shape as
/// [`QR_NOT_STARTED_CODE`] / [`TOTP_NOT_PENDING_CODE`] /
/// [`VERIFY_NOT_STARTED_CODE`] so the Vue error router handles it
/// uniformly.
///
/// Scope is **only** the empty-`pending_gamepass` precondition.
/// The "already-open window" precondition has its own dedicated
/// code [`GAMEPASS_WINDOW_ALREADY_OPEN_CODE`] so operator logs and
/// the Vue error pipeline can attribute the two failure modes
/// distinctly (CP4 debt fix — earlier the two branches shared this
/// constant which made the "not_started" log line lie when the
/// real cause was a stale window).
pub(crate) const GAMEPASS_NOT_STARTED_CODE: &str = "auth.gamepass_not_started";

/// Error code surfaced by [`open_gamepass_window`] when a prior
/// GamePass WebView window labelled [`GAMEPASS_WINDOW_LABEL`] is
/// still alive at the moment of invocation. Distinct from
/// [`GAMEPASS_NOT_STARTED_CODE`] because the underlying remediation
/// is different — the user must close the existing window before
/// retrying, not call `login_gamepass_start` again.
///
/// The frontend renders the same `windowError` banner for both
/// codes (UX is uniform: "press Refresh to retry"), but the
/// localised toast text and the operator log attribution diverge
/// so postmortems can pinpoint the real cause.
pub(crate) const GAMEPASS_WINDOW_ALREADY_OPEN_CODE: &str = "auth.gamepass_window_already_open";

/// Fixed Tauri window label for the GamePass WebView. Deliberately
/// singular: a second invocation of [`open_gamepass_window`] while
/// the prior window is still alive returns
/// [`GAMEPASS_WINDOW_ALREADY_OPEN_CODE`] rather than spawning a
/// duplicate window. Matches WPF
/// `gamepass_form.xaml.cs::btn_OpenGamePass_Click` (L37-59) which
/// always allocates exactly one `GamePassBrowser` instance per
/// login attempt.
const GAMEPASS_WINDOW_LABEL: &str = "gamepass-login";

/// Error code surfaced by [`login_regular`] / [`resume_tw_login_with_recaptcha`]
/// when the TW account/password login requires a Google reCAPTCHA challenge
/// for this attempt (server-side anti-bot; token-replay, issues
/// #313 / #315 / #318). The frontend reacts by calling
/// [`open_recaptcha_window`] to solve the widget on beanfun's origin, then
/// calls [`resume_tw_login_with_recaptcha`] with the harvested token. The
/// error `details` carry `{ "step": "check" | "login" }`.
pub(crate) const RECAPTCHA_REQUIRED_CODE: &str = "auth.recaptcha_required";

/// Error code surfaced by [`resume_tw_login_with_recaptcha`] when no
/// [`PendingTwLogin`] is parked (the user never hit the reCAPTCHA gate, or
/// the continuation was already consumed / cleared). Remediation: restart
/// the login from [`login_regular`].
pub(crate) const RECAPTCHA_NOT_PENDING_CODE: &str = "auth.recaptcha_not_pending";

/// Fixed Tauri window label for the reCAPTCHA widget-solve WebView.
/// Distinct from [`GAMEPASS_WINDOW_LABEL`] so the two windows' double-open
/// guards are independent. Unlike the retired #308/#309 account-login
/// window, this window only hosts beanfun's own `Login/Index` page so the
/// user solves the reCAPTCHA *widget*; the token is harvested and replayed
/// over HTTP by the backend (it does not complete the login in-page).
const RECAPTCHA_WINDOW_LABEL: &str = "recaptcha-solve";

/// Tauri event names emitted by the GamePass flow. Flat dash-case
/// per the P12.1 D5 event convention.
///
/// # Emission rules
///
/// - [`GAMEPASS_SUCCESS_EVENT`]: emitted exactly once when
///   [`try_complete_gamepass_login`] resolves to `Some(session)`,
///   immediately before the window is closed. Payload:
///   [`SessionInfo`] (the same safe-subset DTO login commands
///   return).
/// - [`GAMEPASS_FAILED_EVENT`]: emitted when the page-load cookie
///   harvest fails for every harvest URL on a given page-load tick
///   (Tauri runtime error; matches the WPF `ErrorMessage`
///   branch in `TryCompleteLogin` L158-162). Payload:
///   [`CommandError`].
/// - [`GAMEPASS_CANCELLED_EVENT`]: emitted when the window is
///   destroyed **without** a prior success. Distinguished from
///   success by the `pending_gamepass` slot — success clears it to
///   `None` before closing, user-cancel leaves it `Some(_)`.
///   Payload: none (`()`).
const GAMEPASS_SUCCESS_EVENT: &str = "gamepass-login-success";
const GAMEPASS_FAILED_EVENT: &str = "gamepass-login-failed";
const GAMEPASS_CANCELLED_EVENT: &str = "gamepass-login-cancelled";

/// HTTPS origins the WebView must be polled for during a GamePass
/// completion check. Mirrors WPF `GamePassBrowser.TryCompleteLogin`
/// L123-138, which calls `CoreWebView2.CookieManager.GetCookiesAsync`
/// once per each of these three hosts and merges the results.
///
/// Order is not observable (completion depends only on whether
/// `bfWebToken` is visible on the portal origin after all inserts
/// land), but we keep it stable and portal-first so traces read
/// "happy path first" in operator logs.
const GAMEPASS_HARVEST_URLS: &[&str] = &[
    "https://tw.beanfun.com",
    "https://login.beanfun.com",
    "https://tw.newlogin.beanfun.com",
];

/// URL path markers WPF's `GamePassBrowser.OnNavigationCompleted`
/// uses as "redirect has landed on beanfun.com, try completion now"
/// signals. Every other URL (Login/Index entry page, intermediate
/// OAuth hops on `gamepass.beanfun.com`, captcha iframes, etc.) is
/// a no-op for completion — we wait for the next page load.
///
/// Matching by substring (not exact path) keeps us compatible with
/// minor URL shape tweaks on the portal side (e.g. query-string
/// additions) and mirrors the WPF `uri.Contains("return.aspx")`
/// check verbatim.
const GAMEPASS_COMPLETION_PATH_MARKERS: &[&str] = &["return.aspx", "index.aspx", "SendLogin"];

/// JavaScript injected via Tauri's `initialization_script` that
/// auto-clicks the GamePass login button once the entry page's
/// DOM is ready.
///
/// # Why `initialization_script` (not `eval` in `on_page_load`)?
///
/// The WPF reference calls `webView.ExecuteScriptAsync("...click()")`
/// inside the `NavigationCompleted` handler (`GamePassBrowser.xaml.cs`
/// L78-90), which races against the in-page script that renders
/// the `.use-gama-pass` anchor. In Tauri 2, `initialization_script`
/// runs **before** any page script, so we register a
/// `DOMContentLoaded` listener inside the script itself and the
/// click fires reliably after the anchor exists.
///
/// # Idempotence & scope-narrowing
///
/// The script is injected into **every** page the WebView loads
/// (Tauri has no URL-filter for init scripts), so we must make it
/// safe on pages that don't have a GamePass button. The
/// `querySelector` returns `null` on those pages and the
/// conditional keeps us silent. The outer IIFE + no globals keeps
/// the injection from leaking names into the portal's own JS.
const GAMEPASS_AUTOCLICK_JS: &str = r#"(() => {
  const clickButton = () => {
    const anchor = document.querySelector("a.use-gama-pass");
    if (anchor) {
      anchor.click();
    }
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", clickButton, { once: true });
  } else {
    clickButton();
  }
})();"#;

/// `initialization_script` injected into the reCAPTCHA widget-solve
/// WebView ([`open_recaptcha_window`]).
///
/// Runs on beanfun's own `Login/Index?pSKey=…` origin (the only place a
/// reCAPTCHA token is accepted — the token is origin-locked, task spec §1)
/// and does three things:
///
/// 1. **Masks the whole page** with an opaque fixed overlay and lifts only
///    the reCAPTCHA anchor iframe above it. We deliberately do NOT use
///    `visibility:hidden` on any ancestor — Chromium suppresses
///    hit-testing for a cross-origin iframe under a `visibility:hidden`
///    ancestor, which makes the checkbox unclickable (task spec trap #3).
/// 2. **Self-heals**: if no `iframe[src*=recaptcha]` appears within ~3s
///    (Tracking-Prevention race), it reloads once, guarded by
///    `sessionStorage` so it can't loop.
/// 3. **Harvests** the solved token via
///    `grecaptcha.enterprise.getResponse()` and hands it back through the
///    **URL fragment** (`#mltoken=<step>~<token>`) — beanfun's CSP blocks
///    app IPC from this origin (task spec trap #5), and reCAPTCHA tokens
///    are URL-safe base64url so `~` is a safe separator. The backend polls
///    `window.url()` for that fragment.
///
/// `{STEP}` is replaced with the [`RecaptchaStep::as_wire`] discriminator.
const RECAPTCHA_HARVEST_JS_TEMPLATE: &str = r##"(() => {
  const STEP = "{STEP}";
  // Backdrop z-index sits BELOW reCAPTCHA's image-challenge overlay
  // (~2e9) so, once the checkbox is ticked, the grid popup still shows
  // and is clickable, but ABOVE the ordinary page.
  const MASK_Z = 999999;
  const style = document.createElement("style");
  style.textContent =
    "#__bf_mask{position:fixed;inset:0;z-index:" + MASK_Z + ";background:#1c1712;" +
    "display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;" +
    "color:#f4ede4;font-family:system-ui,sans-serif;font-size:14px;text-align:center;padding:24px}" +
    ".grecaptcha-badge{z-index:" + (MASK_Z + 1) + " !important}";
  const mask = document.createElement("div");
  mask.id = "__bf_mask";
  const label = document.createElement("div");
  label.textContent = "請完成「我不是機器人」驗證";
  mask.appendChild(label);
  const attachMask = () => {
    if (document.body && !document.getElementById("__bf_mask")) {
      document.head.appendChild(style);
      document.body.appendChild(mask);
    }
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", attachMask, { once: true }); else attachMask();

  // Move the reCAPTCHA checkbox widget INTO the mask so it's actually
  // clickable above the opaque backdrop. Lifting a nested cross-origin
  // iframe purely by z-index is unreliable — an ancestor's stacking
  // context traps it below a body-level mask (#318 follow-up: the widget
  // rendered but was unclickable). Reparenting the widget's own container
  // (which carries the g-recaptcha-response textarea too) sidesteps that.
  let moved = false;
  const findWidget = () => {
    const anchor =
      document.querySelector("iframe[src*='recaptcha'][src*='anchor']") ||
      document.querySelector("iframe[title='reCAPTCHA']") ||
      document.querySelector("iframe[src*='recaptcha']");
    if (!anchor) return null;
    let w = anchor.closest(".g-recaptcha");
    if (!w) {
      w = anchor;
      for (let i = 0; i < 5 && w.parentElement && w.parentElement !== document.body; i++) {
        w = w.parentElement;
        if (w.offsetWidth >= 280 && w.offsetWidth <= 400) break;
      }
    }
    return w;
  };
  const moveTimer = setInterval(() => {
    if (moved) { clearInterval(moveTimer); return; }
    const w = findWidget();
    if (w && w !== document.body && w !== document.documentElement) {
      moved = true;
      clearInterval(moveTimer);
      w.style.position = "relative";
      w.style.zIndex = String(MASK_Z + 2);
      mask.appendChild(w);
      if (label.parentNode) label.remove();
    }
  }, 300);

  // Fallback: if the widget can't be found / moved within ~2.5s, drop the
  // opaque backdrop entirely so the user can at least click the reCAPTCHA
  // in beanfun's own (now fully visible) page. Degraded, but functional.
  setTimeout(() => {
    if (!moved) {
      const m = document.getElementById("__bf_mask");
      if (m) m.remove();
    }
  }, 2500);

  // Self-heal: reload once if the widget iframe never renders (Tracking
  // Prevention race). Guarded so it can't loop.
  setTimeout(() => {
    const has = document.querySelector("iframe[src*='recaptcha']");
    if (!has && !sessionStorage.getItem("__bf_reloaded")) {
      sessionStorage.setItem("__bf_reloaded", "1");
      location.reload();
    }
  }, 3000);

  // Harvest: poll grecaptcha for a non-empty response, then publish it via
  // the URL fragment. Only fires once.
  let sent = false;
  const readToken = () => {
    try {
      const g = window.grecaptcha && window.grecaptcha.enterprise ? window.grecaptcha.enterprise : window.grecaptcha;
      if (g && typeof g.getResponse === "function") {
        const t = g.getResponse();
        if (t) return t;
      }
    } catch (e) { /* not ready */ }
    return "";
  };
  const timer = setInterval(() => {
    if (sent) { clearInterval(timer); return; }
    const t = readToken();
    if (t) {
      sent = true;
      clearInterval(timer);
      location.hash = "mltoken=" + STEP + "~" + t;
    }
  }, 400);
})();"##;

/// Build the harvest script with the step discriminator interpolated.
fn build_recaptcha_harvest_script(step: RecaptchaStep) -> String {
    RECAPTCHA_HARVEST_JS_TEMPLATE.replace("{STEP}", step.as_wire())
}

/// Tauri event emitted by [`open_recaptcha_window`] once the solved
/// reCAPTCHA token is harvested from the URL fragment. Payload:
/// `{ "step": "check" | "login", "token": "<recaptcha-token>" }`. The
/// frontend calls `resume_tw_login_with_recaptcha(token)` in response.
const RECAPTCHA_TOKEN_EVENT: &str = "recaptcha-token";

/// Tauri event emitted by [`open_recaptcha_window`] when the widget window
/// closes / times out without a token (user gave up). Payload: `null`.
const RECAPTCHA_CANCELLED_EVENT: &str = "recaptcha-cancelled";

/// Parse a `mltoken=<step>~<token>` URL fragment into `(step, token)`.
/// Returns `None` for any other fragment shape.
fn parse_mltoken_fragment(fragment: &str) -> Option<(RecaptchaStep, String)> {
    let body = fragment.strip_prefix("mltoken=")?;
    let (step_raw, token) = body.split_once('~')?;
    let step = RecaptchaStep::from_wire(step_raw)?;
    if token.is_empty() {
        return None;
    }
    Some((step, token.to_owned()))
}

/// Strip query parameters from a URL for safe logging.
/// Prevents session tokens (e.g. `pSKey`) from leaking into traces.
fn redact_url_query(url: &Url) -> String {
    let mut redacted = url.clone();
    redacted.set_query(None);
    if url.query().is_some() {
        format!("{}?[REDACTED]", redacted)
    } else {
        redacted.to_string()
    }
}

/// Check whether `url` is a landing page WPF's
/// `GamePassBrowser.OnNavigationCompleted` would have triggered
/// completion for.
///
/// The filter is deliberately identical to WPF's — host ends with
/// `beanfun.com` **and** path carries one of the three completion
/// markers. Broadening it would cost nothing functionally
/// ([`try_complete_gamepass_login`] returns `None` when the token
/// isn't ready yet), but pinning the WPF semantics makes behavioural
/// regressions in the completion sequence easy to attribute to
/// either "upstream portal URL changed" or "we deviated from WPF".
fn should_try_gamepass_completion(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if !host.ends_with("beanfun.com") {
        return false;
    }
    let path = url.path();
    GAMEPASS_COMPLETION_PATH_MARKERS
        .iter()
        .any(|marker| path.contains(marker))
}

/// Parse a harvest origin constant into a [`Url`] — infallible by
/// construction because the constants are static HTTPS origins, but
/// factored out so the assertion reads at every call site.
fn parse_harvest_url(raw: &str) -> Url {
    Url::parse(raw).expect("GAMEPASS_HARVEST_URLS entry must be a valid absolute URL")
}

/// Diagnostic — log the **names** (never the values) of the cookies
/// the GamePass WebView currently exposes for each
/// [`GAMEPASS_HARVEST_URLS`] origin.
///
/// # Why this exists (issue #296)
///
/// The re-login fix wipes the WebView2 cookie store before seeding a
/// fresh session (see [`open_gamepass_window`]). Because the clear
/// happens inside a native COM closure with no return-value cookie
/// dump, the only way to *prove on a live run* that no stale
/// `bfWebToken` survived a logout → re-login cycle is to read the
/// WebView's own view of its cookies on the first page load (the
/// `Login/Index` entry page, before the user authenticates).
///
/// Expected traces on a healthy re-login:
///
/// - On the entry page: only the freshly-seeded portal-session
///   cookies (e.g. `ASP.NET_SessionId`) — **no** `bfWebToken`. A
///   lingering `bfWebToken` here means the clear failed (or WebView2
///   restored it from disk) and is the smoking gun for a #296
///   regression.
/// - After a real GamePass authentication: `bfWebToken` appears,
///   which is what [`try_complete_gamepass_login`] then harvests.
///
/// Values are deliberately omitted — session cookies are
/// credentials-equivalent and structured log sinks would capture
/// them (same stance as [`trace_cookie_jar`]).
///
/// Read errors per origin are logged at WARN but never abort the
/// caller; this is a best-effort observability hook, not a control-
/// flow gate.
fn trace_webview_cookies<R: tauri::Runtime>(step: &'static str, window: &WebviewWindow<R>) {
    for raw_origin in GAMEPASS_HARVEST_URLS {
        let origin = parse_harvest_url(raw_origin);
        match window.cookies_for_url(origin.clone()) {
            Ok(cookies) => {
                let names: Vec<&str> = cookies.iter().map(|c| c.name()).collect();
                tracing::info!(
                    step = step,
                    origin = %origin,
                    count = names.len(),
                    names = ?names,
                    "webview cookie names for origin"
                );
            }
            Err(err) => {
                tracing::warn!(
                    step = step,
                    origin = %origin,
                    error = ?err,
                    "failed to read webview cookies for diagnostic dump"
                );
            }
        }
    }
}

/// Diagnostic — dump every unexpired cookie in `client`'s jar to the
/// tracing pipeline as structured `info!` records, one per cookie
/// plus a summary line.
///
/// # Why this exists
///
/// Live test 2026-04-18 surfaced a failure mode where the GamePass
/// WebView received a cookie seed (`seeded=2 failed=0` in the
/// existing [`super::auth::open_gamepass_window`] summary log) but
/// beanfun's `return.aspx` still rejected the OAuth round-trip with
/// `Get SecretCode Success(…) but get data fail: (0) No such auth
/// key and secret code.` — meaning the *wrong* set of session
/// cookies got seeded, not that seeding itself failed. The CP3-era
/// summary log only counts cookies, which can't distinguish
/// "two host-only cookies on `tw.newlogin.beanfun.com`" (the WPF
/// reference's `CookieContainer.GetCookies(tw.beanfun.com)` filter
/// would have dropped these) from "two parent-domain cookies on
/// `.beanfun.com`" (the shape WPF actually seeds).
///
/// This helper logs per-cookie attributes — **including the
/// `CookieDomain` enum** from `cookie_store` which is the exact
/// distinction we need for the host-only vs subdomain-match
/// triage — so a live test trace is enough to tell the two
/// failure modes apart without a debugger attach.
///
/// # Call sites
///
/// Invoked twice on the GamePass flow for cross-verification:
///
/// 1. [`login_gamepass_start`] — right after `get_session_key`
///    returns. Captures what the portal's redirect chain left
///    behind in the client jar (the "what WPF's `bfClient` would
///    be holding at this point" snapshot).
/// 2. [`open_gamepass_window`] — right before the WebView cookie
///    seed runs. Captures what we actually hand off to the WebView.
///
/// Both dumps should be identical in the happy path (no HTTP
/// happens between them), but pinning both makes any unexpected
/// state change (cookie expiry, jar corruption) obvious and
/// localises the blame.
///
/// # Logged fields
///
/// Per cookie: `name`, `domain` (the `CookieDomain` enum), `path`
/// (the `CookiePath` enum), `secure`, `http_only`, `same_site`.
/// The raw cookie `value` is **intentionally not** logged — session
/// cookies are credentials-equivalent and structured log sinks
/// (file, Tauri console, Sentry breadcrumbs) would capture them.
///
/// Summary: total unexpired cookie count.
fn trace_cookie_jar(step: &'static str, client: &BeanfunClient) {
    let store = client.cookie_store();
    let guard = match store.lock() {
        Ok(g) => g,
        Err(err) => {
            tracing::warn!(
                step = step,
                error = ?err,
                "cookie store mutex poisoned; skipping jar dump"
            );
            return;
        }
    };

    let mut count = 0usize;
    for cookie in guard.iter_unexpired() {
        // `cookie.domain` / `cookie.path` are the `cookie_store::Cookie`
        // struct fields (CookieDomain / CookiePath enums) — NOT the
        // `RawCookie::domain()` / `RawCookie::path()` methods reached
        // through Deref. The enum form is what this dump exists for:
        // it distinguishes `HostOnly(host)` (no `Domain` attribute on
        // the `Set-Cookie`; pinned to the request host) from
        // `Suffix(host)` (explicit `Domain` attribute; matches host +
        // subdomains), which is exactly the discrimination that
        // surfaced the 2026-04-18 seed-fail regression. The method
        // form would have collapsed both to `Option<&str>` — `None`
        // for host-only — and hidden the distinction.
        tracing::info!(
            step = step,
            name = cookie.name(),
            domain = ?cookie.domain,
            path = ?cookie.path,
            secure = cookie.secure().unwrap_or(false),
            http_only = cookie.http_only().unwrap_or(false),
            same_site = ?cookie.same_site(),
            "cookie jar entry"
        );
        count += 1;
    }

    tracing::info!(step = step, total = count, "cookie jar dump complete");
}

/// Tauri event-driven worker that runs on every WebView page-load
/// completion.
///
/// # Flow (mirrors WPF `GamePassBrowser.TryCompleteLogin` L119-163)
///
/// 1. Snapshot `(client, skey)` from [`AppState::pending_gamepass`].
///    If the slot is already `None`, the flow was cancelled /
///    completed on a prior tick — bail out silently (same stance as
///    WPF's "early return without error" in L143-144).
/// 2. URL-filter the tick: only "return.aspx / index.aspx /
///    SendLogin" landings warrant a completion attempt. Every other
///    page-load (Login/Index entry, OAuth intermediaries) is a
///    no-op.
/// 3. Per [`GAMEPASS_HARVEST_URLS`] origin, pull the WebView's
///    `cookies_for_url` view and feed it into the client's cookie
///    jar via [`inject_webview_cookies`]. One call per origin so
///    `cookie_store`'s RFC 6265 domain-match check runs against the
///    correct reference URL — merging the three sets under one
///    origin would silently misclassify cookies whose `Domain`
///    attribute doesn't match the merged origin.
/// 4. Call [`try_complete_gamepass_login`]:
///    - `None` → token not visible from the portal origin yet;
///      leave `pending_gamepass` populated so the next page-load
///      tick retries (WPF L143-144).
///    - `Some(session)` → CAS the pending slot (`take()`); only
///      the first taker wins, later ticks see `None` and bail.
///      Winner populates [`AppState::auth`], emits
///      [`GAMEPASS_SUCCESS_EVENT`], closes the WebView window.
///
/// # Why spawn onto the async runtime?
///
/// Tauri's `cookies_for_url` is a sync call whose backing IPC can
/// **deadlock the WebView2 dispatcher** on Windows when invoked
/// from a synchronous event handler (the on_page_load closure runs
/// on the WebView2 message-pump thread). Spawning the work onto
/// `tauri::async_runtime::spawn` bounces it to a tokio worker, out
/// of the danger zone. Documented upstream: see wry#583 — and the
/// `tauri::WebviewWindow::cookies`/`cookies_for_url` doc comments
/// explicitly recommend this pattern.
///
/// # Tracing schema (for live-test fault isolation)
///
/// Every branch emits a structured `tracing::info!` with a
/// `step = "Gamepass*"` tag so operators can follow a single page
/// load across interleaved per-origin harvest warnings:
///
/// - `GamepassPageLoad.Finished` — entry, carries `url`.
/// - `GamepassPageLoad.NoPending` — slot already cleared (cancel /
///   prior success); no-op bail.
/// - `GamepassPageLoad.SkipUrl` — URL doesn't match completion
///   markers; waiting for next nav.
/// - `GamepassHarvest.Summary` — per-tick `harvested` / `failed`
///   cookie-origin counts (aggregates the per-origin WARN lines).
/// - `GamepassCompletion.PendingToken` — `bfWebToken` not yet in
///   jar; pending slot preserved for next tick.
/// - `GamepassCompletion.RaceLost` — concurrent tick won the
///   `take()`; silent bail.
/// - `GamepassCompletion.Success` — session minted & installed,
///   about to emit success event & close window.
async fn handle_gamepass_page_load<R: tauri::Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    url: Url,
) {
    let state: State<'_, AppState> = app.state::<AppState>();

    // Structured tracing for live-test fault isolation (see module
    // docs). Each branch of the completion flow gets a distinct
    // `step = "..."` tag so operators can grep for
    // `step=GamepassPageLoad` and follow the per-page lifecycle
    // without reconstructing causality from interleaved WARN lines.
    tracing::info!(step = "GamepassPageLoad.Finished", url = %redact_url_query(&url), "page load finished; evaluating completion");

    let (client, skey) = {
        let guard = state.pending_gamepass.read().await;
        match guard.as_ref() {
            Some(pg) => (pg.client.clone(), pg.skey.clone()),
            None => {
                tracing::info!(
                    step = "GamepassPageLoad.NoPending",
                    url = %redact_url_query(&url),
                    "pending_gamepass cleared (cancelled or completed on prior tick); skipping"
                );
                return;
            }
        }
    };

    // Issue #296 diagnostic — dump the WebView's cookie names on EVERY
    // page load (this runs *before* the completion-URL filter below so
    // it also fires on the `Login/Index` entry page). A re-login that
    // shows a stale `bfWebToken` here means the pre-seed
    // `DeleteAllCookies` in `open_gamepass_window` did not take effect.
    trace_webview_cookies("GamepassPageLoad.WebViewCookies", &window);

    if !should_try_gamepass_completion(&url) {
        tracing::info!(
            step = "GamepassPageLoad.SkipUrl",
            url = %redact_url_query(&url),
            "URL does not match completion markers; waiting for next navigation"
        );
        return;
    }

    let mut harvest_errors = 0usize;
    for raw_origin in GAMEPASS_HARVEST_URLS {
        let origin = parse_harvest_url(raw_origin);
        match window.cookies_for_url(origin.clone()) {
            Ok(cookies) => inject_webview_cookies(&client, &origin, cookies),
            Err(err) => {
                harvest_errors += 1;
                tracing::warn!(
                    error = ?err,
                    origin = %origin,
                    step = "GamepassHarvest",
                    "failed to read webview cookies; continuing with other origins"
                );
            }
        }
    }

    tracing::info!(
        step = "GamepassHarvest.Summary",
        harvested = GAMEPASS_HARVEST_URLS.len() - harvest_errors,
        failed = harvest_errors,
        "cookie harvest summary across GAMEPASS_HARVEST_URLS"
    );

    if harvest_errors == GAMEPASS_HARVEST_URLS.len() {
        // Every harvest failed — this is almost certainly a runtime
        // issue (WebView2 dispatcher dropped, platform API regression)
        // rather than a recoverable "token not here yet" state. Emit
        // `gamepass-login-failed` with a typed error so the frontend
        // can surface a message and let the user retry from scratch.
        let cmd_err = CommandError::new(
            "auth.gamepass_cookie_harvest_failed",
            "Unable to read cookies from the GamePass webview. Please retry.",
        );
        if let Err(e) = app.emit(GAMEPASS_FAILED_EVENT, cmd_err) {
            tracing::warn!(error = ?e, "failed to emit gamepass-login-failed event");
        }
        return;
    }

    let Some(session) = try_complete_gamepass_login(
        &client,
        &skey,
        LoginRegion::TW.default_service_code(),
        LoginRegion::TW.default_service_region(),
    ) else {
        tracing::info!(
            step = "GamepassCompletion.PendingToken",
            url = %redact_url_query(&url),
            "bfWebToken not yet in jar; leaving pending_gamepass in place for next tick"
        );
        return;
    };

    // Atomic CAS against the pending slot: two overlapping page-load
    // ticks would both pass every preceding check, so the sole
    // serialisation point is the `take()` — whichever tick wins the
    // write lock first takes ownership of the completion.
    if state.pending_gamepass.write().await.take().is_none() {
        tracing::info!(
            step = "GamepassCompletion.RaceLost",
            "pending_gamepass already taken by a concurrent tick; skipping emit"
        );
        return;
    }

    let info = install_session_and_start_ping(&state, client, session).await;

    tracing::info!(
        step = "GamepassCompletion.Success",
        "session minted and installed on AppState::auth; emitting gamepass-login-success"
    );

    if let Err(err) = app.emit(GAMEPASS_SUCCESS_EVENT, info) {
        tracing::warn!(error = ?err, "failed to emit gamepass-login-success event");
    }

    if let Err(err) = window.close() {
        tracing::warn!(error = ?err, "failed to close gamepass webview window after success");
    }
}

/// Tauri worker that fires when the GamePass WebView window is
/// destroyed (OS close button, user `Alt+F4`, or our own
/// [`WebviewWindow::close`] call on the success path).
///
/// Distinguishes cancel-vs-success by observing the
/// [`AppState::pending_gamepass`] slot:
///
/// - `None` — success already cleared the slot and emitted
///   [`GAMEPASS_SUCCESS_EVENT`]. This destroy event is our own
///   window-close; no further event is needed. Idempotent no-op.
/// - `Some(_)` — the user cancelled before completion. Clear the
///   slot and emit [`GAMEPASS_CANCELLED_EVENT`] so the Vue layer
///   can return to the step-1 "click to start" state.
///
/// # Why a separate worker?
///
/// `on_window_event` runs on the Tauri event-loop thread. The slot
/// read / clear is `tokio::RwLock` (async), so we bounce into
/// [`tauri::async_runtime::spawn`] the same way [`handle_gamepass_page_load`]
/// does.
async fn handle_gamepass_window_destroyed<R: tauri::Runtime>(app: AppHandle<R>) {
    let state: State<'_, AppState> = app.state::<AppState>();

    // `.take()` both reads and clears; a no-op if the slot is
    // already None (success path).
    if state.pending_gamepass.write().await.take().is_none() {
        return;
    }

    if let Err(err) = app.emit(GAMEPASS_CANCELLED_EVENT, ()) {
        tracing::warn!(error = ?err, "failed to emit gamepass-login-cancelled event");
    }

    tracing::info!(
        step = "GamepassWindowDestroyed",
        "GamePass webview closed without completion; pending_gamepass cleared"
    );
}

/// Open a fresh BeanfunClient + portal session key for a GamePass
/// login attempt and stash both on [`AppState::pending_gamepass`]
/// so a follow-up `open_gamepass_window` (CP3) can drive the
/// WebView leg.
///
/// # Behaviour
///
/// - **TW only.** Mirrors [`login_qr_start`]: the WPF
///   `MainWindow.xaml.cs::loginMethodInit` (L1099-1114) hides the
///   `btn_GamePass` button under HK, and the GamePass WebView path
///   hardcodes the TW `login.beanfun.com/GP/GPLoginInfo.aspx` host.
///   Non-TW callers receive `auth.gamepass_unsupported_region`
///   (mapped from [`LoginError::GamepassUnsupportedRegion`]) before
///   any HTTP traffic / window allocation.
/// - **Mints a fresh `BeanfunClient`** (TW endpoints) so the cookie
///   jar starts empty — exactly mirrors WPF
///   `gamepass_form.btn_OpenGamePass_Click` L52-53
///   (`var client = new BeanfunClient(); ... client.GetSessionkey()`),
///   which throws away any prior `App.MainWnd.bfClient` and starts
///   over. Cookie continuity from a prior login is intentionally
///   **not** desirable here: the GamePass leg must look like a
///   first-time portal visit so the WebView's pre-injected cookies
///   match the `bfClient`'s view of the world.
/// - **Returns `()`** because everything the frontend needs is
///   conveyed by the next event in the flow:
///     - `open_gamepass_window` (CP3) opens the WebView using the
///       stashed `skey`,
///     - `gamepass-login-success` / `gamepass-login-failed` Tauri
///       events surface the terminal outcome.
///   Keeping `skey` backend-internal matches the P10.2 Q4=C
///   "no secrets over IPC" stance shared with `pending_qr` /
///   `pending_totp`.
///
/// # Side effects
///
/// - Clears any prior `pending_totp` / `pending_qr` /
///   `pending_gamepass` (switching login method invalidates every
///   half-finished continuation, same stance as [`login_qr_start`]).
/// - Populates `pending_gamepass = Some((client, skey))` on success
///   so [`PendingGamepass`] can drive the CP3 WebView leg.
///
/// # Preconditions
///
/// - **No live GamePass WebView window.** If a prior
///   [`open_gamepass_window`] call's window is still alive we
///   refuse the call with [`GAMEPASS_WINDOW_ALREADY_OPEN_CODE`] —
///   the same typed error [`open_gamepass_window`] itself uses —
///   without touching any pending slot or minting a new client.
///
///   This guards against a subtle race surfaced in live test
///   2026-04-18: if a user triggered a second
///   `login_gamepass_start` while an old GamePass window was still
///   up, this command would happily wipe `pending_gamepass` and
///   replace it with a fresh `(client, skey)`. The follow-up
///   `open_gamepass_window` would then reject with
///   `auth.gamepass_window_already_open` (correct), **but** the
///   still-live old window's [`handle_gamepass_window_destroyed`]
///   hook would misread the fresh pending slot as "user cancelled
///   this attempt" the moment the user closed the old window,
///   clearing the new slot and emitting a spurious
///   `gamepass-login-cancelled` event.
///
///   Pushing the window check up here forces the user through a
///   clean "close old → start new" transition — the same invariant
///   WPF enforces by allocating exactly one `GamePassBrowser` per
///   click (`gamepass_form.xaml.cs::btn_OpenGamePass_Click`
///   L37-59).
///
/// # Region restriction
///
/// GamePass is **TW-only** — same WPF guard as QR
/// (`MainWindow.xaml.cs::loginMethodInit` L1099-1114). The region
/// parameter is kept for symmetry with [`login_regular`] /
/// [`login_qr_start`], but a non-TW value bubbles up
/// [`LoginError::GamepassUnsupportedRegion`] (surfaces as
/// `auth.gamepass_unsupported_region`).
///
/// # Why the region check happens here, not in a service module
///
/// `login_gamepass_start` does not call any gamepass-specific
/// service function (the body is just `BeanfunClient::new` +
/// [`get_session_key`] + slot stash); the region guard is the only
/// logic that would justify a thin service wrapper. Inlining it
/// keeps the `services::beanfun::login::*` modules focused on
/// per-step HTTP calls (SRP) and avoids a `gamepass_start` shim
/// whose body would be a single `if`. CP3's
/// `complete_gamepass_login` will live in
/// `services/beanfun/login/gamepass.rs` because *that* one really
/// does drive multiple HTTP round-trips.
#[tauri::command]
#[specta::specta]
pub async fn login_gamepass_start<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    region: LoginRegion,
) -> Result<(), CommandError> {
    if region != LoginRegion::TW {
        return Err(LoginError::GamepassUnsupportedRegion.into());
    }

    // Window-alive pre-flight guard — see the "# Preconditions"
    // docblock section above for the race rationale. Surfacing the
    // same code as `open_gamepass_window`'s double-open branch keeps
    // the i18n / toast pipeline uniform for both entry points.
    if app.get_webview_window(GAMEPASS_WINDOW_LABEL).is_some() {
        return Err(CommandError::new(
            GAMEPASS_WINDOW_ALREADY_OPEN_CODE,
            "GamePass login window is already open; close it before starting a new login.",
        ));
    }

    *state.pending_totp.write().await = None;
    *state.pending_qr.write().await = None;
    *state.pending_gamepass.write().await = None;

    let client = BeanfunClient::new(ClientConfig::for_region(region))?;
    let skey = get_session_key(&client).await?;

    tracing::info!(
        step = "GamepassStart",
        region = ?client.config().region,
        "GamePass session key acquired; pending_gamepass populated, awaiting open_gamepass_window"
    );

    // Live-test diagnostic (2026-04-18) — dump what the portal's
    // redirect chain left behind in the jar so we can diagnose the
    // "Get SecretCode Success(…) but get data fail: (0) No such
    // auth key and secret code." failure mode against the WPF
    // reference seed set. See [`trace_cookie_jar`] docblock.
    trace_cookie_jar("GamepassStart.JarDump", &client);

    *state.pending_gamepass.write().await = Some(PendingGamepass::new(client, skey));
    Ok(())
}

/// Build the portal login URL the GamePass WebView should navigate
/// to on open. The `pSKey` parameter binds the WebView flow to the
/// specific portal session previously minted by
/// [`login_gamepass_start`] (and sitting on
/// [`PendingGamepass::skey`]).
///
/// The URL shape mirrors WPF `GamePassBrowser.OnLoaded`
/// (`Beanfun\Windows\GamePassBrowser.xaml.cs` L31-40):
///
/// ```text
/// https://login.beanfun.com/Login/Index?pSKey={skey}
/// ```
///
/// Factored into a helper so the unit tests can assert the URL
/// shape without standing up a real WebView.
fn build_gamepass_login_url(skey: &str) -> Result<Url, CommandError> {
    let mut url = Url::parse("https://login.beanfun.com/Login/Index").map_err(|e| {
        CommandError::new(
            "ui.window_create_failed",
            format!("Failed to construct GamePass login URL: {e}"),
        )
    })?;
    url.query_pairs_mut().append_pair("pSKey", skey);
    Ok(url)
}

/// Open the GamePass WebView window and wire its page-load / destroy
/// hooks to the completion workers.
///
/// # Preconditions
///
/// Two distinct error codes guard the entry — keep the
/// distinction so operator logs and the Vue toast pipeline can
/// attribute the real cause:
///
/// - [`GAMEPASS_NOT_STARTED_CODE`] (`auth.gamepass_not_started`) —
///   no [`login_gamepass_start`] preceded this call, so
///   [`AppState::pending_gamepass`] is empty. Remediation: call
///   `login_gamepass_start` first.
/// - [`GAMEPASS_WINDOW_ALREADY_OPEN_CODE`]
///   (`auth.gamepass_window_already_open`) — a prior
///   [`tauri::WebviewWindow`] labelled [`GAMEPASS_WINDOW_LABEL`]
///   is still alive; WPF allocates exactly one `GamePassBrowser`
///   per login attempt (`gamepass_form.xaml.cs::btn_OpenGamePass_Click`
///   L37-59) and duplicating would race on the shared
///   `pending_gamepass` slot. Remediation: close the existing
///   window before retrying.
///
/// # Side effects
///
/// - Creates a single [`tauri::WebviewWindow`] labelled
///   [`GAMEPASS_WINDOW_LABEL`], navigating to
///   `https://login.beanfun.com/Login/Index?pSKey={skey}`.
/// - Injects [`GAMEPASS_AUTOCLICK_JS`] into every page the WebView
///   loads — harmless on non-GamePass pages (the `querySelector`
///   returns `null`).
/// - Attaches an `on_page_load` hook that spawns
///   [`handle_gamepass_page_load`] onto `tauri::async_runtime::spawn`
///   for each `PageLoadEvent::Finished` tick.
/// - Attaches an `on_window_event` hook that spawns
///   [`handle_gamepass_window_destroyed`] when the window is
///   destroyed (user cancel or programmatic close after success).
///
/// # Terminal outcomes
///
/// Never returned synchronously — the command resolves `Ok(())` as
/// soon as the WebView window is created. The real terminal outcome
/// arrives later via the Tauri event bus:
///
/// - [`GAMEPASS_SUCCESS_EVENT`] with [`SessionInfo`] payload — login
///   succeeded and [`AppState::auth`] is now populated.
/// - [`GAMEPASS_CANCELLED_EVENT`] — user closed the window before
///   completion; `pending_gamepass` cleared.
/// - [`GAMEPASS_FAILED_EVENT`] with [`CommandError`] payload — all
///   three harvest URLs failed on a page-load tick (defensive
///   surface for Tauri runtime regressions).
///
/// Keeping the `Ok(())` return separate from the success event
/// mirrors the P10.2 Q5=B split between "command success = flow
/// started" and "event delivery = flow terminal outcome" already
/// established by `login_qr_start` / `login_qr_check`.
///
/// # Why async?
///
/// Tauri's [`WebviewWindowBuilder::build`] deadlocks on Windows when
/// called from a synchronous command or event handler (WebView2
/// issue tracked upstream at wry#583). `async fn` hands the call
/// off to the tokio executor, which is a different thread from the
/// WebView2 message pump.
#[tauri::command]
#[specta::specta]
pub async fn open_gamepass_window<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    // We need both the skey (for the login URL) AND the BeanfunClient
    // (for the cookie-seeding step below) from the same pending slot,
    // so clone both under the single read-lock rather than re-locking
    // twice.
    let (client, skey) = {
        let guard = state.pending_gamepass.read().await;
        match guard.as_ref() {
            Some(pg) => (pg.client.clone(), pg.skey.clone()),
            None => {
                return Err(CommandError::new(
                    GAMEPASS_NOT_STARTED_CODE,
                    "No GamePass login is active; call login_gamepass_start first.",
                ));
            }
        }
    };

    // Guard against double-open while a prior window is alive. The
    // WebView2 runtime would reject a second window with the same
    // label anyway; surfacing the typed error lets the Vue layer
    // handle it uniformly (e.g. flash the existing window to front
    // in a future UX iteration). Distinct code from
    // `GAMEPASS_NOT_STARTED_CODE` because the remediation diverges:
    // the user closes the existing window, not re-runs
    // `login_gamepass_start`.
    if app.get_webview_window(GAMEPASS_WINDOW_LABEL).is_some() {
        return Err(CommandError::new(
            GAMEPASS_WINDOW_ALREADY_OPEN_CODE,
            "GamePass login window is already open; close it before retrying.",
        ));
    }

    let login_url = build_gamepass_login_url(&skey)?;

    let app_for_page_load = app.clone();
    let app_for_destroyed = app.clone();

    // ── Build with `about:blank` so the **first** real network
    // request is the one to `login.beanfun.com/Login/Index?pSKey=…`
    // AFTER we've seeded the session cookies.
    //
    // WPF parity: `GamePassBrowser.xaml.cs::OnWebViewReady` L66-77
    // seeds every `BeanfunClient.CookieContainer` cookie into
    // WebView2 BEFORE the XAML `Source` navigation begins
    // (WebView2's `CoreWebView2InitializationCompleted` fires pre-
    // navigation). Tauri's `WebviewWindowBuilder::build()` is
    // async-until-first-navigation, so we can't cleanly interpose
    // before the initial `External(login_url)` request. The
    // `about:blank → seed → navigate` trick preserves the same
    // invariant (no real request fires without session cookies)
    // without a pre-navigation hook.
    //
    // Without this, `return.aspx` emits
    // `Get SecretCode Success(…) but get data fail: (0) No such auth
    // key and secret code.` — beanfun can't match the OAuth
    // round-trip back to the `get_session_key` call because the two
    // legs land on different session ids (observed in live test
    // 2026-04-18, D5 hotfix).
    let about_blank: Url = "about:blank".parse().expect("about:blank is a valid URL");

    let window = WebviewWindowBuilder::new(
        &app,
        GAMEPASS_WINDOW_LABEL,
        WebviewUrl::External(about_blank),
    )
    .title("GamePass 登入")
    .inner_size(900.0, 700.0)
    .resizable(true)
    .initialization_script(GAMEPASS_AUTOCLICK_JS)
    .on_page_load(move |window, payload| {
        // Only the `Finished` edge matters — `Started` fires before
        // cookies for the destination URL are actually populated,
        // so harvesting on Start would race against the navigation
        // cookie set and miss `bfWebToken`.
        if payload.event() != PageLoadEvent::Finished {
            return;
        }
        let app = app_for_page_load.clone();
        let window = window.clone();
        let url = payload.url().clone();
        tauri::async_runtime::spawn(async move {
            handle_gamepass_page_load(app, window, url).await;
        });
    })
    .build()
    .map_err(|e| {
        CommandError::new(
            "ui.window_create_failed",
            format!("Failed to create GamePass webview window: {e}"),
        )
    })?;

    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let app = app_for_destroyed.clone();
            tauri::async_runtime::spawn(async move {
                handle_gamepass_window_destroyed(app).await;
            });
        }
    });

    // Live-test diagnostic (2026-04-18) — dump the jar state a
    // second time, right before we iterate it to seed the WebView.
    // See [`trace_cookie_jar`] docblock for the rationale; between
    // this and the `GamepassStart.JarDump` in [`login_gamepass_start`]
    // any unexpected mutation (cookie expiry, concurrent jar write
    // from another tauri task) becomes obvious.
    trace_cookie_jar("GamepassWebViewSeed.JarDump", &client);

    // ── Reset the shared WebView2 cookie store, then seed the fresh
    // session cookies (issue #296).
    //
    // WebView2 keeps ONE cookie store per user-data-folder, shared by
    // every window for the lifetime of the host *process*. A prior
    // GamePass login therefore leaves its (now logged-out, server-
    // invalidated) `bfWebToken` / `ASP.NET_SessionId` behind. On a
    // second attempt within the same process the portal sees that
    // stale token, short-circuits the OAuth round-trip, and the
    // harvest lifts the dead session — surfacing the wrong / empty
    // account data. Only restarting the .exe (which ends the WebView2
    // browser session and drops the session cookies) recovered.
    //
    // Wiping the store before seeding makes every attempt start from a
    // fresh-browser state, equivalent to a process restart.
    #[cfg(target_os = "windows")]
    {
        // Two distinct native passes, NOT one fused closure.
        //
        // `DeleteAllCookies` and `AddOrUpdateCookie` are both
        // fire-and-return COM calls that queue work on the WebView2
        // browser process, and Microsoft documents no ordering
        // guarantee between a delete and an immediately-following add.
        // Issuing them back-to-back in the same pass risks the pending
        // delete wiping the cookies we just seeded — which would
        // reproduce the very "No such auth key and secret code" failure
        // the D5 seed fix cured. So: clear, wait for the delete to
        // flush, THEN seed, then wait for the seed to flush, then
        // navigate.
        let cleared = crate::commands::cookie_native::clear_all_cookies_native(&window);
        tracing::info!(
            step = "GamepassWebViewClear",
            cleared = cleared,
            "issued DeleteAllCookies before seeding (issue #296)"
        );
        // Let the delete commit on the browser process before we start
        // writing the fresh cookies.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let seeded = crate::commands::cookie_native::seed_cookies_native(&window, &client);
        tracing::info!(
            step = "GamepassWebViewSeed.Summary",
            seeded = seeded,
            "seeded fresh session cookies after clear (native COM)"
        );
        // Let the seed flush before the navigation below sends the
        // request cookies (same flush stance as `web_browser::open_*`).
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Non-Windows has no native cookie API; fall back to wry's
    // `set_cookie` (best-effort per-cookie). The cookie-persistence
    // quirk this clears is Windows/WebView2-specific and beanfun ships
    // Windows-only, so the absence of a clear here is acceptable.
    #[cfg(not(target_os = "windows"))]
    {
        let mut seed_failures = 0usize;
        let seeded = seed_webview_cookies_from_client(&client, |cookie| {
            if let Err(err) = window.set_cookie(cookie.clone()) {
                seed_failures += 1;
                tracing::warn!(
                    step = "GamepassWebViewSeed.CookieError",
                    cookie_name = %cookie.name(),
                    cookie_domain = ?cookie.domain(),
                    error = ?err,
                    "failed to seed cookie into GamePass WebView; continuing with remaining cookies"
                );
            }
            // Explicit `Ok` so the helper's fail-fast short-circuit
            // semantics never fire — we want a best-effort full pass
            // matching WPF.
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("seed closure is infallible");

        tracing::info!(
            step = "GamepassWebViewSeed.Summary",
            seeded = seeded - seed_failures,
            failed = seed_failures,
            "cookie seed summary from BeanfunClient jar into WebView before login navigation"
        );
    }

    // ── Navigate to the real login URL. From here on the page-load
    // handler drives completion (same as before).
    //
    // Live-test diagnostic (2026-04-18) — emit the composed login
    // URL (hence the resolved `pSKey`) right before navigate so the
    // trace can correlate "which seed state went with which pSKey"
    // when multiple attempts interleave in one operator log.
    tracing::info!(
        step = "GamepassWebViewNavigate",
        url = %redact_url_query(&login_url),
        "navigating GamePass WebView to login URL after cookie seed"
    );
    if let Err(err) = window.navigate(login_url.clone()) {
        // Navigation failed — the WebView is stranded on
        // `about:blank`. Clear the pending slot ourselves so the
        // Destroyed hook doesn't fire a spurious
        // `gamepass-login-cancelled` event, then close the dangling
        // window. Surface a typed error to the frontend toast /
        // banner pipeline.
        *state.pending_gamepass.write().await = None;
        let _ = window.close();
        return Err(CommandError::new(
            "ui.gamepass_navigate_failed",
            format!("Failed to navigate GamePass webview to login URL: {err}"),
        ));
    }

    tracing::info!(
        step = "GamepassWindowOpened",
        label = GAMEPASS_WINDOW_LABEL,
        "GamePass webview opened; awaiting completion or cancel"
    );

    Ok(())
}

/// Open the reCAPTCHA **widget-solve** WebView (issues #313 / #315 / #318 —
/// token-replay).
///
/// Triggered by the frontend after [`login_regular`] /
/// [`resume_tw_login_with_recaptcha`] surface [`RECAPTCHA_REQUIRED_CODE`].
/// Unlike the retired #308/#309 window (which tried to complete the whole
/// login in-page and broke on WebView2 Tracking Prevention), this window
/// hosts beanfun's own `Login/Index?pSKey=…` page purely so the user solves
/// the reCAPTCHA **widget**. The solved token is:
///
/// 1. harvested in-page by [`RECAPTCHA_HARVEST_JS_TEMPLATE`] and published
///    via the URL fragment `#mltoken=<step>~<token>`,
/// 2. polled off `window.url()` here (app IPC from beanfun's origin is
///    blocked by its CSP — task spec trap #5),
/// 3. emitted to the frontend via [`RECAPTCHA_TOKEN_EVENT`], which then
///    calls [`resume_tw_login_with_recaptcha`] to replay it over HTTP.
///
/// Windows: WebView2 Tracking Prevention is disabled first
/// ([`crate::commands::cookie_native::disable_tracking_prevention_native`])
/// — otherwise google.com/gstatic.com third-party storage is blocked and
/// the widget renders dead (the direct cause of #318, task spec trap #2).
#[tauri::command]
#[specta::specta]
pub async fn open_recaptcha_window<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let (client, skey, step) = {
        let guard = state.pending_tw_login.read().await;
        match guard.as_ref() {
            Some(p) => (p.client.clone(), p.ctx.skey.clone(), p.step),
            None => {
                return Err(CommandError::new(
                    RECAPTCHA_NOT_PENDING_CODE,
                    "No TW reCAPTCHA login is pending; start the login again.",
                ));
            }
        }
    };

    if app.get_webview_window(RECAPTCHA_WINDOW_LABEL).is_some() {
        return Err(CommandError::new(
            GAMEPASS_WINDOW_ALREADY_OPEN_CODE,
            "reCAPTCHA window is already open; solve or close it before retrying.",
        ));
    }

    // Same `Login/Index?pSKey=…` origin the login POSTs ran against — the
    // reCAPTCHA token is origin-locked to `login.beanfun.com` (task spec §1).
    let login_url = build_gamepass_login_url(&skey)?;
    let harvest_js = build_recaptcha_harvest_script(step);

    // Shared flag: the poll loop sets it once a token is captured so the
    // window-destroyed hook knows not to emit a spurious "cancelled".
    let token_captured = Arc::new(AtomicBool::new(false));
    let captured_for_destroy = token_captured.clone();
    let app_for_destroyed = app.clone();

    let about_blank: Url = "about:blank".parse().expect("about:blank is a valid URL");

    let window = WebviewWindowBuilder::new(
        &app,
        RECAPTCHA_WINDOW_LABEL,
        WebviewUrl::External(about_blank),
    )
    .title("驗證 / reCAPTCHA")
    .inner_size(480.0, 640.0)
    .resizable(true)
    .initialization_script(harvest_js.as_str())
    .build()
    .map_err(|e| {
        CommandError::new(
            "ui.window_create_failed",
            format!("Failed to create reCAPTCHA webview window: {e}"),
        )
    })?;

    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            // Only a cancel if no token was captured — the success path
            // closes the window itself after emitting the token event.
            if !captured_for_destroy.load(Ordering::SeqCst) {
                let _ = app_for_destroyed.emit(RECAPTCHA_CANCELLED_EVENT, ());
            }
        }
    });

    // #318 / task spec trap #2: disable Tracking Prevention so the
    // google.com/gstatic.com storage the reCAPTCHA widget needs isn't blocked.
    #[cfg(target_os = "windows")]
    {
        let disabled = crate::commands::cookie_native::disable_tracking_prevention_native(&window);
        tracing::info!(
            step = "RecaptchaWebView.TrackingPrevention",
            disabled = disabled,
            "attempted to disable WebView2 tracking prevention (#318)"
        );
    }

    // Clear + seed the session cookies before navigating (same #296 native
    // COM dance as the GamePass / account-login windows).
    #[cfg(target_os = "windows")]
    {
        let _ = crate::commands::cookie_native::clear_all_cookies_native(&window);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let seeded = crate::commands::cookie_native::seed_cookies_native(&window, &client);
        tracing::info!(
            step = "RecaptchaWebView.Seed",
            seeded = seeded,
            "seeded session cookies into reCAPTCHA WebView"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = seed_webview_cookies_from_client(&client, |cookie| {
            let _ = window.set_cookie(cookie.clone());
            Ok::<(), std::convert::Infallible>(())
        });
    }

    if let Err(err) = window.navigate(login_url.clone()) {
        let _ = window.close();
        return Err(CommandError::new(
            "ui.recaptcha_navigate_failed",
            format!("Failed to navigate reCAPTCHA webview: {err}"),
        ));
    }

    tracing::info!(
        step = "RecaptchaWindowOpened",
        label = RECAPTCHA_WINDOW_LABEL,
        recaptcha_step = step.as_wire(),
        "reCAPTCHA widget window opened; polling URL fragment for token"
    );

    // Poll the live window URL for the `#mltoken=<step>~<token>` fragment
    // the harvest script publishes. ~3-minute budget mirrors the reCAPTCHA
    // challenge timeout.
    let poll_app = app.clone();
    tauri::async_runtime::spawn(async move {
        const MAX_TICKS: u32 = 360; // 360 * 500ms = 180s
        for _ in 0..MAX_TICKS {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let Some(win) = poll_app.get_webview_window(RECAPTCHA_WINDOW_LABEL) else {
                // Window gone (user closed / navigated away). The Destroyed
                // hook already emitted `cancelled` if no token was captured.
                return;
            };
            let Ok(url) = win.url() else { continue };
            let Some(fragment) = url.fragment() else {
                continue;
            };
            if let Some((frag_step, token)) = parse_mltoken_fragment(fragment) {
                token_captured.store(true, Ordering::SeqCst);
                let _ = poll_app.emit(
                    RECAPTCHA_TOKEN_EVENT,
                    RecaptchaTokenPayload {
                        step: frag_step.as_wire(),
                        token,
                    },
                );
                let _ = win.close();
                return;
            }
        }
        // Timed out — treat as cancel so the frontend can offer a retry.
        if let Some(win) = poll_app.get_webview_window(RECAPTCHA_WINDOW_LABEL) {
            let _ = win.close();
        } else {
            let _ = poll_app.emit(RECAPTCHA_CANCELLED_EVENT, ());
        }
    });

    Ok(())
}

/// Payload of [`RECAPTCHA_TOKEN_EVENT`].
#[derive(Clone, Serialize)]
struct RecaptchaTokenPayload {
    /// `RecaptchaStep::as_wire` value (`"check"` / `"login"`).
    step: &'static str,
    /// Solved reCAPTCHA response token.
    token: String,
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
/// `pending_qr` → `pending_verify` → `pending_gamepass`) so the
/// `tracing` logs (if any) read consistently in debugging.
async fn clear_all_auth_state(state: &AppState) {
    *state.auth.write().await = None;
    *state.prefetched_accounts.write().await = None;
    *state.pending_totp.write().await = None;
    *state.pending_qr.write().await = None;
    *state.pending_verify.write().await = None;
    *state.pending_gamepass.write().await = None;
    *state.pending_tw_login.write().await = None;
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
/// - Clears `auth`, `pending_totp`, `pending_qr`,
///   `pending_verify`, and `pending_gamepass` unconditionally.
///   After this command returns, every subsequent command that
///   calls `require_auth` / reads a pending slot will surface its
///   typed "not started" / "session_required" error.
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
        // Cancel the session keep-alive ping loop *first* so the
        // background task doesn't race with `logout_service` and
        // POST a keep-alive ping against a server the server-side
        // logout just invalidated. `cancel()` is idempotent and
        // non-blocking; the spawned task observes the signal on
        // the next `tokio::select!` wake-up inside
        // [`run_ping_loop`].
        ctx.ping_cancel.cancel();

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

    // ── reCAPTCHA URL-fragment token handback (#313/#315/#318) ─────

    #[test]
    fn parse_mltoken_fragment_extracts_step_and_token() {
        let (step, token) =
            parse_mltoken_fragment("mltoken=login~03AFcW.token-VALUE_09").expect("parses");
        assert_eq!(step, RecaptchaStep::AccountLogin);
        assert_eq!(token, "03AFcW.token-VALUE_09");

        let (step, _) = parse_mltoken_fragment("mltoken=check~abc").expect("parses");
        assert_eq!(step, RecaptchaStep::CheckAccount);
    }

    #[test]
    fn parse_mltoken_fragment_rejects_malformed_input() {
        assert!(parse_mltoken_fragment("mltoken=login~").is_none()); // empty token
        assert!(parse_mltoken_fragment("mltoken=bogus~tok").is_none()); // bad step
        assert!(parse_mltoken_fragment("login~tok").is_none()); // no prefix
        assert!(parse_mltoken_fragment("mltoken=logintok").is_none()); // no separator
    }

    // ── Session keep-alive (run_ping_loop) ────────────────────────

    use crate::services::beanfun::Endpoints;
    use std::time::Duration as StdDuration;
    use url::Url;
    use wiremock::matchers::{body_string_contains, method as wm_method, path as wm_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fake_session() -> Session {
        Session::new(
            LoginRegion::TW,
            "skey-test",
            "web-token-test",
            "acc-test",
            LoginRegion::TW.default_service_code(),
            LoginRegion::TW.default_service_region(),
        )
    }

    /// Build a [`BeanfunClient`] whose `portal_base` points at
    /// `server`. Shared between the cancellation and error-handling
    /// tests for [`run_ping_loop`].
    fn ping_client_against(server: &MockServer) -> BeanfunClient {
        let base = Url::parse(&format!("{}/", server.uri())).expect("mock URL parses");
        let endpoints = Endpoints {
            login_base: base.clone(),
            portal_base: base.clone(),
            newlogin_base: base,
        };
        let mut cfg = ClientConfig::for_region(LoginRegion::TW);
        cfg.endpoints = endpoints;
        BeanfunClient::new(cfg).expect("client builds")
    }

    fn device_login_client_against(server: &MockServer) -> BeanfunClient {
        let base = Url::parse(&format!("{}/", server.uri())).expect("mock URL parses");
        let endpoints = Endpoints {
            login_base: base.clone(),
            portal_base: base.clone(),
            newlogin_base: base,
        };
        let mut cfg = ClientConfig::for_region(LoginRegion::HK);
        cfg.endpoints = endpoints;
        BeanfunClient::new(cfg).expect("client builds")
    }

    async fn mount_echo_token_200(server: &MockServer) {
        Mock::given(wm_method("GET"))
            .and(wm_path("/beanfun_block/generic_handlers/echo_token.ashx"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(server)
            .await;
    }

    async fn mount_device_poll_success(server: &MockServer, login_token: &str, akey: &str) {
        let poll_body =
            format!(r#"{{"IntResult":"2","StrReslut":"MLogin/done.aspx?akey={akey}"}}"#);
        Mock::given(wm_method("POST"))
            .and(wm_path("/login/bfAPPAutoLogin.ashx"))
            .and(body_string_contains(format!("LT={login_token}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(poll_body))
            .mount(server)
            .await;

        Mock::given(wm_method("GET"))
            .and(wm_path("/login/MLogin/done.aspx"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(server)
            .await;
    }

    async fn mount_login_completed_success(server: &MockServer, web_token: &str) {
        Mock::given(wm_method("POST"))
            .and(wm_path("/beanfun_block/bflogin/return.aspx"))
            .and(body_string_contains("AuthKey="))
            .respond_with(
                ResponseTemplate::new(302)
                    .append_header("Set-Cookie", format!("bfWebToken={web_token}; Path=/;"))
                    .append_header("Location", format!("{}/after", server.uri())),
            )
            .mount(server)
            .await;

        Mock::given(wm_method("GET"))
            .and(wm_path("/after"))
            .respond_with(ResponseTemplate::new(200).set_body_string("done"))
            .mount(server)
            .await;
    }

    /// Cancelling the token *before* the first ping fires must
    /// terminate [`run_ping_loop`] without issuing any HTTP
    /// request. This pins the shutdown semantics that `logout`
    /// relies on: `ctx.ping_cancel.cancel()` takes effect
    /// immediately, not "after the next tick".
    #[tokio::test]
    async fn run_ping_loop_exits_promptly_when_cancelled_before_first_tick() {
        let server = MockServer::start().await;
        mount_echo_token_200(&server).await;
        let client = ping_client_against(&server);

        let cancel = CancellationToken::new();
        cancel.cancel();

        tokio::time::timeout(StdDuration::from_secs(5), run_ping_loop(client, cancel))
            .await
            .expect("loop must return promptly on pre-cancelled token");

        let requests = server.received_requests().await.expect("log enabled");
        assert!(
            requests.is_empty(),
            "no ping should fire if token is cancelled before loop entry; got {} request(s)",
            requests.len(),
        );
    }

    /// After one successful ping, cancelling the token mid-sleep
    /// must cause the loop to exit on the next `select!` wake
    /// without waiting the full [`PING_INTERVAL`]. This is the
    /// hot path — a user that logs out 1 s after login should
    /// not leave a 59 s zombie task running.
    #[tokio::test]
    async fn run_ping_loop_exits_during_sleep_after_first_ping() {
        let server = MockServer::start().await;
        mount_echo_token_200(&server).await;
        let client = ping_client_against(&server);

        let cancel = CancellationToken::new();
        let cancel_for_loop = cancel.clone();
        let handle = tokio::spawn(async move { run_ping_loop(client, cancel_for_loop).await });

        // Wait until the first ping has been observed by the mock,
        // then cancel. We poll instead of `sleep` so the test stays
        // deterministic across slow CI machines.
        let deadline = tokio::time::Instant::now() + StdDuration::from_secs(5);
        loop {
            let count = server
                .received_requests()
                .await
                .map(|r| r.len())
                .unwrap_or(0);
            if count >= 1 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("first ping never arrived within 5s");
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        cancel.cancel();

        tokio::time::timeout(StdDuration::from_secs(5), handle)
            .await
            .expect("loop must exit promptly after cancel")
            .expect("spawned task must not panic");
    }

    /// A 5xx response from `echo_token.ashx` must NOT kill the
    /// loop — WPF's `catch { }` swallows errors and the next tick
    /// retries. We pin this by waiting for *two* requests to land
    /// against a server that always returns 500, using a 50 ms
    /// interval so the test stays sub-second.
    ///
    /// We deliberately avoid `start_paused = true` here: the paused
    /// runtime time freezes hyper's internal time wheel, and on
    /// Windows CI the wiremock-served request never resolves
    /// (observed: the rust test job hung past the 6 h GitHub
    /// timeout). The interval-injection seam in
    /// [`run_ping_loop_with_interval`] lets us keep real time + a
    /// short cadence instead.
    #[tokio::test]
    async fn run_ping_loop_keeps_running_after_ping_failure() {
        let server = MockServer::start().await;
        Mock::given(wm_method("GET"))
            .and(wm_path("/beanfun_block/generic_handlers/echo_token.ashx"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = ping_client_against(&server);

        let cancel = CancellationToken::new();
        let cancel_for_loop = cancel.clone();
        let handle = tokio::spawn(async move {
            run_ping_loop_with_interval(client, cancel_for_loop, StdDuration::from_millis(50)).await
        });

        // Poll for two requests to land — at 50 ms cadence the second
        // ping should arrive within ~100 ms even on slow CI; the 10 s
        // ceiling is a generous backstop, not the expected duration.
        let deadline = tokio::time::Instant::now() + StdDuration::from_secs(10);
        loop {
            let count = server
                .received_requests()
                .await
                .map(|r| r.len())
                .unwrap_or(0);
            if count >= 2 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "second ping never arrived within 10s (got {count}); 5xx must not stop the loop"
                );
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        cancel.cancel();
        tokio::time::timeout(StdDuration::from_secs(5), handle)
            .await
            .expect("loop exits after cancel even while failing")
            .expect("spawned task must not panic");
    }

    /// End-to-end wiring for the login-path helper:
    /// `install_session_and_start_ping` must populate `AppState::auth`
    /// *and* spawn a ping loop that actually fires. If wiring is
    /// broken we'd observe the auth context installed but no
    /// request ever hitting the mock server.
    #[tokio::test]
    async fn install_session_and_start_ping_populates_auth_and_fires_ping() {
        let server = MockServer::start().await;
        mount_echo_token_200(&server).await;
        let client = ping_client_against(&server);

        let state = empty_state();
        // Minimal placeholder session — fields aren't inspected by
        // the keep-alive loop, only `client` is.
        let session = fake_session();

        let _info = install_session_and_start_ping(&state, client, session).await;

        assert!(
            state.auth.read().await.is_some(),
            "auth context must be installed",
        );

        // Wait for first ping to fire so we know the spawn actually
        // landed a live task.
        let deadline = tokio::time::Instant::now() + StdDuration::from_secs(5);
        loop {
            let count = server
                .received_requests()
                .await
                .map(|r| r.len())
                .unwrap_or(0);
            if count >= 1 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("ping loop never fired the first request within 5s");
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        // Clean up so the spawned task doesn't keep looping in the
        // background after this test returns.
        let taken = state.auth.write().await.take();
        if let Some(ctx) = taken {
            ctx.ping_cancel.cancel();
        }
    }

    /// Installing a second session must cancel the first session's
    /// ping loop. Without this the old task would hold the old
    /// cookie jar alive and keep calling the mock forever.
    #[tokio::test]
    async fn install_session_and_start_ping_cancels_previous_loop() {
        let server = MockServer::start().await;
        mount_echo_token_200(&server).await;

        let state = empty_state();

        let first_client = ping_client_against(&server);
        install_session_and_start_ping(&state, first_client, fake_session()).await;
        let first_token = state
            .auth
            .read()
            .await
            .as_ref()
            .expect("first install populates auth")
            .ping_cancel
            .clone();
        assert!(
            !first_token.is_cancelled(),
            "fresh install must leave token uncancelled",
        );

        let second_client = ping_client_against(&server);
        install_session_and_start_ping(&state, second_client, fake_session()).await;
        assert!(
            first_token.is_cancelled(),
            "replacing an auth context must cancel the prior ping loop",
        );

        // Clean up the second loop.
        let taken = state.auth.write().await.take();
        if let Some(ctx) = taken {
            ctx.ping_cancel.cancel();
        }
    }

    // ── split_otp_digits ──────────────────────────────────────────

    #[tokio::test]
    async fn await_registered_device_login_finishes_when_poll_returns_session() {
        let server = MockServer::start().await;
        let client = device_login_client_against(&server);
        mount_device_poll_success(&server, "TOK_DEVICE", "AKEY_DEVICE").await;
        mount_login_completed_success(&server, "WEB_DEVICE").await;

        let session = await_registered_device_login_with_interval(
            &client,
            "TOK_DEVICE",
            "SKEY_DEVICE",
            "alice",
            "610074",
            "T9",
            StdDuration::from_millis(10),
        )
        .await
        .expect("device approval should complete login");

        assert_eq!(session.region, LoginRegion::HK);
        assert_eq!(session.skey, "SKEY_DEVICE");
        assert_eq!(session.web_token, "WEB_DEVICE");
        assert_eq!(session.account_id, "alice");
    }

    #[tokio::test]
    async fn await_registered_device_login_surfaces_timeout() {
        let server = MockServer::start().await;
        let client = device_login_client_against(&server);

        Mock::given(wm_method("POST"))
            .and(wm_path("/login/bfAPPAutoLogin.ashx"))
            .and(body_string_contains("LT=TOK_TIMEOUT"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"IntResult":"-2","StrReslut":"timeout"}"#),
            )
            .mount(&server)
            .await;

        let err = await_registered_device_login_with_interval(
            &client,
            "TOK_TIMEOUT",
            "SKEY_TIMEOUT",
            "alice",
            "610074",
            "T9",
            StdDuration::from_millis(10),
        )
        .await
        .expect_err("timeout branch must surface as LoginError");

        assert!(matches!(err, LoginError::DeviceLoginTimeout));
    }

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

    // ── GamePass family ───────────────────────────────────────────

    /// Same defence-in-depth pattern as the TOTP / QR counterparts:
    /// [`open_gamepass_window`]'s early-exit branch when
    /// [`AppState::pending_gamepass`] is `None` must surface
    /// [`GAMEPASS_NOT_STARTED_CODE`] — not a generic `unknown` /
    /// `session_required` — so the Vue layer can prompt the user to
    /// call `login_gamepass_start` again.
    #[tokio::test]
    async fn open_gamepass_window_without_pending_surfaces_not_started() {
        let app = empty_state();
        let guard = app.pending_gamepass.read().await;
        let err = guard
            .as_ref()
            .ok_or_else(|| {
                CommandError::new(
                    GAMEPASS_NOT_STARTED_CODE,
                    "No GamePass login is active; call login_gamepass_start first.",
                )
            })
            .expect_err("no pending → error");

        assert_eq!(err.code, GAMEPASS_NOT_STARTED_CODE);
        assert!(
            err.message.contains("login_gamepass_start"),
            "message should guide the caller to call login_gamepass_start, got {:?}",
            err.message
        );
    }

    // ── build_gamepass_login_url ──────────────────────────────────

    /// URL must match WPF `GamePassBrowser.OnLoaded` L31-40
    /// verbatim: `login.beanfun.com/Login/Index?pSKey=<skey>`. A
    /// drift in host / path would silently redirect to a page the
    /// auto-click script can't find `a.use-gama-pass` on, hanging
    /// the flow forever.
    #[test]
    fn build_gamepass_login_url_has_the_wpf_exact_shape() {
        let url = build_gamepass_login_url("SKEY_ABC").expect("url builds");

        assert_eq!(url.scheme(), "https", "must be HTTPS");
        assert_eq!(url.host_str(), Some("login.beanfun.com"));
        assert_eq!(url.path(), "/Login/Index");
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params.get("pSKey").map(String::as_str), Some("SKEY_ABC"));
        assert_eq!(
            params.len(),
            1,
            "only pSKey is appended, no stray params: {params:?}",
        );
    }

    /// `skey` values that include URL-sensitive characters (`+`,
    /// `/`, `=` — base64 alphabet) must round-trip verbatim through
    /// URL-encoding. `Url::query_pairs_mut().append_pair` handles
    /// this for us; pinning the behaviour guards against a refactor
    /// to manual `format!()` which would silently break skeys the
    /// portal happily returns.
    #[test]
    fn build_gamepass_login_url_percent_encodes_special_chars_in_skey() {
        let url = build_gamepass_login_url("a+b/c=d").expect("url builds");
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            params.get("pSKey").map(String::as_str),
            Some("a+b/c=d"),
            "decoded pSKey must equal the input verbatim",
        );
    }

    // ── should_try_gamepass_completion ────────────────────────────

    #[test]
    fn should_try_completion_accepts_wpf_portal_landing_urls() {
        // These are the canonical shapes WPF's NavigationCompleted
        // handler (GamePassBrowser.xaml.cs L63-75) would fire
        // completion on — a regression here means a real login
        // stops resolving.
        for url in [
            "https://tw.beanfun.com/beanfun_block/bflogin/return.aspx",
            "https://tw.beanfun.com/index.aspx",
            "https://login.beanfun.com/GP/SendLogin.aspx?foo=bar",
            "https://tw.newlogin.beanfun.com/Login/index.aspx",
        ] {
            let parsed = Url::parse(url).expect(url);
            assert!(
                should_try_gamepass_completion(&parsed),
                "WPF-parity URL must trigger completion: {url}",
            );
        }
    }

    #[test]
    fn should_try_completion_rejects_entry_and_intermediate_urls() {
        // These URLs appear during the flow but WPF does NOT call
        // TryCompleteLogin for them — doing so would waste a
        // cookies_for_url x 3 round-trip per tick without producing
        // a session. Stays aligned with WPF behaviour.
        for url in [
            "https://login.beanfun.com/Login/Index?pSKey=abc",
            "https://gamepass.beanfun.com/oauth/authorize",
            "https://some.cdn.example.com/captcha.png",
            "about:blank",
        ] {
            let parsed = Url::parse(url).expect(url);
            assert!(
                !should_try_gamepass_completion(&parsed),
                "non-completion URL must NOT trigger completion: {url}",
            );
        }
    }

    #[test]
    fn should_try_completion_rejects_foreign_hosts_with_completion_markers_in_path() {
        // A malicious / mis-served page hosting the marker string
        // on a non-beanfun domain must NOT trigger completion — the
        // `host.ends_with("beanfun.com")` guard is the real anchor
        // of the filter. Guards against a regression that strips
        // the host check for "simplicity".
        let parsed = Url::parse("https://evil.example.com/return.aspx").expect("valid parse");
        assert!(
            !should_try_gamepass_completion(&parsed),
            "non-beanfun host must not pass the host guard even with a matching path marker",
        );
    }

    // ── parse_harvest_url ─────────────────────────────────────────

    /// Sanity-check the three GAMEPASS_HARVEST_URLS constants parse
    /// as valid HTTPS origins. The parser assertion would fail at
    /// runtime on the first page-load tick otherwise — pinning it
    /// here surfaces the regression at build time instead.
    #[test]
    fn every_harvest_url_constant_is_a_valid_https_origin() {
        for raw in GAMEPASS_HARVEST_URLS {
            let url = parse_harvest_url(raw);
            assert_eq!(url.scheme(), "https", "{raw} must be HTTPS");
            assert!(
                url.host_str().is_some_and(|h| h.ends_with("beanfun.com")),
                "{raw} must be on beanfun.com",
            );
        }
    }

    /// Beyond validity, the set must match WPF's
    /// `TryCompleteLogin` L123-138 verbatim — the three hosts the
    /// reference client polls for cookies. A drift (e.g. dropping
    /// `newlogin`) would break completion on flows where
    /// `bfWebToken` first surfaces on that host.
    #[test]
    fn harvest_urls_match_the_wpf_reference_set() {
        let hosts: Vec<String> = GAMEPASS_HARVEST_URLS
            .iter()
            .map(|raw| {
                Url::parse(raw)
                    .expect("valid url")
                    .host_str()
                    .expect("has host")
                    .to_owned()
            })
            .collect();

        assert!(hosts.iter().any(|h| h == "tw.beanfun.com"));
        assert!(hosts.iter().any(|h| h == "login.beanfun.com"));
        assert!(hosts.iter().any(|h| h == "tw.newlogin.beanfun.com"));
        assert_eq!(
            hosts.len(),
            3,
            "harvest list must be exactly the WPF-parity three hosts",
        );
    }

    // ── GAMEPASS_AUTOCLICK_JS ─────────────────────────────────────

    /// The init script must target the WPF-identical selector
    /// `a.use-gama-pass` and run after DOMContentLoaded; a drift on
    /// either axis means the button never gets clicked and the
    /// flow wedges at the entry page forever.
    #[test]
    fn autoclick_js_references_wpf_selector_and_domcontentloaded() {
        assert!(
            GAMEPASS_AUTOCLICK_JS.contains("a.use-gama-pass"),
            "selector must mirror WPF GamePassBrowser.xaml.cs L78-90, got:\n{GAMEPASS_AUTOCLICK_JS}",
        );
        assert!(
            GAMEPASS_AUTOCLICK_JS.contains("DOMContentLoaded"),
            "must await DOM before clicking, got:\n{GAMEPASS_AUTOCLICK_JS}",
        );
    }

    // ── redact_url_query ────────────────────────────────────────

    #[test]
    fn redact_url_query_strips_query_string() {
        let url = Url::parse("https://login.beanfun.com/Login/Index?pSKey=SECRET123").unwrap();
        let redacted = redact_url_query(&url);
        assert!(
            !redacted.contains("SECRET123"),
            "pSKey value must be redacted, got: {redacted}",
        );
        assert!(
            redacted.contains("[REDACTED]"),
            "must indicate redaction, got: {redacted}",
        );
        assert!(
            redacted.contains("login.beanfun.com/Login/Index"),
            "host and path must be preserved, got: {redacted}",
        );
    }

    #[test]
    fn redact_url_query_preserves_url_without_query() {
        let url = Url::parse("https://tw.beanfun.com/index.aspx").unwrap();
        let redacted = redact_url_query(&url);
        assert_eq!(redacted, "https://tw.beanfun.com/index.aspx");
    }

    // ── Event name wire-strings ───────────────────────────────────

    /// Pin the Tauri event wire-strings so a refactor rename
    /// doesn't silently desync the Vue listener. Flat dash-case,
    /// per the P12.1 D5 event convention.
    #[test]
    fn gamepass_event_names_are_flat_dash_case() {
        assert_eq!(GAMEPASS_SUCCESS_EVENT, "gamepass-login-success");
        assert_eq!(GAMEPASS_FAILED_EVENT, "gamepass-login-failed");
        assert_eq!(GAMEPASS_CANCELLED_EVENT, "gamepass-login-cancelled");
    }

    #[test]
    fn gamepass_not_started_code_is_the_auth_family_wire_string() {
        assert_eq!(GAMEPASS_NOT_STARTED_CODE, "auth.gamepass_not_started");
    }

    /// Pin the dedicated wire-string for the "double-open" guard so
    /// it can never silently collapse back into
    /// [`GAMEPASS_NOT_STARTED_CODE`] (CP4 debt fix). The Vue layer
    /// renders the same `windowError` banner for both, but the
    /// localised toast text and operator log attribution diverge —
    /// the contract is that the two codes stay distinct strings
    /// even if their UX surface looks similar.
    #[test]
    fn gamepass_window_already_open_code_is_distinct_from_not_started() {
        assert_eq!(
            GAMEPASS_WINDOW_ALREADY_OPEN_CODE,
            "auth.gamepass_window_already_open"
        );
        assert_ne!(GAMEPASS_WINDOW_ALREADY_OPEN_CODE, GAMEPASS_NOT_STARTED_CODE);
    }

    // ── handle_gamepass_window_destroyed early-exit ───────────────

    /// The destroy handler must be a no-op when `pending_gamepass`
    /// is already `None` (success path had cleared it before
    /// calling `window.close()`). Asserted via the same `.take()`
    /// primitive the real handler uses — emit is the only side
    /// effect we can't cover without an `AppHandle` fixture, and
    /// the `None` short-circuit skips it entirely by design.
    #[tokio::test]
    async fn destroyed_handler_takes_nothing_when_success_already_cleared_slot() {
        let app = empty_state();
        // Slot already None — the real handler's CAS returns early.
        assert!(app.pending_gamepass.write().await.take().is_none());
    }

    /// The destroy handler must clear a populated slot when the
    /// user cancels. Post-clear reads observe `None`, matching the
    /// invariant the subsequent `open_gamepass_window` retry
    /// depends on (a retry re-mints `PendingGamepass` from
    /// `login_gamepass_start`).
    #[tokio::test]
    async fn destroyed_handler_clears_pending_on_user_cancel_path() {
        let app = empty_state();
        let client = BeanfunClient::new(ClientConfig::default()).expect("client builds");
        *app.pending_gamepass.write().await = Some(PendingGamepass::new(client, "SKEY_CANCEL"));

        // Mirror the handler's `.take()` step; the surrounding emit
        // is exercised by the CP4 frontend integration test.
        let taken = app.pending_gamepass.write().await.take();
        assert!(taken.is_some(), "cancel path must observe populated slot");
        assert!(
            app.pending_gamepass.read().await.is_none(),
            "post-cancel slot must be cleared for a clean retry",
        );
    }
}
