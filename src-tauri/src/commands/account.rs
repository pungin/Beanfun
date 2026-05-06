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
    get_service_contract as service_get_service_contract,
    unconnected_game_add_account as service_unconnected_game_add_account,
    unconnected_game_add_account_check as service_unconnected_game_add_account_check,
    unconnected_game_add_account_check_nickname as service_unconnected_game_add_account_check_nickname,
    unconnected_game_change_password as service_unconnected_game_change_password,
    unconnected_game_init_add_account_payload as service_unconnected_game_init_add_account_payload,
    AccountListResult, AddAccountInit, AddAccountOutcome, AddAccountSession, ChangePasswordOutcome,
    CheckOutcome, ServiceAccount,
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

/// Update the **active service code / region** on the live
/// [`Session`][sn] so subsequent session-gated commands target the
/// game the user just picked from `windows/GameList.vue`.
///
/// # Why a backend command (not a frontend-only Pinia mutation)?
///
/// Every other account-family command (`get_accounts` /
/// `add_service_account` / `change_display_name` /
/// `unconnected_game_*` / `get_contract` / etc.) deliberately pulls
/// `service_code` / `service_region` off the in-memory
/// [`Session`][sn] inside the command body — the IPC contract
/// **does not** accept the pair as a parameter, on the
/// "single-source-of-truth" rationale documented in
/// [`add_service_account`]'s docblock. That contract works for the
/// post-login first paint (the login flow seeds the session with the
/// region's default game) but breaks the moment the user picks a
/// different game from the picker dialog: the frontend's
/// `useGameStore.selectedGameCode` updates, but the backend session
/// keeps pointing at the original game, and the next
/// `get_accounts` / `add_service_account` round-trip silently
/// queries the wrong service.
///
/// WPF avoided this entirely because `MainWindow.service_code`
/// **is** the source of truth — there's no separate per-call
/// session struct, the field is mutated in place by
/// `MainWindow.GameList.SelectionChanged` (mirrored at L661 / L520
/// / L523 of `MainWindow.xaml.cs`) before the next
/// `bfClient.GetAccounts(service_code, service_region)` runs (L638
/// of the same file). Re-introducing that mutability on the SPA
/// backend keeps the IPC contract intact while preserving WPF's
/// "switching games re-targets every subsequent service call"
/// semantic.
///
/// # Contract
///
/// - Acquires the [`AppState::auth`] **write** lock (rare path —
///   only fires on game-picker confirmation, and the `tokio::sync::RwLock`
///   queues writers behind any in-flight readers from concurrent
///   commands).
/// - Returns `auth.session_required` when no session is active
///   (the picker is gated behind the AccountList route which
///   itself is auth-required, but the defensive guard keeps the
///   contract honest).
/// - **Stateless on the wire**: empty input is rejected by the
///   service-layer parser at the next `get_accounts` call, not
///   here — this command is a pure swap and has no policy on
///   what counts as a valid `(code, region)` pair (mirrors WPF's
///   "any string the picker dialog supplies is fine; let the
///   gamezone POST surface a server-side error if it's bogus"
///   stance).
///
/// # Frontend usage
///
/// Called from `pages/AccountList.vue::onGameChanged` (P12.3 D8)
/// inside the `<GameList @select>` handler, **before** the
/// follow-up `useAccountStore.refresh()` that paints the new
/// game's account list. Persisting `loginGame` to `Config.xml`
/// (so the choice survives a re-login) is a separate concern
/// owned by the frontend (`useConfigStore.set('loginGame', ...)`)
/// because Config.xml is a frontend-mediated cache (see
/// `useConfigStore` docblock).
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
///
/// [sn]: crate::services::beanfun::Session
#[tauri::command]
#[specta::specta]
pub async fn set_active_service(
    state: State<'_, AppState>,
    service_code: String,
    service_region: String,
) -> Result<(), CommandError> {
    set_active_service_internal(state.inner(), service_code, service_region).await
}

/// Tauri-State-free body of [`set_active_service`].
///
/// Split out from the `#[tauri::command]` wrapper for the same reason
/// [`list_accounts_internal`] is — `tauri::State<'_, _>` cannot be
/// constructed outside of `tauri::test::mock_app()`, so the unit
/// tests in [`tests`] target this helper directly. The wrapper above
/// is a one-line `state.inner()` adapter (no business logic), so a
/// behavioural test on the helper covers the whole command surface
/// without dragging the Tauri test runtime (and its WebView2
/// dependency) into the test binary.
async fn set_active_service_internal(
    state: &AppState,
    service_code: String,
    service_region: String,
) -> Result<(), CommandError> {
    let mut guard = state.auth.write().await;
    match guard.as_mut() {
        Some(ctx) => {
            ctx.session.service_code = service_code;
            ctx.session.service_region = service_region;
            Ok(())
        }
        None => Err(CommandError::new(
            crate::commands::session::SESSION_REQUIRED_CODE,
            crate::commands::session::SESSION_REQUIRED_MESSAGE,
        )),
    }
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

// =============================================================================
// P12.3 D3 — Unconnected-game account management
// =============================================================================
//
// These five commands surface the 5-step "unconnected game" account
// management flow that WPF wires into
// `Windows/UnconnectedGame_AddAccount.xaml.cs` and
// `Windows/UnconnectedGame_ChangePassword.xaml.cs`. The service-layer
// functions in [`crate::services::beanfun::account`] mirror WPF's
// `BeanfunClient.Account.cs::UnconnectedGame_*` family one-for-one;
// the command bodies below are zero-logic adapters whose only jobs
// are:
//
// 1. Gate on `require_auth` (every step requires the bfWebToken
//    cookie to be live in the client jar).
// 2. Pull `service_code` / `service_region` off the active
//    [`Session`][sn] for the steps that need it (init payload +
//    change-password). The frontend never passes these as IPC
//    parameters — same contract as `add_service_account` /
//    `change_display_name` (see those commands' docs for the
//    "single source of truth" rationale).
// 3. Forward the typed [`AddAccountSession`] / [`AddAccountInit`] /
//    [`CheckOutcome`] / [`AddAccountOutcome`] /
//    [`ChangePasswordOutcome`] DTOs verbatim — all five derive
//    `serde::Serialize` (and [`AddAccountSession`] additionally
//    derives `serde::Deserialize` because the frontend round-trips
//    it through three POSTs as an opaque cursor) + `specta::Type`
//    so `bindings.ts` mirrors the Rust contract.
//
// The frontend dialogs (`UnconnectedGame_AddAccount.vue` /
// `UnconnectedGame_ChangePassword.vue`, P12.3 D6 / D7) own all
// validation + UX wiring. WPF's pre-flight client-side checks
// (empty name, length range, password mismatch, contract
// checkbox) belong on the frontend; the service layer's defensive
// `LoginError::Unknown(_)` returns for empty inputs are a backstop,
// not the primary validation surface.
//
// [sn]: crate::services::beanfun::Session

/// Open the unconnected-game add-account dialog session — runs the
/// auth.aspx → 02.aspx GET / POST pair to seed cookies + parse the
/// initial view-state triplet, returning game name + account-id
/// length range + nickname-check support flag.
///
/// # Contract
///
/// Mirrors [`services::beanfun::unconnected_game_init_add_account_payload`][svc]
/// verbatim. Pulls `service_code` / `service_region` from the active
/// [`Session`][sn] (same lock-down as [`add_service_account`] — the
/// dialog only ever targets the user's currently selected
/// unconnected game).
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Every [`LoginError`][le] surfaced by the service
///   (`AccountMgmtMissingViewState` /
///   `AccountMgmtMissingViewStateGenerator` /
///   `AccountMgmtMissingEventValidation` /
///   `AccountMgmtMissingGameName` /
///   `AccountMgmtMissingAccountLen` from the parser, plus
///   transport / non-2xx / body-too-large from the HTTP layer).
///
/// # Frontend usage
///
/// Called once on `UnconnectedGame_AddAccount.vue` mount. The
/// returned [`AddAccountInit::session`] is stashed in component
/// state and threaded through every subsequent
/// [`unconnected_game_add_account_check`] /
/// [`unconnected_game_add_account_check_nickname`] /
/// [`unconnected_game_add_account`] call — the frontend treats it
/// as an opaque cursor (no field inspection).
///
/// [svc]: crate::services::beanfun::unconnected_game_init_add_account_payload
/// [sn]: crate::services::beanfun::Session
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn unconnected_game_init_add_account_payload(
    state: State<'_, AppState>,
) -> Result<AddAccountInit, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let init = service_unconnected_game_init_add_account_payload(
        &client,
        &session,
        &session.service_code,
        &session.service_region,
    )
    .await?;
    Ok(init)
}

/// Validate a candidate account-id (and optional display name)
/// before final submission — POST `02.aspx` with
/// `__EVENTTARGET=lbtnCheckAccount`.
///
/// # Contract
///
/// Mirrors [`services::beanfun::unconnected_game_add_account_check`][svc]
/// verbatim:
///
/// - `mgmt_session` is the round-tripped [`AddAccountSession`] from
///   the previous call (`init_add_account_payload` for the first
///   check, or the prior `CheckOutcome.session` for follow-ups).
/// - `name` is the candidate account id.
/// - `account_dn` is the optional display-name field — `Some("")` /
///   `Some(non_empty)` opt into the `t1` (TW) / `txtServiceAccountDN`
///   (HK) form field; `None` skips it entirely (matches WPF's
///   `txtServiceAccountDN != null` gate).
///
/// Returns a [`CheckOutcome`] carrying the refreshed view-state
/// triplet plus the optional `lblErrorMessage` text.
///
/// # Errors
///
/// As for [`unconnected_game_init_add_account_payload`].
///
/// [svc]: crate::services::beanfun::unconnected_game_add_account_check
#[tauri::command]
#[specta::specta]
pub async fn unconnected_game_add_account_check(
    state: State<'_, AppState>,
    mgmt_session: AddAccountSession,
    name: String,
    account_dn: Option<String>,
) -> Result<CheckOutcome, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let outcome = service_unconnected_game_add_account_check(
        &client,
        &session,
        &mgmt_session,
        &name,
        account_dn.as_deref(),
    )
    .await?;
    Ok(outcome)
}

/// Validate a candidate display name before final submission —
/// POST `02.aspx` with `__EVENTTARGET=lbtnCheckNickName` (the
/// account-id field is sent empty for this endpoint).
///
/// # Contract
///
/// Mirrors [`services::beanfun::unconnected_game_add_account_check_nickname`][svc]
/// verbatim. See [`unconnected_game_add_account_check`] for the
/// `mgmt_session` / `account_dn` round-trip semantics.
///
/// # Errors
///
/// As for [`unconnected_game_add_account_check`].
///
/// [svc]: crate::services::beanfun::unconnected_game_add_account_check_nickname
#[tauri::command]
#[specta::specta]
pub async fn unconnected_game_add_account_check_nickname(
    state: State<'_, AppState>,
    mgmt_session: AddAccountSession,
    account_dn: Option<String>,
) -> Result<CheckOutcome, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let outcome = service_unconnected_game_add_account_check_nickname(
        &client,
        &session,
        &mgmt_session,
        account_dn.as_deref(),
    )
    .await?;
    Ok(outcome)
}

/// Finalise unconnected-game account creation — POST `02.aspx`
/// with the full add-account form (id + password ×2 + optional
/// display name + `chkBox1=on` + `imgbtn_Submit.x/y=0`).
///
/// # Contract
///
/// Mirrors [`services::beanfun::unconnected_game_add_account`][svc]
/// verbatim. Returns [`AddAccountOutcome::Success`] when the
/// response carries no (or empty) `lblErrorMessage`, otherwise
/// [`AddAccountOutcome::ErrorMessage`] carrying the message text.
///
/// # Why pre-validate empty inputs?
///
/// The service layer rejects empty `name` / `new_password` /
/// `new_password_confirm` with `LoginError::Unknown(_)` (mapped
/// to `auth.unknown` at the `CommandError` boundary). This is a
/// backstop — the frontend dialog (`UnconnectedGame_AddAccount.vue`)
/// runs WPF-equivalent client-side validation (length range,
/// password mismatch, contract checkbox) before invoking, so this
/// path should never trigger in practice. Surfacing the typed
/// error keeps the contract honest if a future caller bypasses
/// the dialog.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - `auth.unknown` — empty `name` / `new_password` /
///   `new_password_confirm` (defensive).
/// - Any [`LoginError`][le] surfaced by the service.
///
/// [svc]: crate::services::beanfun::unconnected_game_add_account
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn unconnected_game_add_account(
    state: State<'_, AppState>,
    mgmt_session: AddAccountSession,
    name: String,
    new_password: String,
    new_password_confirm: String,
    account_dn: Option<String>,
) -> Result<AddAccountOutcome, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let outcome = service_unconnected_game_add_account(
        &client,
        &session,
        &mgmt_session,
        &name,
        &new_password,
        &new_password_confirm,
        account_dn.as_deref(),
    )
    .await?;
    Ok(outcome)
}

/// Drive the 5-step unconnected-game change-password flow — auth
/// preamble + 01Accounts.aspx GET / POST + 03.aspx GET / POST
/// (HK uses `http://` for the last 3 steps by upstream design;
/// see service-layer module docs).
///
/// # Contract
///
/// Mirrors [`services::beanfun::unconnected_game_change_password`][svc]
/// verbatim. Returns one of:
///
/// - [`ChangePasswordOutcome::VerifyCodeSent`] — server emitted a
///   `verify_code=<token>` query parameter on the final redirect
///   URL. Caller surfaces the token to the user so they can paste
///   it into the Beanfun verify dialog.
/// - [`ChangePasswordOutcome::ErrorMessage`] — server rendered a
///   non-empty `lblErrorMessage` span. Caller shows the verbatim
///   text in the dialog.
///
/// # Why pull `service_code` / `service_region` from the session?
///
/// Same reason as [`unconnected_game_init_add_account_payload`] —
/// the dialog only ever targets the user's currently selected
/// unconnected game.
///
/// # Why expose `num` over IPC?
///
/// `num` is the 0-based row index inside `gvServiceAccountList`
/// the user clicked on (WPF `MainWindow.xaml.cs::ResetPassword_Click`
/// passes `int`; we use `i32` for direct parity). The frontend
/// gets it from the row position the user invoked "change
/// password" on, so it has to flow through IPC. The backend has no
/// other way to know which row the user picked.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - All three `AccountMgmtMissing*` view-state variants from the
///   parser steps (step 2 + step 4 of the 5-step flow).
/// - Any [`LoginError`][le] surfaced by the service (transport /
///   non-2xx on any of the five HTTP calls).
///
/// [svc]: crate::services::beanfun::unconnected_game_change_password
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn unconnected_game_change_password(
    state: State<'_, AppState>,
    num: i32,
    email: String,
) -> Result<ChangePasswordOutcome, CommandError> {
    let (client, session) = require_auth(state.inner()).await?;
    let outcome = service_unconnected_game_change_password(
        &client,
        &session,
        &session.service_code,
        &session.service_region,
        num,
        &email,
    )
    .await?;
    Ok(outcome)
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
        let _ = unconnected_game_init_add_account_payload;
        let _ = unconnected_game_add_account_check;
        let _ = unconnected_game_add_account_check_nickname;
        let _ = unconnected_game_add_account;
        let _ = unconnected_game_change_password;
        let _ = set_active_service;
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

    /// `AddAccountSession` must be **round-trippable** through serde
    /// because the frontend treats it as an opaque cursor — it
    /// receives the triplet from
    /// [`unconnected_game_init_add_account_payload`] and re-passes
    /// it verbatim to [`unconnected_game_add_account_check`] /
    /// [`unconnected_game_add_account_check_nickname`] /
    /// [`unconnected_game_add_account`]. A future `#[serde(skip)]`
    /// or rename slipping into the struct would silently truncate
    /// the round-trip and the next POST would fail with
    /// `AccountMgmtMissingViewState` / `…ViewStateGenerator` /
    /// `…EventValidation` (or, worse, send an empty `region`
    /// discriminant that throws off the HK `__VIEWSTATEENCRYPTED`
    /// splice). This assertion is the structural backstop.
    #[test]
    fn add_account_session_serde_roundtrip_preserves_all_fields() {
        let original = AddAccountSession {
            viewstate: "/wEPDwULLTE4OTQyMTYwOTRkZIIxN…=".into(),
            viewstate_generator: "B41B0BB6".into(),
            event_validation: "/wEdAANEYWGV…=".into(),
            region: crate::services::beanfun::LoginRegion::HK,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: AddAccountSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, original);
    }

    /// Pin the `tag = "kind", content = "data"` enum representation
    /// for [`AddAccountOutcome`] — the frontend's discriminated
    /// union switch (`switch (outcome.kind) { case "success": ...
    /// case "error_message": ... }`) would silently break if a
    /// future maintainer dropped the `#[serde(tag = ..., content =
    /// ...)]` attribute. Matches the equivalent guard
    /// [`launcher::tests`] applies to [`GameStartMode`].
    #[test]
    fn add_account_outcome_serde_shape_is_stable() {
        let success = serde_json::to_value(AddAccountOutcome::Success).expect("ser");
        assert_eq!(success, serde_json::json!({"kind": "success"}));

        let err = serde_json::to_value(AddAccountOutcome::ErrorMessage("bad".into())).expect("ser");
        assert_eq!(
            err,
            serde_json::json!({"kind": "error_message", "data": "bad"})
        );
    }

    /// As above, for [`ChangePasswordOutcome`] — the frontend
    /// branches on `outcome.kind` to either show the
    /// "MsgChangePassword" toast (with `data` as the verify token)
    /// or render an inline error banner (with `data` as the
    /// `lblErrorMessage` body).
    #[test]
    fn change_password_outcome_serde_shape_is_stable() {
        let ok = serde_json::to_value(ChangePasswordOutcome::VerifyCodeSent("tok123".into()))
            .expect("ser");
        assert_eq!(
            ok,
            serde_json::json!({"kind": "verify_code_sent", "data": "tok123"})
        );

        let err =
            serde_json::to_value(ChangePasswordOutcome::ErrorMessage("nope".into())).expect("ser");
        assert_eq!(
            err,
            serde_json::json!({"kind": "error_message", "data": "nope"})
        );
    }

    // -------------------------------------------------------------
    // P12.3 D8a — set_active_service tests
    // -------------------------------------------------------------

    /// Build an [`AuthContext`] seeded with the WPF-default service
    /// code/region pair so each `set_active_service` test starts
    /// from the same well-known baseline. Mirrors the helper of the
    /// same shape in [`super::super::session::tests`] —
    /// intentionally duplicated rather than re-exported because
    /// `pub(super)` would force the `session` module to be aware of
    /// the `account` module's test fixtures (P10.2 cross-module
    /// test isolation).
    fn seeded_auth_context() -> crate::commands::state::AuthContext {
        use crate::services::beanfun::client::{BeanfunClient, ClientConfig, LoginRegion};
        use crate::services::beanfun::session::Session;
        let client = BeanfunClient::new(ClientConfig::default()).expect("client builds");
        let session = Session::new(
            LoginRegion::TW,
            "SKEY_TEST",
            "BFWT_TEST",
            "alice",
            "610074",
            "T9",
        );
        crate::commands::state::AuthContext::new(client, session)
    }

    /// Hot path: switching the active service must mutate
    /// `session.service_code` / `session.service_region` in place
    /// so a subsequent `require_auth` snapshot (= the next
    /// `get_accounts` call) sees the new pair. Asserted by reading
    /// back through the `RwLock` — exactly the path
    /// `list_accounts_internal` takes.
    #[tokio::test]
    async fn set_active_service_updates_session_pair_in_place() {
        let app = empty_state();
        {
            let mut guard = app.auth.write().await;
            *guard = Some(seeded_auth_context());
        }

        set_active_service_internal(&app, "610153".into(), "TN".into())
            .await
            .expect("logged-in update should succeed");

        let guard = app.auth.read().await;
        let ctx = guard.as_ref().expect("auth still populated after swap");
        assert_eq!(ctx.session.service_code, "610153");
        assert_eq!(ctx.session.service_region, "TN");
    }

    /// Other session fields (`account_id`, `region`, secrets) must
    /// survive a service swap untouched — losing `account_id` would
    /// break the i18n footer label, losing `region` would point
    /// every follow-up POST at the wrong host, and losing the
    /// secrets would force the user to re-login mid-game-switch.
    /// This is the structural guard that the hot-path test pairs
    /// with.
    #[tokio::test]
    async fn set_active_service_preserves_other_session_fields() {
        use crate::services::beanfun::client::LoginRegion;

        let app = empty_state();
        {
            let mut guard = app.auth.write().await;
            *guard = Some(seeded_auth_context());
        }

        set_active_service_internal(&app, "610085".into(), "TC".into())
            .await
            .expect("update succeeds");

        let guard = app.auth.read().await;
        let ctx = guard.as_ref().expect("auth populated");
        assert_eq!(ctx.session.account_id, "alice");
        assert_eq!(ctx.session.region, LoginRegion::TW);
        assert_eq!(ctx.session.skey, "SKEY_TEST");
        assert_eq!(ctx.session.web_token, "BFWT_TEST");
    }

    /// `set_active_service` must surface
    /// [`SESSION_REQUIRED_CODE`][crate::commands::session::SESSION_REQUIRED_CODE]
    /// when no login is active, mirroring every other
    /// session-required command. Defends against a future refactor
    /// that drops the `None` arm — the picker is gated behind the
    /// auth-required AccountList route, but a non-auth caller
    /// (background task, unit test, future broker) must still see
    /// a structured error rather than a silent no-op.
    #[tokio::test]
    async fn set_active_service_without_session_surfaces_session_required() {
        let app = empty_state();
        let err = set_active_service_internal(&app, "610153".into(), "TN".into())
            .await
            .expect_err("no session → error");

        assert_eq!(err.code, SESSION_REQUIRED_CODE);
        assert!(
            !err.message.is_empty(),
            "the message must be non-empty so `tracing` surfaces something useful",
        );
    }

    /// Same-game re-selection must be a successful no-op (the
    /// frontend doesn't gate the picker against picking the
    /// already-selected game). Asserted to avoid a future
    /// `if old != new { ... }` micro-optimization that would skip
    /// the write and break a downstream assumption that the
    /// command **always** completes the user's intent (including
    /// any future side-effects we add to it).
    #[tokio::test]
    async fn set_active_service_same_pair_is_a_noop_success() {
        let app = empty_state();
        {
            let mut guard = app.auth.write().await;
            *guard = Some(seeded_auth_context());
        }

        set_active_service_internal(&app, "610074".into(), "T9".into())
            .await
            .expect("re-selecting the current game is fine");

        let guard = app.auth.read().await;
        let ctx = guard.as_ref().expect("auth populated");
        assert_eq!(ctx.session.service_code, "610074");
        assert_eq!(ctx.session.service_region, "T9");
    }

    /// Empty `(code, region)` strings must be accepted at the
    /// command boundary — input validation is the service layer's
    /// responsibility (the next `get_accounts` POST will surface
    /// the gamezone server-side rejection). This mirrors WPF's
    /// "the picker dialog supplies whatever it supplies" stance
    /// (see `MainWindow.GameList.SelectionChanged`) and prevents
    /// a future overzealous guard from silently swallowing a
    /// legitimate (but unusual) game code the dialog might surface.
    #[tokio::test]
    async fn set_active_service_accepts_empty_strings() {
        let app = empty_state();
        {
            let mut guard = app.auth.write().await;
            *guard = Some(seeded_auth_context());
        }

        set_active_service_internal(&app, String::new(), String::new())
            .await
            .expect("validation deferred to the next get_accounts call");

        let guard = app.auth.read().await;
        let ctx = guard.as_ref().expect("auth populated");
        assert_eq!(ctx.session.service_code, "");
        assert_eq!(ctx.session.service_region, "");
    }
}
