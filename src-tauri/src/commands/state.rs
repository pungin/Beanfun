//! `AppState` — shared runtime dependencies injected into every Tauri
//! command.
//!
//! Managed through [`tauri::Builder::manage`] at startup (P10.1 D7)
//! so every `#[tauri::command]` function can access the same instance
//! via `State<'_, AppState>` (owned by Tauri, cloned by reference).
//!
//! # Contents
//!
//! - `storage_root`: [`PathBuf`] pointing at the root directory under
//!   which every on-disk artifact lives
//!   (`%APPDATA%\Beanfun` in production, a `tempfile::TempDir` path
//!   in tests). The caller (Tauri `setup` hook) resolves this once at
//!   boot; `AppState` treats it as an opaque root.
//! - `auth`: [`RwLock<Option<AuthContext>>`] — the authenticated
//!   Beanfun session **and** its HTTP client, wrapped together so the
//!   pair is swapped atomically on login / logout (no window where the
//!   session points at a stale cookie jar or vice versa). `None` until
//!   the user logs in. Uses [`tokio::sync::RwLock`] (not the std one)
//!   so guards are `Send` and survive `.await` points inside async
//!   command bodies.
//!
//! # Why one lock for `{client, session}` instead of two?
//!
//! Holding the HTTP client ([`BeanfunClient`], which owns the cookie
//! jar) and the [`Session`] (which owns the `skey` / `web_token`) under
//! **one** lock eliminates the atomicity gap where a reader could
//! observe `session = Some(...)` but `client = None` (or vice versa) —
//! e.g. mid-logout. [`AuthContext`] bundles the two so every login
//! stores the pair with a single write, every logout clears the pair
//! with a single write, and every caller inspects the pair with a
//! single read. The P10.2 pre-flight Q2=B decision (Todo.md L895)
//! locks in this shape.
//!
//! # Lifecycle
//!
//! ```text
//! main()
//!   │
//!   ├─ resolve %APPDATA%\Beanfun (fallible — surfaces the
//!   │   env-var-missing case as system.app_data_missing)
//!   │
//!   ├─ AppState::new(root)            infallible today
//!   │
//!   ├─ tauri::Builder::default()
//!   │     .manage(app_state)          ← injects into every command
//!   │     .invoke_handler(...)
//!   │     .run(...)
//!   │
//!   ├─ login_regular / login_qr_check / login_totp / …
//!   │     set auth = Some(AuthContext { client, session })
//!   │
//!   ├─ … every other command …
//!   │     commands::session::require_auth(&state) → (client, session)
//!   │
//!   └─ logout
//!         set auth = None (the BeanfunClient drops, its cookie jar
//!         goes with it — every follow-up call must re-login)
//! ```
//!
//! # Future expansion
//!
//! - **P10.3** extends [`AuthContext`] with the per-launch child-process
//!   handle(s) for auto-paste bookkeeping (separate `Mutex<Vec<Child>>`
//!   on `AppState` rather than entangling with `auth`, because
//!   child-process handles outlive logout).

use std::path::PathBuf;

use tokio::sync::RwLock;

use crate::services::beanfun::{
    client::BeanfunClient,
    login::{QrLoginInit, TotpChallenge},
    session::Session,
    verify::VerifyPageInfo,
};

/// Authenticated Beanfun session plus its HTTP client.
///
/// Stored as the `Some(_)` variant of [`AppState::auth`] once the user
/// logs in. Cleared back to `None` on logout. The pair is always
/// swapped together so readers never observe a half-populated state.
///
/// Cloning is cheap:
/// - [`BeanfunClient`] is `Arc`-based internally (cookie store and
///   `reqwest::Client` share structural state).
/// - [`Session`] is a handful of `String`s.
///
/// [`commands::session::require_auth`][crate::commands::session::require_auth]
/// uses this cheapness to hand callers an owned `(BeanfunClient,
/// Session)` tuple without holding the `RwLock` read guard across an
/// `.await` point.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// HTTP client that owns the per-session cookie jar. Every Beanfun
    /// call after login goes through this client so the `bfWebToken`
    /// cookie is sent automatically.
    pub client: BeanfunClient,

    /// Session identifiers minted by the login flow (region, `skey`,
    /// `web_token`, `account_id`, service code/region). Sensitive
    /// fields are redacted by the `Debug` impl — see
    /// [`Session`]'s module docs for the sensitivity policy.
    pub session: Session,
}

impl AuthContext {
    /// Bundle a freshly-minted [`BeanfunClient`] and [`Session`] into a
    /// single swap-able unit.
    ///
    /// Callers (login commands) should immediately `write()` the
    /// result into [`AppState::auth`] so downstream commands see it.
    pub fn new(client: BeanfunClient, session: Session) -> Self {
        Self { client, session }
    }
}

/// TOTP continuation waiting for a 6-digit code.
///
/// Stored as `Some(_)` on [`AppState::pending_totp`] after a
/// `login_hk_regular` call returns [`LoginError::TotpRequired`]
/// ([`crate::services::beanfun::LoginError::TotpRequired`]). The
/// backend holds onto the challenge (and its client, so the cookie
/// jar lineage is preserved) because [`TotpChallenge`] carries
/// server-side secrets (`session_key` / `viewstate`) that would
/// violate the P10.2 Q4=C "no secrets over IPC" contract if exposed
/// to the frontend.
///
/// The frontend observes the pending state only through a
/// [`CommandError`][crate::commands::error::CommandError] with code
/// `auth.totp_required` carrying a
/// [`TotpChallengeInfo`][crate::commands::dto::TotpChallengeInfo]
/// (safe subset: `totp_url` + `account_id`). The actual challenge
/// and client are consumed from the slot by `login_totp(code)`.
///
/// # Lifecycle
///
/// ```text
/// login_regular(region=HK, account, password)
///   ├─ Ok(session)                                → set auth = Some(..)
///   ├─ Err(LoginError::TotpRequired(challenge))   → set pending_totp = Some(..)
///   │                                                surface CommandError(auth.totp_required)
///   └─ Err(...)                                    → surface CommandError(...)
///
/// login_totp(code)
///   ├─ read pending_totp (clone client+challenge, keep slot populated for retry)
///   ├─ services::beanfun::login::login_totp(..) returns
///   │   ├─ Ok(session)         → pending_totp = None; set auth = Some(..)
///   │   └─ Err(..)             → slot untouched (user may retry with new code)
/// ```
///
/// Cancelling a pending TOTP (user closes the OTP prompt) happens
/// via the `logout` command (P10.2 D7), which clears both `auth` and
/// `pending_totp` in one go — P10.2 YAGNI on a separate
/// `cancel_totp` cmd (Todo.md L892 doesn't list it; a dedicated UX
/// affordance can be added in P11/P12 if the UX calls for it).
///
/// [`LoginError::TotpRequired`]: crate::services::beanfun::LoginError::TotpRequired
#[derive(Debug, Clone)]
pub struct PendingTotp {
    /// Same [`BeanfunClient`] that issued the credentialled POST —
    /// carries the accumulated login cookies so `login_totp` picks
    /// up exactly where `login_hk_regular` stopped. Cloning is
    /// cheap (Arc-based internals).
    pub client: BeanfunClient,
    /// Opaque continuation handle produced by
    /// [`crate::services::beanfun::login::login_hk_regular`]. Clone
    /// across the await boundary so the RwLock read guard can be
    /// dropped before the OTP POST fires.
    pub challenge: TotpChallenge,
}

impl PendingTotp {
    /// Bundle a client + challenge pair ready to be parked on
    /// [`AppState::pending_totp`].
    pub fn new(client: BeanfunClient, challenge: TotpChallenge) -> Self {
        Self { client, challenge }
    }
}

/// QR-login continuation waiting for the user to approve the scan in
/// the mobile app.
///
/// Stored as `Some(_)` on [`AppState::pending_qr`] after a
/// `login_qr_start` call succeeds. The backend holds onto both the
/// `BeanfunClient` (cookie jar continuity) and the
/// [`QrLoginInit`] (carries the `skey` + `verification_token`
/// needed by the poll / finalize calls) because:
///
/// - `skey` is the portal session key; treating it as a backend-only
///   secret avoids broadcasting it through every poll round-trip.
/// - `verification_token` is the antiforgery token the poll step
///   echoes back as a header — keeping it backend-side means the
///   Vue frontend never needs to re-transmit it.
///
/// The frontend observes the pending state indirectly by calling
/// `login_qr_check`, which returns a status DTO (`pending` /
/// `approved` / `expired` / `retry`) based on the server response.
/// On `approved`, the command internally runs `finalize_qr_login`
/// and populates [`AppState::auth`] in one go — the frontend never
/// sees the intermediate `QrLoginInit`.
///
/// # Lifecycle
///
/// ```text
/// login_qr_start
///   ├─ Ok(init) → clear old pending_qr & pending_totp,
///   │             set pending_qr = Some((client, init)),
///   │             return QrStart { bitmap_base64, deeplink }
///   └─ Err(..)   → surface CommandError(..)
///
/// login_qr_check
///   ├─ read pending_qr (clone client+init so poll can run outside lock)
///   ├─ poll_qr_login_status(..) returns
///   │   ├─ WaitLogin      → QrCheck::Pending (slot kept)
///   │   ├─ Failed         → QrCheck::Retry   (slot kept, transient)
///   │   ├─ TokenExpired   → QrCheck::Expired (slot cleared — frontend
///   │   │                                     must call login_qr_start again)
///   │   └─ Approved       → finalize_qr_login(..) → Session
///   │                       pending_qr = None
///   │                       auth = Some((client, session))
///   │                       QrCheck::Approved(SessionInfo)
/// ```
///
/// Cancelling a pending QR (user closes the QR dialog without
/// scanning) is handled by the D7 `logout` command — same single
/// cleanup lever as [`PendingTotp`].
#[derive(Debug, Clone)]
pub struct PendingQr {
    /// Same [`BeanfunClient`] that ran the QR init — the cookie jar
    /// carries the portal session cookies the poll + finalize POSTs
    /// bind to. Cloning is cheap (Arc-based internals).
    pub client: BeanfunClient,
    /// Bootstrap payload returned by
    /// [`crate::services::beanfun::login::init_qr_login`]; carries
    /// the `skey` + `verification_token` the poll / finalize calls
    /// need. Clone across `await` boundaries so the RwLock read
    /// guard can drop before network IO.
    pub init: QrLoginInit,
}

impl PendingQr {
    /// Bundle a client + init pair ready to be parked on
    /// [`AppState::pending_qr`].
    pub fn new(client: BeanfunClient, init: QrLoginInit) -> Self {
        Self { client, init }
    }
}

/// AdvanceCheck-verify continuation waiting for a user-supplied
/// captcha + auth code.
///
/// Stored as `Some(_)` on [`AppState::pending_verify`] after a
/// `get_verify_page_info` call succeeds. The backend holds both the
/// `BeanfunClient` (cookie continuity across the captcha GET + POST
/// round-trips) and the [`VerifyPageInfo`] payload because:
///
/// - `VerifyPageInfo` carries `__VIEWSTATE` / `__EVENTVALIDATION`
///   (ASP.NET server-side state) that must round-trip back on the
///   verify submit — same "no secrets over IPC" policy as
///   [`PendingTotp`] / [`PendingQr`].
/// - `samplecaptcha` is the captcha id; the backend uses it to fetch
///   the captcha image without requiring the frontend to pass it
///   back on every command.
///
/// # Lifecycle
///
/// ```text
/// get_verify_page_info(url)
///   ├─ Ok(info)  → set pending_verify = Some((client, info)),
///   │              return VerifyPage (display-only)
///   └─ Err(..)    → surface CommandError(..)
///
/// get_verify_captcha
///   ├─ read pending_verify (keep slot)
///   ├─ get_verify_captcha_service(.., samplecaptcha) → PNG bytes
///   └─ return data URL (base64)
///
/// submit_verify(code, captcha)
///   ├─ read pending_verify (keep slot in case of retry)
///   ├─ submit_verify_service(..) → VerifyOutcome
///   │   ├─ Success          → pending_verify = None; return Success
///   │   ├─ WrongCaptcha     → slot kept; return WrongCaptcha
///   │   ├─ WrongAuthInfo    → slot kept; return WrongAuthInfo
///   │   └─ ServerMessage    → slot kept; return ServerMessage(msg)
/// ```
///
/// # Why the verify client is separate from the login client
///
/// P10.2 Q6 = A: login surfaces `auth.advance_check_required` and
/// the frontend drives the verify flow in a **new** command chain
/// — this prevents the login command from holding a plaintext
/// password across the verify round-trips. Verify lives on its own
/// [`BeanfunClient`] (per-flow cookie jar); retrying the login
/// after verify completes mints yet another client (clean cookie
/// jar), which the server accepts because the AdvanceCheck pass
/// is tracked server-side by IP/device fingerprint, not by client
/// cookies.
#[derive(Debug, Clone)]
pub struct PendingVerify {
    /// [`BeanfunClient`] dedicated to the verify flow. Always TW
    /// endpoints because AdvanceCheck.aspx lives on the TW
    /// `newlogin` host regardless of the upstream login region
    /// (see `services::beanfun::verify` module docs).
    pub client: BeanfunClient,
    /// Parsed AdvanceCheck page — the viewstate bundle + captcha
    /// id + form action the subsequent captcha / submit POSTs
    /// need. Clone-friendly (all `String`s, cheap).
    pub page_info: VerifyPageInfo,
}

impl PendingVerify {
    /// Bundle a client + page_info pair ready to be parked on
    /// [`AppState::pending_verify`].
    pub fn new(client: BeanfunClient, page_info: VerifyPageInfo) -> Self {
        Self { client, page_info }
    }
}

/// GamePass-login continuation waiting for the WebView window to
/// drive its OAuth-style flow to completion.
///
/// Stored as `Some(_)` on [`AppState::pending_gamepass`] after a
/// `login_gamepass_start` call succeeds. The backend holds onto
/// both the `BeanfunClient` (cookie jar continuity — the GamePass
/// flow re-uses the same portal session cookies the WebView will
/// see) and the `skey` (passed to the WebView URL as `pSKey={skey}`
/// to bind the GamePass login attempt to the correct portal
/// session) because:
///
/// - `skey` is the portal session key; treating it as backend-only
///   prevents the frontend from having to thread it through every
///   subsequent open-window / complete IPC, and matches the
///   `pending_qr` / `pending_totp` "no secrets over IPC" stance
///   (P10.2 Q4=C).
/// - `BeanfunClient` carries the cookie jar that the WebView's
///   pre-injection step will mirror (the WPF reference at
///   `Beanfun\Windows\GamePassBrowser.xaml.cs` L99-104 copies every
///   cookie from `bfClient.cookieContainer` into the WebView2
///   profile before navigating; we mirror that with
///   `Webview::set_cookie` in P12.1 D5b CP3).
///
/// # Lifecycle
///
/// ```text
/// login_gamepass_start(region)
///   ├─ Reject HK with auth.gamepass_unsupported_region (TW-only;
///   │   WPF MainWindow.xaml.cs::loginMethodInit hides the GamePass
///   │   button under HK at L1099-1114).
///   ├─ clear pending_totp / pending_qr / pending_gamepass
///   ├─ mint fresh BeanfunClient (TW endpoints)
///   ├─ get_session_key(client) → skey
///   ├─ pending_gamepass = Some((client, skey))
///   └─ Ok(())   (skey stays backend-internal)
///
/// open_gamepass_window         (P12.1 D5b CP3)
///   ├─ read pending_gamepass (clone client + skey)
///   ├─ build WebviewWindow at login.beanfun.com URL with pSKey
///   ├─ inject every cookie from client.cookie_store() into the
///   │   webview cookie store via Webview::set_cookie
///   ├─ on_navigation hook: every URL change → read
///   │   webview cookies for the 3 portal domains; if bfWebToken
///   │   appears, run complete_gamepass_login(..) inline:
///   │     ├─ Ok(session)  → pending_gamepass = None;
///   │     │                 set auth = Some((client, session));
///   │     │                 emit gamepass-login-success { session };
///   │     │                 close window
///   │     └─ Err(e)       → pending_gamepass = None;
///   │                       emit gamepass-login-failed { error };
///   │                       close window
///   └─ window-close (no token yet) = user cancel = clear
///       pending_gamepass; no event emitted
/// ```
///
/// Cancelling a pending GamePass (user closes the WebView before
/// scanning / approving) is handled by the D7 `logout` command and
/// the on-close hook above — same single-cleanup-lever stance as
/// [`PendingTotp`] / [`PendingQr`].
#[derive(Debug, Clone)]
pub struct PendingGamepass {
    /// Same [`BeanfunClient`] that ran the GamePass init — its
    /// cookie jar is the source of truth for the WebView cookie
    /// pre-injection step in `open_gamepass_window`. Cloning is
    /// cheap (Arc-based internals).
    pub client: BeanfunClient,
    /// Portal session key returned by
    /// [`crate::services::beanfun::login::get_session_key`]. The
    /// WebView URL uses this as the `pSKey` query param so the
    /// GamePass login attempt binds back to the same portal session
    /// the cookie jar represents. Clone-friendly (`String`).
    pub skey: String,
}

impl PendingGamepass {
    /// Bundle a client + skey pair ready to be parked on
    /// [`AppState::pending_gamepass`].
    pub fn new(client: BeanfunClient, skey: impl Into<String>) -> Self {
        Self {
            client,
            skey: skey.into(),
        }
    }
}

/// Shared application state injected into every Tauri command.
///
/// See the [module-level documentation][self] for the lifecycle and
/// expansion plan.
pub struct AppState {
    /// Root directory for every on-disk artifact (Users.dat,
    /// Config.xml, update cache, logs). Typically `%APPDATA%\Beanfun`
    /// in production; a `tempfile::TempDir` path in tests.
    pub storage_root: PathBuf,

    /// Current authenticated Beanfun session + its HTTP client.
    /// `None` at startup; populated by the login commands (P10.2) and
    /// cleared on `logout` or expiry.
    ///
    /// Uses [`tokio::sync::RwLock`] — guards are `Send` so they
    /// survive `.await` points inside async command bodies, unlike
    /// [`std::sync::RwLock`] which poisons the `!Send` `Guard`.
    ///
    /// Multiple concurrent readers are the common case (every "is the
    /// user logged in?" check takes a read lock); writers (login /
    /// logout) are rare and exclusive.
    pub auth: RwLock<Option<AuthContext>>,

    /// Backend-held TOTP continuation — see [`PendingTotp`] for the
    /// full lifecycle. `None` whenever no login is awaiting a
    /// 6-digit OTP response. Uses its own [`RwLock`] rather than
    /// being folded into [`AuthContext`] because:
    ///
    /// - `auth` and `pending_totp` are **mutually exclusive by
    ///   design** (a successful login clears the pending slot; an
    ///   incomplete login hasn't populated `auth` yet), so a single
    ///   combined lock would prevent no real race.
    /// - The two slots have different readers. `auth` is read by
    ///   every downstream command; `pending_totp` is read only by
    ///   `login_totp`. Keeping them separate avoids a surface-level
    ///   command stalling the rare OTP POST.
    pub pending_totp: RwLock<Option<PendingTotp>>,

    /// Backend-held QR-login continuation — see [`PendingQr`] for the
    /// full lifecycle. `None` whenever no QR challenge is active
    /// (fresh process / post-`logout` / post-finalize).
    ///
    /// Sibling slot to `pending_totp`: both represent the same
    /// "half-finished login" concept but for different flows, and
    /// `login_qr_start` / `login_regular` clear **the other** at
    /// their top to guarantee only one continuation is outstanding
    /// at a time.
    pub pending_qr: RwLock<Option<PendingQr>>,

    /// Backend-held AdvanceCheck-verify continuation — see
    /// [`PendingVerify`] for the full lifecycle. `None` whenever no
    /// verify flow is active.
    ///
    /// Unlike `pending_totp` / `pending_qr`, this slot is
    /// **orthogonal** to login: a verify flow is kicked off by the
    /// frontend *after* a login attempt surfaced
    /// `auth.advance_check_required`, and the backend does not
    /// retry the login automatically — the frontend re-runs
    /// `login_regular` itself once `submit_verify` returns
    /// `Success`. This means `pending_verify` can legitimately
    /// coexist with a stale `pending_totp` or `pending_qr`;
    /// `logout` (D7) clears all three in one swoop.
    pub pending_verify: RwLock<Option<PendingVerify>>,

    /// Backend-held GamePass-login continuation — see
    /// [`PendingGamepass`] for the full lifecycle. `None` whenever
    /// no GamePass WebView session is active (fresh process /
    /// post-`logout` / post-complete / user-cancel).
    ///
    /// Sibling slot to `pending_totp` / `pending_qr`: the three
    /// represent the same "half-finished login" concept across
    /// different flows, and `login_gamepass_start` /
    /// `login_qr_start` / `login_regular` clear **the others** at
    /// their top to guarantee at most one continuation is
    /// outstanding at a time.
    pub pending_gamepass: RwLock<Option<PendingGamepass>>,
}

impl AppState {
    /// Build an [`AppState`] rooted at `storage_root`.
    ///
    /// Currently infallible — `AppState` owns no resource whose
    /// initialization can fail. The HTTP client is not created here
    /// because it lives **inside** [`AuthContext`] and is minted per
    /// login (each login mints a fresh cookie jar, so re-login from a
    /// clean state is the single source of truth).
    pub fn new(storage_root: PathBuf) -> Self {
        Self {
            storage_root,
            auth: RwLock::new(None),
            pending_totp: RwLock::new(None),
            pending_qr: RwLock::new(None),
            pending_verify: RwLock::new(None),
            pending_gamepass: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::beanfun::client::{ClientConfig, LoginRegion};

    fn sample_auth_context() -> AuthContext {
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

    #[test]
    fn new_stores_storage_root_verbatim() {
        let root = PathBuf::from(r"C:\tmp\beanfun-test");
        let state = AppState::new(root.clone());
        assert_eq!(state.storage_root, root);
    }

    #[tokio::test]
    async fn auth_starts_as_none() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        let guard = state.auth.read().await;
        assert!(guard.is_none(), "auth must be None before login");
    }

    #[tokio::test]
    async fn auth_can_be_populated_then_cleared() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));

        {
            let mut guard = state.auth.write().await;
            *guard = Some(sample_auth_context());
        }

        {
            let guard = state.auth.read().await;
            let ctx = guard.as_ref().expect("auth populated");
            assert_eq!(ctx.session.account_id, "alice");
            assert_eq!(ctx.session.region, LoginRegion::TW);
            assert_eq!(ctx.client.config().region, LoginRegion::TW);
        }

        {
            let mut guard = state.auth.write().await;
            *guard = None;
        }

        assert!(state.auth.read().await.is_none());
    }

    /// Asserts that `Option::take` swaps `client` and `session` out in
    /// a single lock acquisition — there is no intermediate state in
    /// which one is cleared before the other.
    #[tokio::test]
    async fn auth_take_atomically_clears_both_fields() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        {
            let mut guard = state.auth.write().await;
            *guard = Some(sample_auth_context());
        }

        let taken = {
            let mut guard = state.auth.write().await;
            guard.take()
        };

        assert!(taken.is_some(), "take() returns the previous value");
        assert!(
            state.auth.read().await.is_none(),
            "auth is None after take(), regardless of what the caller does with the taken value",
        );
    }

    /// `AppState::new` must zero-init every pending slot. Covered
    /// specifically because P10.2 added multiple pending slots
    /// (D4 added `pending_totp`; D5 added `pending_qr`; P12.1 D5
    /// added `pending_gamepass`); keeping a separate assertion per
    /// slot makes a future slot's absent initialization a
    /// localized failure.
    #[tokio::test]
    async fn pending_slots_start_as_none() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        assert!(
            state.pending_totp.read().await.is_none(),
            "pending_totp must be None before any TOTP-bearing login attempt",
        );
        assert!(
            state.pending_qr.read().await.is_none(),
            "pending_qr must be None before any QR login attempt",
        );
        assert!(
            state.pending_verify.read().await.is_none(),
            "pending_verify must be None before any AdvanceCheck flow",
        );
        assert!(
            state.pending_gamepass.read().await.is_none(),
            "pending_gamepass must be None before any GamePass login attempt",
        );
    }

    #[tokio::test]
    async fn pending_gamepass_can_be_populated_then_taken() {
        let state = AppState::new(PathBuf::from(r"C:\tmp"));
        let client = BeanfunClient::new(ClientConfig::default()).expect("client builds");
        let pending = PendingGamepass::new(client, "SKEY_GP_TEST");

        {
            let mut guard = state.pending_gamepass.write().await;
            *guard = Some(pending);
        }

        let taken = {
            let mut guard = state.pending_gamepass.write().await;
            guard.take()
        };

        let pending = taken.expect("populated value returned by take()");
        assert_eq!(pending.skey, "SKEY_GP_TEST");
        assert_eq!(pending.client.config().region, LoginRegion::TW);
        assert!(state.pending_gamepass.read().await.is_none());
    }
}
