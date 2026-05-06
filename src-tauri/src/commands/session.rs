//! Session-required command prelude.
//!
//! Every command that needs an authenticated Beanfun session — which
//! is the vast majority of P10.2/P10.3 commands — starts with one
//! line:
//!
//! ```ignore
//! use crate::commands::{error::CommandError, session::require_auth, state::AppState};
//! use tauri::State;
//!
//! #[tauri::command]
//! #[specta::specta]
//! pub async fn get_otp(state: State<'_, AppState>) -> Result<String, CommandError> {
//!     let (client, session) = require_auth(state.inner()).await?;
//!     // ... business logic using `client` + `session` ...
//!     Ok("...".into())
//! }
//! ```
//!
//! # Why a helper?
//!
//! - **DRY** — the `.auth.read().await` / `.as_ref().ok_or(...)`
//!   dance would otherwise appear at the top of every session-scoped
//!   command. Centralizing it in one place means the
//!   `auth.session_required` [`CommandError`] code is minted from
//!   a single source (see [`SESSION_REQUIRED_CODE`]) — crucial for
//!   frontend i18n key stability.
//! - **SRP** — authentication gating is orthogonal to the command's
//!   business logic. Keeping the guard in its own module keeps the
//!   per-command file focused on its domain call.
//! - **No held lock across `.await`** — the helper returns **owned**
//!   clones of `BeanfunClient` and `Session`, so the short-lived
//!   `RwLock` read guard is dropped before the caller's `.await`
//!   points. This matters because a command that held the read guard
//!   throughout its body would block `logout` / re-login from
//!   acquiring the write lock — potentially indefinitely on a slow
//!   Beanfun response.
//!
//! Cloning is cheap:
//! - [`BeanfunClient`] is `Arc`-based (cookie store, inner `reqwest`
//!   clients, config).
//! - [`Session`] is a small bundle of `String`s; the plaintext-secrets
//!   it contains (`skey`, `web_token`) are already living in memory
//!   as long as the user is logged in, so a clone does not change the
//!   secret-exposure footprint.
//!
//! # Command-layer-owned codes
//!
//! | Code                       | When                                        |
//! |----------------------------|---------------------------------------------|
//! | `auth.session_required`    | command invoked while [`AppState::auth`] is `None` |
//!
//! This is the only code minted in this module. Every **domain**
//! error surfaces through the [`From`] impls in
//! [`super::error`][crate::commands::error] instead.

use crate::commands::{error::CommandError, state::AppState};
use crate::services::beanfun::{client::BeanfunClient, session::Session};

/// Stable [`CommandError::code`][crate::commands::error::CommandError::code]
/// returned from [`require_auth`] when no session is active. Exposed
/// as a `const` so tests, documentation, and the frontend i18n table
/// all reference one source of truth.
pub const SESSION_REQUIRED_CODE: &str = "auth.session_required";

/// Human-readable message paired with [`SESSION_REQUIRED_CODE`].
/// Shared across every site that mints this error so a future
/// wording change only touches one line.
pub const SESSION_REQUIRED_MESSAGE: &str =
    "No active Beanfun session. Please log in and try again.";

/// Resolve the `(client, session)` pair from
/// [`AppState::auth`][crate::commands::state::AppState::auth], mapping
/// the unauthenticated case to a [`CommandError`] with
/// [`SESSION_REQUIRED_CODE`].
///
/// Returns **owned** clones so the caller can drop the `RwLock` read
/// guard immediately — see the [module-level docs][self] for why
/// this matters.
pub(crate) async fn require_auth(
    state: &AppState,
) -> Result<(BeanfunClient, Session), CommandError> {
    let guard = state.auth.read().await;
    match guard.as_ref() {
        Some(ctx) => Ok((ctx.client.clone(), ctx.session.clone())),
        None => Err(CommandError::new(
            SESSION_REQUIRED_CODE,
            SESSION_REQUIRED_MESSAGE,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::state::AuthContext;
    use crate::services::beanfun::client::{ClientConfig, LoginRegion};
    use std::path::PathBuf;

    fn sample_context() -> AuthContext {
        let client = BeanfunClient::new(ClientConfig::default()).expect("client builds");
        let session = Session::new(
            LoginRegion::TW,
            "SKEY_TEST",
            "BFWT_TEST",
            "alice",
            "610074",
            "T9",
        );
        AuthContext::new(client, session)
    }

    #[tokio::test]
    async fn require_auth_on_empty_state_returns_session_required() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        let err = require_auth(&state)
            .await
            .expect_err("must reject unauthenticated call");
        assert_eq!(err.code, SESSION_REQUIRED_CODE);
        assert!(
            !err.message.is_empty(),
            "message must be non-empty for `tracing` surfaces"
        );
        assert!(
            err.details.is_none(),
            "no structured details needed for the session-required case"
        );
    }

    #[tokio::test]
    async fn require_auth_on_populated_state_returns_cloned_pair() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        {
            let mut guard = state.auth.write().await;
            *guard = Some(sample_context());
        }

        let (client, session) = require_auth(&state).await.expect("auth populated");
        assert_eq!(client.config().region, LoginRegion::TW);
        assert_eq!(session.account_id, "alice");
        assert_eq!(session.service_code, "610074");
        assert_eq!(session.service_region, "T9");
    }

    /// The returned tuple must be **owned** — no hidden guard hangs
    /// around that would block a concurrent writer. If this test
    /// regresses, a future command that writes to `state.auth` (e.g.
    /// `logout`) while the caller is awaiting a slow Beanfun response
    /// would deadlock.
    #[tokio::test]
    async fn require_auth_does_not_retain_lock_after_return() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        {
            let mut guard = state.auth.write().await;
            *guard = Some(sample_context());
        }

        let _pair = require_auth(&state).await.expect("populated");
        assert!(
            state.auth.try_write().is_ok(),
            "require_auth must not retain the read guard across the return value",
        );
    }
}
