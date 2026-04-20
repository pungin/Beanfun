//! Game commands — fetch the per-region game catalogue (INI +
//! ServiceList) so the frontend's `GameList.vue` dialog and the
//! `selectedGameChanged` pipeline have something to render.
//!
//! # Family exposed in P12.3
//!
//! | Command       | Family | Purpose                                                                  |
//! |---------------|--------|--------------------------------------------------------------------------|
//! | [`list_games`]| game   | Atomic INI + ServiceList fetch for the current session's region          |
//!
//! # Why one command instead of two?
//!
//! WPF's `MainWindow.xaml.cs::reLoadGameInfo` (L682-729) fetches
//! `get_service_ini.ashx` and `game_zone/` back-to-back inside one
//! method body and only writes the parsed result into
//! `MainWindow.GameList[region]` once **both** halves have parsed
//! successfully. Splitting them across two IPC calls would either
//! leave the frontend with a half-populated `useGameStore` mid-load
//! (race-y for the GameList dialog open/close cycle) or force the
//! frontend to re-implement the atomicity guarantee. One command,
//! one round-trip, one [`GameInfoBundle`] — matches WPF and
//! keeps the IPC surface minimal (P10 SRP).
//!
//! # Why session-gated?
//!
//! WPF's only call sites for `reLoadGameInfo` are:
//!
//! - `loginCompleted()` (post-login bootstrap)
//! - `selectedGameChanged()` (when `INIData == null`, i.e. the
//!   first per-session game switch)
//!
//! Both happen **after** the user has authenticated. The
//! `get_service_ini.ashx` and `game_zone/` endpoints don't require
//! the bfWebToken cookie technically, but gating on session here
//! mirrors WPF's runtime invariant (game catalog is only ever
//! requested from inside the logged-in shell) and keeps the
//! [`State<AppState>`] usage uniform across the command surface
//! (every read goes through [`require_auth`]). If a future
//! "splash screen browse without login" UX is wanted, a
//! `list_games_anonymous(region)` sibling can be added without
//! touching this one — locking down the session-gated variant
//! today doesn't paint us into a corner.

use tauri::State;

use crate::commands::{error::CommandError, session::require_auth, state::AppState};
use crate::services::beanfun::{list_games as service_list_games, GameInfoBundle};

/// Fetch the per-region INI of executable metadata + the ordered
/// list of game services for the active session's region.
///
/// Mirrors `MainWindow.xaml.cs::reLoadGameInfo` (L682-729) — one
/// atomic fetch returns both halves so the frontend never observes
/// a half-populated state.
///
/// # Returns
///
/// A [`GameInfoBundle`] with:
///
/// - `ini` — `HashMap<String, GameIniEntry>` keyed by
///   `<service_code>_<service_region>` (e.g. `"610074_T9"`).
/// - `services` — `Vec<GameService>` preserving server ordering.
///
/// # Errors
///
/// - `auth.session_required` — no login is active.
/// - Every [`LoginError`][le] surfaced by the underlying service —
///   `network.http_failed`, `network.body_too_large`,
///   `network.json_decode_failed`, `auth.invalid_utf8`,
///   `auth.invalid_url`, `auth.unknown` (non-2xx),
///   `game.service_list_missing` (catastrophic upstream
///   regression — `Services.ServiceList = …;` literal absent).
///
/// # Frontend usage
///
/// Called once per session by `useGameStore.loadGames()` on
/// AccountList mount, with optional `force=true` re-runs if the
/// user manually retries from the GameList dialog's error
/// banner.
///
/// [le]: crate::services::beanfun::LoginError
#[tauri::command]
#[specta::specta]
pub async fn list_games(state: State<'_, AppState>) -> Result<GameInfoBundle, CommandError> {
    let (client, _session) = require_auth(state.inner()).await?;
    let bundle = service_list_games(&client).await?;
    Ok(bundle)
}
