//! Orchestrator for the **TW Regular** login flow — `account + password`
//! against the Taiwan portal.
//!
//! Sequence (one arrow per HTTP call, WPF reference line numbers in
//! `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`):
//!
//! 1. `GET /` → session key                             (L30-38, step 0)
//! 2. `GET Login/Index?pSKey=…` → antiforgery token     (L40-54)
//! 3. `POST Login/CheckAccountType` → captcha token     (L57-78)
//! 4. `POST Login/AccountLogin` → success / advance ck  (L80-113)
//! 5. `GET Login/SendLogin` → hidden-form payload       (L114-146)
//! 6. `POST return.aspx` (no-redirect) → bfWebToken     (L148-176)
//!
//! # reCAPTCHA token-replay (issues #313 / #315 / #318)
//!
//! Steps 3 and 4 can each be gated behind a Google reCAPTCHA Enterprise
//! challenge. Rather than doing the whole login inside a WebView (the
//! #308/#309 approach, which the WebView's Tracking Prevention broke), we
//! keep the login **headless** and only pop a tiny window to solve the
//! reCAPTCHA *widget* on beanfun's own origin, then replay the solved
//! token over HTTP.
//!
//! Mechanically this needs the flow to **pause** after step 3 and/or step
//! 4 when the server demands a token, hand a resumable context up to the
//! command layer, and resume the *same* HTTP session once the user solves
//! the widget. Hence the two entry points below —
//! [`tw_login_start`] (fresh) and [`tw_login_resume`] (after a solve) —
//! both returning a [`TwStepOutcome`] that is either a finished
//! [`Session`] or a [`TwStepOutcome::RecaptchaRequired`] carrying the
//! [`TwLoginContext`] needed to resume. Empty-first: each step is tried
//! with an empty token, and only escalates to a solve when the server says
//! so.

use super::{
    account_login, check_account_type, get_login_index, get_session_key, post_return_aspx,
    send_login, AccountLoginOutcome, CheckAccountOutcome,
};
use crate::services::beanfun::{BeanfunClient, Credentials, LoginError, LoginRegion, Session};

/// Which TW-Regular POST is currently gated behind a reCAPTCHA solve.
///
/// Serialised into the `auth.recaptcha_required` command-error details
/// (and the WebView URL-fragment discriminator) so the frontend and the
/// backend agree on which step to replay the solved token into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RecaptchaStep {
    /// `POST Login/CheckAccountType` (after the account, before the
    /// password).
    CheckAccount,
    /// `POST Login/AccountLogin` (after the password).
    AccountLogin,
}

impl RecaptchaStep {
    /// Compact wire token used in the reCAPTCHA WebView's
    /// `#mltoken=<step>~<token>` URL fragment.
    pub fn as_wire(self) -> &'static str {
        match self {
            RecaptchaStep::CheckAccount => "check",
            RecaptchaStep::AccountLogin => "login",
        }
    }

    /// Parse the wire token produced by [`Self::as_wire`].
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "check" => Some(RecaptchaStep::CheckAccount),
            "login" => Some(RecaptchaStep::AccountLogin),
            _ => None,
        }
    }
}

/// Everything needed to resume a paused TW-Regular login on the **same**
/// HTTP session after the user solves a reCAPTCHA widget.
///
/// The `BeanfunClient` (which owns the cookie jar) is held by the command
/// layer alongside this context, so only the plain login-page tokens live
/// here.
#[derive(Debug, Clone)]
pub struct TwLoginContext {
    /// Portal session key (`pSKey`). Reused verbatim — re-fetching it (or
    /// re-running `CheckAccountType` against a fresh key) loops the
    /// challenge and can trip a ~5-minute IP lock (task spec §4).
    pub skey: String,
    /// `__RequestVerificationToken` scraped from `Login/Index`.
    pub verification_token: String,
    /// `Login/Index?pSKey=…` URL, sent verbatim as `Referer`.
    pub index_url: String,
}

/// Outcome of a [`tw_login_start`] / [`tw_login_resume`] step.
pub enum TwStepOutcome {
    /// The login finished; the caller installs this [`Session`].
    Complete(Box<Session>),
    /// The server demands a reCAPTCHA token for `step`. The command layer
    /// stashes `ctx` (+ the client), opens the widget window, and calls
    /// [`tw_login_resume`] with the solved token.
    RecaptchaRequired {
        ctx: TwLoginContext,
        step: RecaptchaStep,
    },
    /// `AccountLogin` returned advance-check (進階驗證, `ResultCode==2`).
    /// The command layer keeps `ctx` parked so that, once the user clears
    /// the verify challenge, phase 2 can be **re-submitted on the same
    /// session** — re-running `CheckAccountType` from scratch loops the
    /// challenge and trips an IP lock (task spec §4).
    AdvanceCheckRequired {
        ctx: TwLoginContext,
        url: Option<String>,
    },
}

/// Begin a fresh TW-Regular login: acquire the session key + antiforgery
/// token, then run `CheckAccountType` (and, if it passes, `AccountLogin`)
/// with **empty** reCAPTCHA tokens. Escalates to
/// [`TwStepOutcome::RecaptchaRequired`] at the first step the server gates.
pub async fn tw_login_start(
    client: &BeanfunClient,
    creds: &Credentials,
) -> Result<TwStepOutcome, LoginError> {
    debug_assert_eq!(
        client.config().region,
        LoginRegion::TW,
        "tw_login_start requires a TW-configured BeanfunClient"
    );

    let skey = get_session_key(client).await?;
    let index = get_login_index(client, &skey).await?;
    let ctx = TwLoginContext {
        skey,
        verification_token: index.verification_token,
        index_url: index.index_url.as_str().to_owned(),
    };

    run_from_check(client, ctx, creds, "").await
}

/// Resume a paused login after the user solved the reCAPTCHA widget for
/// `step`. `token` is the solved-on-origin reCAPTCHA token, replayed into
/// that step's `Captcha` field.
pub async fn tw_login_resume(
    client: &BeanfunClient,
    ctx: TwLoginContext,
    creds: &Credentials,
    step: RecaptchaStep,
    token: &str,
) -> Result<TwStepOutcome, LoginError> {
    match step {
        RecaptchaStep::CheckAccount => run_from_check(client, ctx, creds, token).await,
        RecaptchaStep::AccountLogin => run_from_login(client, ctx, creds, token).await,
    }
}

/// Run from `CheckAccountType` (with `check_captcha`) onward. On a passing
/// check, continues into `AccountLogin` with an **empty** token
/// (empty-first for that step too).
async fn run_from_check(
    client: &BeanfunClient,
    ctx: TwLoginContext,
    creds: &Credentials,
    check_captcha: &str,
) -> Result<TwStepOutcome, LoginError> {
    match check_account_type(
        client,
        &ctx.skey,
        &creds.account,
        check_captcha,
        &ctx.verification_token,
        &ctx.index_url,
    )
    .await?
    {
        CheckAccountOutcome::RecaptchaRequired => {
            tracing::info!(
                step = "TwRegular.CheckAccountType",
                account_id = %creds.account,
                "reCAPTCHA required; pausing for widget solve"
            );
            Ok(TwStepOutcome::RecaptchaRequired {
                ctx,
                step: RecaptchaStep::CheckAccount,
            })
        }
        CheckAccountOutcome::Proceed { server_captcha } => {
            run_from_login(client, ctx, creds, &server_captcha).await
        }
    }
}

/// Run from `AccountLogin` (with `account_captcha`) onward: on success,
/// finish with the TW completion tail (`SendLogin` → `return.aspx`) and
/// build the [`Session`].
async fn run_from_login(
    client: &BeanfunClient,
    ctx: TwLoginContext,
    creds: &Credentials,
    account_captcha: &str,
) -> Result<TwStepOutcome, LoginError> {
    let outcome = match account_login(
        client,
        &ctx.skey,
        creds,
        account_captcha,
        &ctx.verification_token,
        &ctx.index_url,
    )
    .await
    {
        Ok(outcome) => outcome,
        // Advance-check must carry `ctx` up so the command layer can park it
        // and re-submit phase 2 on the SAME session after verify (task spec
        // §4). The `?` would drop `ctx`, forcing a session-losing restart.
        Err(LoginError::AdvanceCheckRequired { url }) => {
            tracing::info!(
                step = "TwRegular.AccountLogin",
                account_id = %creds.account,
                "advance-check required; parking session for post-verify resume"
            );
            return Ok(TwStepOutcome::AdvanceCheckRequired { ctx, url });
        }
        Err(e) => return Err(e),
    };

    match outcome {
        AccountLoginOutcome::RecaptchaRequired => {
            tracing::info!(
                step = "TwRegular.AccountLogin",
                account_id = %creds.account,
                "reCAPTCHA required; pausing for widget solve"
            );
            Ok(TwStepOutcome::RecaptchaRequired {
                ctx,
                step: RecaptchaStep::AccountLogin,
            })
        }
        AccountLoginOutcome::Success => {
            // WPF L124 — TW Regular's SendLogin Accept header. Differs from
            // the QR flow's Accept; see `login/send_login.rs` module docs.
            let form = send_login(
                client,
                &ctx.index_url,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .await?;
            let web_token = post_return_aspx(client, &form).await?;

            tracing::info!(
                step = "TwRegular",
                region = ?LoginRegion::TW,
                account_id = %creds.account,
                "login flow completed successfully"
            );

            Ok(TwStepOutcome::Complete(Box::new(Session::new(
                LoginRegion::TW,
                &ctx.skey,
                &web_token,
                &creds.account,
                LoginRegion::TW.default_service_code(),
                LoginRegion::TW.default_service_region(),
            ))))
        }
    }
}

/// Single-shot TW-Regular login, kept for the top-level [`super::login_with`]
/// dispatcher and the wiremock integration tests.
///
/// Drives [`tw_login_start`] but has no way to solve a reCAPTCHA (there is
/// no interactive surface here), so a reCAPTCHA demand surfaces as
/// [`LoginError::RecaptchaRequired`]. The interactive command layer uses
/// [`tw_login_start`] / [`tw_login_resume`] directly instead.
pub async fn login_tw_regular(
    client: &BeanfunClient,
    creds: &Credentials,
) -> Result<Session, LoginError> {
    match tw_login_start(client, creds).await? {
        TwStepOutcome::Complete(session) => Ok(*session),
        TwStepOutcome::RecaptchaRequired { ctx, .. } => {
            Err(LoginError::RecaptchaRequired { skey: ctx.skey })
        }
        TwStepOutcome::AdvanceCheckRequired { url, .. } => {
            Err(LoginError::AdvanceCheckRequired { url })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recaptcha_step_wire_round_trips() {
        for step in [RecaptchaStep::CheckAccount, RecaptchaStep::AccountLogin] {
            assert_eq!(RecaptchaStep::from_wire(step.as_wire()), Some(step));
        }
    }

    #[test]
    fn recaptcha_step_from_wire_rejects_unknown() {
        assert_eq!(RecaptchaStep::from_wire("bogus"), None);
    }
}
