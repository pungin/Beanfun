//! Account commands — service-account management for the logged-in
//! Beanfun session.
//!
//! # Families exposed in P10.2
//!
//! | Command                 | Family       | Purpose                                                          |
//! |-------------------------|--------------|------------------------------------------------------------------|
//! | [`get_accounts`]        | base         | Fetch the sorted service-account list + quota notice             |
//! | [`refresh`]             | base         | Semantic alias — re-runs the same flow as [`get_accounts`]       |
//! | `add_service_account`   | management   | (D9) Add a connected-game service account                       |
//! | `change_display_name`   | management   | (D9) Rename a service account                                    |
//! | `get_contract`          | info         | (D10) Fetch service contract URL                                |
//! | `get_email`             | info         | (D10) Fetch account email                                       |
//! | `get_remain_point`      | info         | (D10) Fetch remaining Beanfun points                            |
//!
//! The `unconnected_game_*` family (unconnected-game flows) is
//! **deferred to P12** — they're UI-driven (captcha prompt, display
//! name picker, password change wizard) and their command shape
//! depends on the Vue UX (P10.2 pre-flight Q7 = A).
//!
//! # Session gating
//!
//! Every command in this module is **session-required** — they
//! start by calling [`commands::session::require_auth`] which
//! surfaces `auth.session_required` when no login is active. The
//! shared [`list_accounts_internal`] helper below centralises the
//! auth check + service dispatch so `get_accounts` and `refresh`
//! stay a single line apiece.
//!
//! # DTO policy (P10.2 Q4 = C)
//!
//! [`ServiceAccount`], [`AccountListResult`], and
//! [`AmountLimitNotice`][crate::services::beanfun::AmountLimitNotice]
//! are **pure data types** — no secrets, no binary blobs — so they
//! derive `serde::Serialize + specta::Type` directly on the
//! service-layer struct/enum (not a shadow DTO). The command layer
//! returns them by value and `tauri-specta` emits a matching
//! TypeScript type into `bindings.ts`. Field names are WPF-verbatim
//! (`sid` / `ssn` / `sname` / …) because keeping the Rust ↔ WPF
//! mapping 1:1 outweighs the TypeScript style win of `camelCase`.
//!
//! [`commands::session::require_auth`]: crate::commands::session::require_auth

use tauri::State;

use crate::commands::{error::CommandError, session::require_auth, state::AppState};
use crate::services::beanfun::{
    add_service_account as service_add_service_account,
    change_service_account_display_name as service_change_display_name,
    get_accounts as service_get_accounts, get_email as service_get_email,
    get_remain_point as service_get_remain_point,
    get_service_contract as service_get_service_contract, AccountListResult, ServiceAccount,
};

/// Internal helper shared by [`get_accounts`] and [`refresh`].
///
/// Unwinds the auth check + service-layer dispatch so each public
/// command body collapses to a one-liner — single source of truth
/// for "how do we fetch the current session's account list?". If
/// a future tweak to the flow is needed (e.g. bypass cache, force
/// cookie refresh, swap service provider), this is the only place
/// the change lands.
///
/// # Why `require_auth` clones the client + session
///
/// The helper returns owned [`BeanfunClient`][bc] + [`Session`][sesh]
/// values so the [`AppState::auth`] read guard can drop before the
/// HTTP `get_accounts` call begins. Holding a guard across `.await`
/// would block the `logout` command from acquiring its write guard
/// concurrently.
///
/// [bc]: crate::services::beanfun::BeanfunClient
/// [sesh]: crate::services::beanfun::Session
async fn list_accounts_internal(state: &AppState) -> Result<AccountListResult, CommandError> {
    let (client, session) = require_auth(state).await?;
    let result = service_get_accounts(
        &client,
        &session,
        &session.service_code,
        &session.service_region,
    )
    .await?;
    Ok(result)
}

/// List the service accounts the logged-in user can launch into the
/// session's current service + region.
///
/// # Returns
///
/// An [`AccountListResult`] bundle with:
///
/// - `accounts` — sorted by ascending `ssn` (WPF first-pass sort).
/// - `amount_limit_notice` — typed quota-notice classification
///   (`None` / `AuthReLoginRequired` / `Other { message }`).
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Every [`LoginError`][le] surfaced by the service-layer
///   `get_accounts` (transport / parse / body-too-large). The
///   P10.1 `From<LoginError>` impl handles mapping verbatim.
///
/// # Frontend usage
///
/// Called on first render of the account-picker screen. See
/// [`refresh`] for the UI's "reload" affordance.
///
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<AccountListResult, CommandError> {
    list_accounts_internal(state.inner()).await
}

/// Semantic alias for [`get_accounts`] — re-fetch the account list.
///
/// # Why a second command instead of just `get_accounts`?
///
/// Two separate commands let the frontend's intent be legible at the
/// call site (`invoke('get_accounts')` on first render vs.
/// `invoke('refresh')` on the reload button) without the backend
/// diverging behaviour. A future requirement (analytics counter,
/// stricter rate limit, cache bypass) can land in one `#[tauri::command]`
/// body without touching the other's contract.
///
/// # Implementation
///
/// Delegates to the same [`list_accounts_internal`] helper as
/// [`get_accounts`] — both commands are pure wire adapters on top
/// of the single internal primitive, so there is no duplicated
/// flow logic to keep in sync (DRY).
///
/// # When to call
///
/// On user-initiated "refresh" button clicks, and after commands
/// that invalidate the list (e.g. `add_service_account`,
/// `change_display_name` — both in D9).
#[tauri::command]
#[specta::specta]
pub async fn refresh(state: State<'_, AppState>) -> Result<AccountListResult, CommandError> {
    list_accounts_internal(state.inner()).await
}

/// Add a new service account (character slot) for the logged-in user
/// under the session's current service + region.
///
/// # Contract
///
/// Mirrors [`services::beanfun::add_service_account`][svc] verbatim:
///
/// - Empty `name` → `Ok(false)` *without firing a request* (server
///   roundtrip is redundant — the form validation on the WPF dialog
///   gates the same way, so we preserve both the UI semantic and the
///   zero-network-cost shape).
/// - Non-empty → `POST gamezone.ashx` with
///   `strFunction=AddServiceAccount`; response's `intResult == 1` →
///   `true`, anything else (including empty body or missing field) →
///   `false`.
///
/// # Why pull `service_code` / `service_region` from the session?
///
/// WPF's `MainWindow.AddServiceAccount` (`Beanfun/MainWindow.xaml.cs`)
/// uses the same globals — the add-account dialog only ever targets
/// the user's current game. Exposing the two fields as IPC parameters
/// would invite the frontend to pass mismatched values (e.g. a stale
/// account-list snapshot from before the region switched), so we lock
/// the source of truth to [`Session`][sesh] on the backend.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service (`auth.aspx`
///   pre-flight / `gamezone.ashx` transport / JSON parse / body-too-
///   large). Already mapped to `CommandError` by the P10.1
///   `From<LoginError>` impl.
///
/// # Frontend usage
///
/// After a successful return, the caller should invoke [`refresh`]
/// to pick up the new row (gamezone does not echo the account back).
///
/// [svc]: crate::services::beanfun::add_service_account
/// [sesh]: crate::services::beanfun::Session
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn add_service_account(
    state: State<'_, AppState>,
    name: String,
) -> Result<bool, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let accepted = service_add_service_account(
        &client,
        &session,
        &name,
        &session.service_code,
        &session.service_region,
    )
    .await?;
    Ok(accepted)
}

/// Rename an existing service account's display name.
///
/// # Contract
///
/// Mirrors [`services::beanfun::change_service_account_display_name`][svc]
/// verbatim:
///
/// - `new_name.is_empty()` **or** `new_name == account.sname` →
///   `Ok(false)` without firing a request (WPF early-out — server
///   would reject identical names anyway, so we skip the roundtrip).
/// - Otherwise → `POST gamezone.ashx` with
///   `strFunction=ChangeServiceAccountDisplayName, sl=<game_code>,
///   said=<account.sid>, nsadn=<new_name>`; response's
///   `intResult == 1` → `true`, anything else → `false`.
///
/// # Why echo the whole `ServiceAccount` from the frontend?
///
/// The service layer mirrors WPF's signature (which takes the whole
/// `ServiceAccount` so the call site can early-out on
/// `newName == account.sname`). Rather than reshape the service
/// call or build a partially-populated `ServiceAccount` in the
/// command layer (which would require manually updating every time
/// the struct gains a new field), we let the frontend echo the
/// object it already has in hand from [`get_accounts`]. `ServiceAccount`
/// contains only display-oriented public fields (no secrets), so
/// the echo round-trip is safe — which is why it derives
/// `serde::Deserialize` alongside `Serialize + specta::Type`.
///
/// # Why pull `game_code` from the session?
///
/// `game_code = "{service_code}_{service_region}"` — constructed on
/// the backend to prevent the frontend from drifting the two halves
/// against each other (exactly as [`add_service_account`] locks
/// down the service code / region split).
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service.
///
/// # Frontend usage
///
/// On `Ok(true)`, the caller should update its local `ServiceAccount`
/// (`sname = new_name`) or invoke [`refresh`]. On `Ok(false)` — either
/// the caller passed an invalid / unchanged name (expected UI
/// prevention), or the server rejected the change (show a generic
/// "could not rename" message — mirrors WPF's `MsgChangeDisplayNameError`).
///
/// [svc]: crate::services::beanfun::change_service_account_display_name
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn change_display_name(
    state: State<'_, AppState>,
    new_name: String,
    account: ServiceAccount,
) -> Result<bool, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let game_code = format!("{}_{}", session.service_code, session.service_region);
    let accepted =
        service_change_display_name(&client, &session, &new_name, &game_code, &account).await?;
    Ok(accepted)
}

/// Fetch the EULA / service contract HTML for the session's current
/// service + region.
///
/// # Contract
///
/// Thin wrapper over [`services::beanfun::get_service_contract`][svc].
/// Same `service_code` / `service_region` policy as
/// [`add_service_account`] — pulled from [`Session`][sesh] so the
/// frontend cannot drift the two halves against each other.
///
/// Returns the raw HTML fragment the server emits in the
/// `strResult` JSON field (or `""` when `intResult != 1` / the body
/// is empty — matching WPF).
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service (transport,
///   JSON parse, body-too-large).
///
/// # Frontend usage
///
/// The UI renders the returned HTML inside the "service contract"
/// dialog (matching WPF's `Contract.xaml`). We return the body
/// verbatim so the frontend's XSS policy — a dedicated render
/// component with a sanitizer — owns the sanitisation decision;
/// applying a sanitiser here would hard-code one policy for every
/// consumer.
///
/// [svc]: crate::services::beanfun::get_service_contract
/// [sesh]: crate::services::beanfun::Session
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn get_contract(state: State<'_, AppState>) -> Result<String, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let contract = service_get_service_contract(
        &client,
        &session,
        &session.service_code,
        &session.service_region,
    )
    .await?;
    Ok(contract)
}

/// Fetch the logged-in user's e-mail address.
///
/// # Contract
///
/// Thin wrapper over [`services::beanfun::get_email`][svc]. TW
/// sessions return the captured address; HK sessions short-circuit
/// to `""` **without** firing a request (the HK portal does not
/// expose this endpoint — mirrors WPF `BeanfunClient.cs::getEmail`
/// L245-246).
///
/// Returns the e-mail string, or `""` when the TW regex does not
/// match / the session is HK.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service (transport,
///   body-too-large).
///
/// # Frontend usage
///
/// The AccountList "view e-mail" affordance hides itself when the
/// return is empty (matches WPF's `AccountList.xaml.cs`
/// `m_GetEmail_Click` behaviour — nothing is shown for empty).
///
/// [svc]: crate::services::beanfun::get_email
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn get_email(state: State<'_, AppState>) -> Result<String, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let email = service_get_email(&client, &session).await?;
    Ok(email)
}

/// Fetch the remaining Beanfun points balance.
///
/// # Contract
///
/// Thin wrapper over [`services::beanfun::get_remain_point`][svc].
/// Returns an `i32` for drop-in parity with WPF's `int` return
/// (`BeanfunClient.cs::getRemainPoint` L214).
///
/// Returns `0` when the server response does not match the
/// `"RemainPoint" : "…"` regex **or** the captured value is not a
/// valid `i32` — matches WPF's blanket `catch { return 0; }`.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Any [`LoginError`][le] surfaced by the service. (The WPF
///   `catch` would swallow these as `0`; we propagate so the
///   frontend can distinguish "server rejected" from "network
///   down" — the UI can apply the `→ 0` rule locally if strict
///   WPF parity is desired.)
///
/// # Frontend usage
///
/// The AccountList header surfaces this as the "剩餘 B$" ticker
/// (matches WPF `AccountList.xaml.cs` L139 → `updateRemainPoint`).
///
/// [svc]: crate::services::beanfun::get_remain_point
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn get_remain_point(state: State<'_, AppState>) -> Result<i32, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let pts = service_get_remain_point(&client, &session).await?;
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::session::SESSION_REQUIRED_CODE;
    use std::path::PathBuf;

    fn empty_state() -> AppState {
        AppState::new(PathBuf::from(r"C:\tmp"))
    }

    /// When [`AppState::auth`] is `None`, the shared helper must
    /// short-circuit with `auth.session_required`. Asserted on the
    /// helper (not the commands) so both `get_accounts` and
    /// `refresh` inherit the behaviour through their one-liner
    /// delegation — the test is their joint contract.
    #[tokio::test]
    async fn list_accounts_internal_without_session_surfaces_session_required() {
        let app = empty_state();
        let err = list_accounts_internal(&app)
            .await
            .expect_err("no session → error");

        assert_eq!(err.code, SESSION_REQUIRED_CODE);
    }

    /// All four P10.2 account-family commands must exist with their
    /// declared signatures. The `_ = <fn>` pattern is a readable
    /// way to force a symbol reference without invoking the
    /// `State<'_, _>`-requiring body (Tauri's `State` wrapper can't
    /// be constructed outside `tauri::test::mock_app()` — which we
    /// deliberately avoid per the auth-module convention).
    /// Session-gating is validated through
    /// [`list_accounts_internal_without_session_surfaces_session_required`]
    /// and the `require_auth` tests in [`super::super::session`];
    /// D9 management commands inherit the same behaviour via the
    /// shared `require_auth` call.
    #[test]
    fn account_commands_exist_with_declared_signatures() {
        let _ = get_accounts;
        let _ = refresh;
        let _ = add_service_account;
        let _ = change_display_name;
        let _ = get_contract;
        let _ = get_email;
        let _ = get_remain_point;
    }

    /// `ServiceAccount` must be **round-trippable** through serde
    /// so the frontend can echo the object it got from
    /// [`get_accounts`] back to [`change_display_name`]. Full-field
    /// equality here guards against a future `#[serde(skip)]` or
    /// rename slipping in and silently dropping data the service
    /// layer depends on (`sid` / `sname`) — the rename flow would
    /// break silently on the transport boundary otherwise.
    #[test]
    fn service_account_serde_roundtrip_preserves_all_fields() {
        let original = sample_service_account();
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: ServiceAccount = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    fn sample_service_account() -> ServiceAccount {
        ServiceAccount {
            is_enable: true,
            visible: true,
            is_inherited: false,
            sid: "sid_test".into(),
            ssn: "42".into(),
            sname: "AliceTheFirst".into(),
            screatetime: Some("2024-01-02 03:04:05".into()),
            slastusedtime: None,
            sauthtype: None,
        }
    }
}
