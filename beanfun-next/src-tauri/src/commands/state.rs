//! `AppState` — shared runtime dependencies injected into every Tauri
//! command.
//!
//! Managed through [`tauri::Builder::manage`] at startup (P10.1 D7)
//! so every `#[tauri::command]` function can access the same instance
//! via `State<'_, AppState>` (owned by Tauri, cloned by reference).
//!
//! # Contents (P10.1 minimal)
//!
//! - `storage_root`: [`PathBuf`] pointing at the root directory under
//!   which every on-disk artifact lives
//!   (`%APPDATA%\Beanfun` in production, a `tempfile::TempDir` path
//!   in tests). The caller (Tauri `setup` hook) resolves this once at
//!   boot; `AppState` treats it as an opaque root.
//! - `session`: [`RwLock<Option<Session>>`] — the authenticated
//!   Beanfun session, `None` until the user logs in. Wrapped in
//!   [`tokio::sync::RwLock`] (not the std one) so guards are `Send`
//!   and survive `.await` points inside async command bodies.
//!
//! # Lifecycle
//!
//! ```text
//! main()
//!   │
//!   ├─ resolve %APPDATA%\Beanfun (fallible — future P10.1 D7 will
//!   │   surface the env-var-missing case as system.app_data_missing)
//!   │
//!   ├─ AppState::new(root)            infallible in P10.1
//!   │
//!   ├─ tauri::Builder::default()
//!   │     .manage(app_state)          ← injects into every command
//!   │     .invoke_handler(...)
//!   │     .run(...)
//! ```
//!
//! # Future expansion
//!
//! - **P10.2** adds `http_client: reqwest::Client` (cookie-enabled)
//!   and fills [`Session`] with `bf_web_token` / `avatar` / `bf_id` /
//!   cached account list; `new` will become fallible (reqwest builder
//!   can fail on TLS misconfiguration, mapped to
//!   `system.http_client_init_failed`).
//! - **P10.3** extends [`Session`] with the per-launch child-process
//!   handle(s) for auto-paste bookkeeping.
//!
//! Keeping the P10.1 shape minimal avoids dead-code warnings and
//! premature coupling to types (e.g. `reqwest::Client`) whose first
//! real consumer doesn't land until P10.2.

use std::path::PathBuf;

use tokio::sync::RwLock;

/// Authenticated Beanfun session payload. **Placeholder** for P10.1 —
/// the concrete shape is deferred to P10.2 (`bf_web_token`, `avatar`,
/// `bf_id`, cached service-account list).
///
/// Being an empty struct keeps the type automatically `Send + Sync`,
/// which lets [`AppState::session`] compile against the full
/// [`RwLock`] bound in P10.1 without follow-up refactors when the
/// real fields land.
#[derive(Debug, Default)]
pub struct Session;

/// Shared application state injected into every Tauri command.
///
/// See the [module-level documentation][self] for the lifecycle and
/// expansion plan.
pub struct AppState {
    /// Root directory for every on-disk artifact (Users.dat,
    /// Config.xml, update cache, logs). Typically `%APPDATA%\Beanfun`
    /// in production; a `tempfile::TempDir` path in tests.
    pub storage_root: PathBuf,

    /// Current authenticated Beanfun session. `None` at startup;
    /// populated by the login commands (P10.2) and cleared on
    /// `logout` or expiry.
    ///
    /// Uses [`tokio::sync::RwLock`] — guards are `Send` so they
    /// survive `.await` points inside async command bodies, unlike
    /// [`std::sync::RwLock`] which poisons the `!Send` `Guard`.
    pub session: RwLock<Option<Session>>,
}

impl AppState {
    /// Build an [`AppState`] rooted at `storage_root`.
    ///
    /// Currently infallible — P10.1 owns no resource whose
    /// initialization can fail. Fallibility is reintroduced in P10.2
    /// when the HTTP client is added (see
    /// [module-level docs](self#future-expansion)).
    pub fn new(storage_root: PathBuf) -> Self {
        Self {
            storage_root,
            session: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_storage_root_verbatim() {
        let root = PathBuf::from(r"C:\tmp\beanfun-test");
        let state = AppState::new(root.clone());
        assert_eq!(state.storage_root, root);
    }

    #[tokio::test]
    async fn session_starts_as_none() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        let guard = state.session.read().await;
        assert!(guard.is_none(), "session must be None before login");
    }

    #[tokio::test]
    async fn session_can_be_populated_then_cleared() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        {
            let mut guard = state.session.write().await;
            *guard = Some(Session);
        }
        assert!(state.session.read().await.is_some());
        {
            let mut guard = state.session.write().await;
            *guard = None;
        }
        assert!(state.session.read().await.is_none());
    }
}
