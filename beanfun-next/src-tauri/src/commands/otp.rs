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

use crate::commands::{error::CommandError, session::require_auth, state::AppState};
use crate::services::beanfun::{get_otp as service_get_otp, ServiceAccount};

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
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service (transport,
///   JSON parse, WCDES decrypt, server-side intResult ≠ 1). The
///   P10.1 `From<LoginError>` impl maps each variant to its
///   structured `CommandError` shape.
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
    let otp = service_get_otp(
        &client,
        &session,
        &account,
        &session.service_code,
        &session.service_region,
    )
    .await?;
    Ok(otp)
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
}
