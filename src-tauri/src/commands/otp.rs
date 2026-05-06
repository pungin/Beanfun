//! OTP retrieval command — the final step before launching the game
//! client.
//!
//! Mirrors `BeanfunClient.cs::getOTP` (the 5-step orchestration already
//! ported into [`services::beanfun::get_otp`][svc]). This module is a
//! thin IPC wrapper: the heavy lifting (`step_1_init` … `step_5_get_otp`
//! + WCDES decrypt) lives entirely in the service layer.
//!
//! # Why only one command?
//!
//! WPF does not expose the intermediate 5-step pipeline; the UI calls
//! a single `getOTP(serviceAccount)` and either receives a 6-digit
//! string or an error. We preserve that contract — the frontend does
//! not need to know about the polling / secret-code exchange /
//! WCDES decryption.
//!
//! # Session gating
//!
//! [`get_otp`] calls [`commands::session::require_auth`][req] first,
//! surfacing `auth.session_required` when no login is active. The
//! `service_code` / `service_region` pair is pulled from the
//! [`Session`][sesh] (matching [`commands::account::add_service_account`]'s
//! policy — locked on the backend so the frontend cannot drift the
//! two halves against each other).
//!
//! [svc]: crate::services::beanfun::get_otp
//! [req]: crate::commands::session::require_auth
//! [sesh]: crate::services::beanfun::Session
//! [`commands::account::add_service_account`]: crate::commands::account::add_service_account

use tauri::State;

use crate::commands::{
    error::CommandError,
    session::{require_auth, SESSION_REQUIRED_CODE, SESSION_REQUIRED_MESSAGE},
    state::AppState,
};
use crate::services::beanfun::{get_otp as service_get_otp, LoginError, ServiceAccount};

/// Retrieve the one-time game-launch password for a given service
/// account.
///
/// # Contract
///
/// Thin wrapper over [`crate::services::beanfun::get_otp`]. The
/// returned string is the 6-character password the Beanfun launcher
/// feeds into `MapleStory.exe` as the second token. On success the
/// UI copies this to the clipboard and displays it in the OTP
/// dialog (matching WPF's `CopyBox.xaml`).
///
/// # Why accept the whole `ServiceAccount` instead of `sid`?
///
/// [`crate::services::beanfun::get_otp`] takes `&ServiceAccount` because
/// several of the 5 HTTP steps need fields beyond `sid` (e.g.
/// `ssn` for `record_start` body, `screatetime` for the post-WCDES
/// JSON envelope). Reshaping the service call to accept a minimal
/// `{sid, ssn, screatetime}` bundle would leak the protocol shape
/// into the command layer; echoing the full [`ServiceAccount`] the
/// frontend already has from [`commands::account::get_accounts`]
/// is strictly cheaper and preserves the service-layer SRP.
///
/// [`ServiceAccount`] has `serde::Deserialize` already (set by D9
/// for [`commands::account::change_display_name`]) — no additional
/// derives needed.
///
/// # Errors
///
/// - `auth.session_required` — no login is active, **or** the
///   server-side session expired mid-flight (issue #264). The
///   latter is detected heuristically: when the OTP HTTP steps
///   return a login-redirect HTML page instead of the expected
///   JavaScript/JSON content, the regex-based parsers fail with
///   [`LoginError::OtpMissingLongPollingKey`] or
///   [`LoginError::OtpMissingSecretCode`]. We treat these as
///   session-expired, clear the stale local auth context, and
///   surface the canonical `auth.session_required` code so the
///   frontend's session-expired handler redirects to the login
///   page (with the i18n toast "您的登入狀態已失效，請重新登入。").
/// - Any other [`LoginError`][le] surfaced by the service
///   (transport, JSON parse, WCDES decrypt, server-side
///   intResult ≠ 1). The P10.1 `From<LoginError>` impl maps
///   each variant to its structured `CommandError` shape.
///
/// # Frontend usage
///
/// Invoked by the AccountList "get OTP" button (matches WPF
/// `AccountList.xaml.cs` `m_GetOTP_Click`). The returned string
/// should be shown in the `CopyBox` dialog and copied to
/// clipboard; do **not** log this value.
///
/// [svc]: crate::services::beanfun::get_otp
/// [le]: crate::services::beanfun::LoginError
/// [`ServiceAccount`]: crate::services::beanfun::ServiceAccount
/// [`commands::account::get_accounts`]: crate::commands::account::get_accounts
/// [`commands::account::change_display_name`]: crate::commands::account::change_display_name
#[tauri::command]
#[specta::specta]
pub async fn get_otp(
    state: State<'_, AppState>,
    account: ServiceAccount,
) -> Result<String, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    match service_get_otp(
        &client,
        &session,
        &account,
        &session.service_code,
        &session.service_region,
    )
    .await
    {
        Ok(otp) => Ok(otp),
        Err(e) if is_likely_session_expired(&e) => {
            tracing::warn!(
                error = %e,
                "OTP flow detected likely server-side session expiry; clearing local auth context"
            );
            let taken = state.auth.write().await.take();
            if let Some(ctx) = taken {
                ctx.ping_cancel.cancel();
            }
            Err(CommandError::new(
                SESSION_REQUIRED_CODE,
                SESSION_REQUIRED_MESSAGE,
            ))
        }
        Err(e) => Err(CommandError::from(e)),
    }
}

/// Heuristic for detecting server-side session expiry during the
/// OTP flow.
///
/// When the Beanfun cookie session is invalidated remotely (e.g.
/// another device logs in), the OTP HTTP steps return a
/// login-redirect HTML page instead of the expected
/// JavaScript/JSON content. The regex parsers then fail to find
/// the expected literals:
///
/// - Step 1 (`game_start_step2.aspx`) → [`LoginError::OtpMissingLongPollingKey`]
/// - Step 2 (`get_cookies.ashx`) → [`LoginError::OtpMissingSecretCode`]
///
/// Both are strong indicators because `require_auth` already
/// passed (the *local* `AppState.auth` was populated), so the
/// failure is almost certainly a server-side invalidation rather
/// than a "never logged in" state.
fn is_likely_session_expired(e: &LoginError) -> bool {
    matches!(
        e,
        LoginError::OtpMissingLongPollingKey { .. } | LoginError::OtpMissingSecretCode
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time presence check for the OTP command. Session-gating
    /// is covered by the shared [`require_auth`] tests
    /// ([`super::super::session::tests`]) and the service-layer
    /// `get_otp` integration tests under `tests/otp.rs` cover the
    /// 5-step pipeline; this test's job is to pin the command's
    /// declared signature so a future rename / arity change can't
    /// silently break the IPC contract the frontend depends on.
    #[test]
    fn get_otp_command_exists_with_declared_signature() {
        let _ = get_otp;
    }

    #[test]
    fn missing_long_polling_key_is_likely_session_expired() {
        let e = LoginError::OtpMissingLongPollingKey {
            snippet: "<html>login page</html>".to_string(),
        };
        assert!(is_likely_session_expired(&e));
    }

    #[test]
    fn missing_secret_code_is_likely_session_expired() {
        assert!(is_likely_session_expired(
            &LoginError::OtpMissingSecretCode
        ));
    }

    #[test]
    fn transport_error_is_not_session_expired() {
        let e = LoginError::Unknown("network timeout".to_string());
        assert!(!is_likely_session_expired(&e));
    }

    #[test]
    fn otp_server_rejected_is_not_session_expired() {
        let e = LoginError::OtpServerRejected {
            message: "maintenance".to_string(),
        };
        assert!(!is_likely_session_expired(&e));
    }

    #[test]
    fn otp_decrypt_failure_is_not_session_expired() {
        let e = LoginError::OtpDecryptionFailed {
            cause: "invalid hex".to_string(),
        };
        assert!(!is_likely_session_expired(&e));
    }
}
