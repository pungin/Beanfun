//! Tauri IPC command surface — the thin async boundary between the
//! frontend (`invoke("...", {...})`) and the service layer.
//!
//! This module is the **only** place that should expose Rust APIs to
//! JavaScript. Upstream crates under [`crate::services`] stay
//! framework-agnostic; downstream consumers under
//! `src/types/bindings.ts` are auto-generated from the
//! `#[tauri::command] #[specta::specta]` signatures here.
//!
//! # Architecture (P10 chunk 10.1)
//!
//! ```text
//! ┌─────────────────┐  invoke("cmd", {args})  ┌────────────────────────┐
//! │ Vue + Pinia     │ ──────────────────────▶ │ tauri-specta runtime    │
//! │ (bindings.ts)   │ ◀────────────────────── │ (invoke_handler)        │
//! └─────────────────┘      JSON DTO           └───────────┬────────────┘
//!                                                         │
//!                                      State<'_, AppState>│
//!                                                         ▼
//!                                             ┌────────────────────────┐
//!                                             │ commands::{auth, ...}  │
//!                                             │  (this module)         │
//!                                             └───────────┬────────────┘
//!                                                         │
//!                                           sync call /    │
//!                                        `spawn_blocking(…)`│
//!                                                         ▼
//!                                             ┌────────────────────────┐
//!                                             │ services::{beanfun,…}  │
//!                                             │ (domain, framework-    │
//!                                             │  agnostic)             │
//!                                             └────────────────────────┘
//! ```
//!
//! # Design principles (locked in P10 pre-flight)
//!
//! - **Single [`AppState`][state::AppState]** — HTTP client + storage
//!   root + login session, wired via `Builder::manage(AppState::new(..))`
//!   (P10-Q2 = A). Chunk 10.2 extends `AppState` with three additional
//!   `RwLock<Option<...>>` slots ([`PendingTotp`][pt],
//!   [`PendingQr`][pq], [`PendingVerify`][pv]) that hold continuation
//!   state for multi-step login flows on the backend so secrets never
//!   cross IPC.
//! - **Atomic auth bundle ([`AuthContext`][ac])** — the HTTP client and
//!   [`Session`][sn] are stored together under one lock so readers can
//!   never observe a half-populated state mid-login or mid-logout
//!   (P10.2-Q2 = B).
//! - **Session gating via [`require_auth`][ra]** — every session-scoped
//!   command opens with `let (client, session) = require_auth(state.inner()).await?;`,
//!   yielding `auth.session_required` when no login is active. The
//!   helper returns **owned** clones so the `RwLock` read guard is
//!   dropped before the caller's `.await` points (P10.2-Q3 = A).
//! - **Thin error DTO [`CommandError`][error::CommandError]** — domain
//!   errors are converted through `impl Into<CommandError>` at the
//!   command boundary; the wire format is stable across all commands
//!   (P10-Q3 = C). Continuation flows surface through structured
//!   `details` payloads (e.g. `auth.totp_required` carries
//!   [`TotpChallengeInfo`][tci], `auth.advance_check_required` carries
//!   `{url}`).
//! - **Hybrid DTO strategy** — pure-data domain types
//!   (`ServiceAccount`, [`AccountListResult`][alr], [`LoginRegion`][lr],
//!   QR / verify / TOTP payloads) derive [`specta::Type`] directly;
//!   secret-bearing types ([`Session`][sn], `Credentials`) are reshaped
//!   into [`SessionInfo`][si] / [`TotpChallengeInfo`][tci] safe-subset
//!   DTOs (P10.2-Q4 = C). Binary payloads (QR PNG, verify captcha) are
//!   Base64 data URLs over the wire ([`encode_png_base64`][epb]).
//! - **Blocking isolation** — Win32 / registry / `ShellExecuteW` calls
//!   are synchronous and must run inside
//!   `tokio::task::spawn_blocking` so the async runtime isn't stalled
//!   (P10-Q5 = A; accumulated guidance from P8.2 R8.2-4, P9.1 R9.1-4,
//!   P9.2 R9.2-3).
//! - **Auto-generated TS types** — `tauri-specta` + `specta-typescript`
//!   export all command signatures and the [`CommandError`][error::CommandError]
//!   DTO to `src/types/bindings.ts` on every debug build
//!   (P10-Q4 = A, P10.1-Q6/Q8). The same builder is also reachable
//!   through `cargo run --example export_bindings` for out-of-band
//!   regeneration (added in chunk 10.3 D6 alongside the
//!   [`crate::default_bindings_path`] helper that funnels every
//!   regeneration path to the same on-disk target).
//! - **Platform gating with stable IPC surface** (chunk 10.3) —
//!   Windows-only commands (every command in [`storage`] plus
//!   [`launcher::detect_game_path`], [`launcher::list_game_processes`]
//!   / [`launcher::kill_game_processes`], [`launcher::auto_paste`])
//!   keep their `#[tauri::command]` signatures **unconditional** so
//!   `bindings.ts` exposes the same symbol set on every host.
//!   Non-Windows builds short-circuit the body and surface
//!   `<domain>.platform_unsupported` ([`storage`], [`launcher`]) —
//!   this lets `cargo check` on macOS / Linux dev boxes stay green
//!   without breaking the frontend type contract.
//! - **Hybrid credential pass-through** (chunk 10.3 Q7 = A) —
//!   [`storage::load_accounts`] / [`storage::save_account`] /
//!   [`storage::import_records`] / [`storage::export_records`]
//!   surface decrypted passwords verbatim across IPC, and
//!   [`launcher::launch_game`] / [`launcher::auto_paste`] take
//!   plaintext `account` + `password` parameters. This mirrors the
//!   WPF flow (single-trust-boundary Windows process) and keeps
//!   import/export interoperable with legacy `Users.dat` JSON
//!   exports. The [`storage`] / [`launcher`] module docs
//!   spell out the rationale; future chunks that introduce a
//!   secondary unprivileged renderer would re-apply redaction at
//!   that boundary instead of churning these commands.
//!
//! # Chunk layout
//!
//! | Chunk | Status   | Focus                                                                                                                         |
//! |-------|----------|-------------------------------------------------------------------------------------------------------------------------------|
//! | 10.1  | done     | IPC infrastructure + `version` / `ping` smoke                                                                                 |
//! | 10.2  | done     | `auth` (regular / QR / verify / logout) + `account` (base / management / info) + `otp`                                        |
//! | 10.3  | **done** | `system::open_url` (D1) + `config` 3-cmd (D2) + `storage` 5-cmd (D3) + `update::check_update` (D4) + `launcher` 6-cmd (D5a~d) |
//!
//! [ac]: state::AuthContext
//! [pt]: state::PendingTotp
//! [pq]: state::PendingQr
//! [pv]: state::PendingVerify
//! [ra]: session::require_auth
//! [sn]: crate::services::beanfun::Session
//! [si]: dto::SessionInfo
//! [tci]: dto::TotpChallengeInfo
//! [alr]: crate::services::beanfun::AccountListResult
//! [lr]: crate::services::beanfun::LoginRegion
//! [epb]: dto::encode_png_base64

// Several internal helpers (`require_auth`, `list_accounts_internal`,
// `*_NOT_PENDING_CODE`/`*_NOT_STARTED_CODE` consts, `split_otp_digits`)
// are intentionally `pub(crate)` so the public command surface stays
// minimal, but the docs above and on individual commands link to them
// because they explain the gating / continuation contracts that frontend
// integrators care about. The links remain navigable when docs are
// generated with `--document-private-items` (our default for internal
// docs); silencing the lint here keeps that policy in one place.
#![allow(rustdoc::private_intra_doc_links)]

pub mod account;
pub mod auth;
pub mod config;
#[cfg(target_os = "windows")]
pub mod cookie_native;
pub mod dto;
pub mod error;
pub mod game;
pub mod launcher;
pub mod maple_cache;
pub mod otp;
pub mod session;
pub mod state;
pub mod storage;
pub mod system;
pub mod update;
pub mod web_browser;

use tauri_specta::{collect_commands, Builder};

/// Single source of truth for the set of Tauri commands this crate
/// exposes — every consumer that needs to know "which commands
/// exist?" goes through this helper.
///
/// # Why one helper instead of inlining?
///
/// Two code paths depend on the same command list:
///
/// 1. **[`crate::run`]** attaches the builder's
///    [`invoke_handler`][Builder::invoke_handler] to the
///    [`tauri::Builder`] so commands are dispatched at runtime.
/// 2. **`export_specta_bindings`** (P10.1 D8, private helper in
///    `lib.rs`) calls [`Builder::export`] on every debug-build boot
///    to regenerate `src/types/bindings.ts`, keeping
///    frontend types in lock-step with the Rust signatures.
///
/// Keeping the `collect_commands!` call site in one place means
/// adding a command is a one-line edit (DRY) — there's no risk of
/// runtime and `bindings.ts` drifting against each other. Signatures
/// in this repo have already drifted once across three placement
/// sites in earlier Tauri prototypes; this helper is the structural
/// fix so we don't repeat that mistake.
///
/// # Why generic over `R: tauri::Runtime`?
///
/// Production code instantiates this as `build_specta_builder::<Wry>`
/// so the Tauri dispatcher lines up with the real webview runtime.
/// Future mock-invoke integration tests (planned for P10.2+ once the
/// first business-logic command gives a round-trip assertion a
/// non-trivial payload to validate) are expected to instantiate this
/// as `build_specta_builder::<tauri::test::MockRuntime>` to avoid
/// pulling a full Wry runtime into the test harness. Keeping the
/// helper generic today costs nothing and leaves that door open
/// without forcing a later signature change.
///
/// # Adding a command
///
/// 1. Write `#[tauri::command] #[specta::specta] pub fn foo(...)` in
///    the appropriate sub-module (`auth.rs`, `launcher.rs`, …).
/// 2. Append `module::foo` to the `collect_commands!` list below.
/// 3. Run `cargo tauri dev` once — D8 regenerates `bindings.ts` into
///    `src/types/bindings.ts`. Commit the regenerated
///    file alongside the Rust change; the `bindings_file_tests`
///    submodule (lib-test only) guards CI against accidental drift.
pub fn build_specta_builder<R: tauri::Runtime>() -> Builder<R> {
    Builder::<R>::new().commands(collect_commands![
        // system (P10.1)
        system::version,
        system::ping,
        // auth (P10.2 — regular family)
        auth::login_regular,
        auth::login_totp,
        // auth (P10.2 — QR family)
        auth::login_qr_start,
        auth::login_qr_check,
        // auth (P12.1 D5 — GamePass family)
        //
        // Both `login_gamepass_start` and `open_gamepass_window` are
        // generic over `R: tauri::Runtime` because they take an
        // `AppHandle<R>` (the former to run a window-alive pre-flight
        // guard, the latter to build a `WebviewWindow<R>`).
        // `collect_commands!` forwards generic commands to both
        // `tauri::generate_handler!` (which strips the turbofish and
        // re-injects `R` from the surrounding closure) and
        // `specta::function::collect_functions!` (which expands into
        // a non-generic inner `fn export(...)`). The specta side
        // therefore needs a *concrete* runtime in the turbofish —
        // using the outer `R` trips rustc E0401 (`use of generic
        // parameter from outer item`).
        //
        // TS binding generation is runtime-agnostic — specta only
        // inspects serialisable argument/return types — so pinning
        // the specta side to `tauri::Wry` does not leak into the
        // emitted `bindings.ts` contract; the tauri side retains
        // the generic `R` so the command still monomorphises
        // correctly for whichever runtime `build_specta_builder`
        // is instantiated with (production uses `Wry`).
        auth::login_gamepass_start::<tauri::Wry>,
        auth::open_gamepass_window::<tauri::Wry>,
        // auth (P10.2 — verify family)
        auth::get_verify_page_info,
        auth::get_verify_captcha,
        auth::submit_verify,
        // auth (P10.2 — logout)
        auth::logout,
        // account (P10.2 — base)
        account::get_accounts,
        account::refresh,
        // account (P10.2 — management)
        account::add_service_account,
        account::change_display_name,
        // account (P10.2 — info)
        account::get_contract,
        account::get_email,
        account::get_remain_point,
        // account (P12.3 D3 — unconnected-game flows)
        account::unconnected_game_init_add_account_payload,
        account::unconnected_game_add_account_check,
        account::unconnected_game_add_account_check_nickname,
        account::unconnected_game_add_account,
        account::unconnected_game_change_password,
        // account (P12.3 D8a — game switcher session sync)
        account::set_active_service,
        // otp (P10.2)
        otp::get_otp,
        // system (P10.3 — D1 open_url)
        system::open_url,
        // system (TitleBar minimize-to-tray bridge)
        //
        // Generic over `R: tauri::Runtime` because it takes an
        // `AppHandle<R>` (to drive the window + tray side effects);
        // see the `auth::login_gamepass_start` comment for the full
        // E0401 / runtime-agnostic bindings rationale.
        system::minimize_main_window::<tauri::Wry>,
        // config (P10.3 — D2)
        config::get_config_value,
        config::get_all_config,
        config::set_config,
        // storage (P10.3 — D3)
        storage::load_accounts,
        storage::save_account,
        storage::remove_account,
        storage::import_records,
        storage::export_records,
        // storage AES backup (P12.2 — D10.0; AccRecovery WPF parity)
        storage::backup_export,
        storage::backup_restore,
        // update (P10.3 — D4)
        update::check_update,
        // launcher (P10.3 — D5a launch)
        launcher::launch_game,
        // launcher (P10.3 — D5b path)
        launcher::set_game_path,
        launcher::detect_game_path,
        // launcher (P10.3 — D5c process)
        launcher::list_game_processes,
        launcher::kill_game_processes,
        // launcher (P10.3 — D5d auto-paste)
        launcher::auto_paste,
        // game (P12.3 D2 — list_games)
        game::list_games,
        // maple_cache (P12.5 D1 — Recycling button on MapleTools)
        maple_cache::clean_maple_game_cache,
        // web_browser (P12.4-followup-B — in-app browser window)
        //
        // Generic over `R: tauri::Runtime` for the same reason as
        // `auth::open_gamepass_window`: it takes an `AppHandle<R>`
        // (to call `WebviewWindowBuilder::new`). The specta
        // turbofish must pin to `tauri::Wry` while the tauri side
        // monomorphises against the runtime `build_specta_builder`
        // is instantiated with — see the `auth::login_gamepass_start`
        // comment above for the full E0401 / runtime-agnostic
        // bindings rationale.
        web_browser::open_in_app_browser::<tauri::Wry>,
        // web_browser (P12.4-followup-B-fix F9 — Member Center)
        //
        // Sibling of `open_in_app_browser` that builds the
        // member-center URL backend-side so the session's
        // `web_token` (a server-side secret per `commands::dto`'s
        // sentinel test) never crosses IPC. Same `tauri::Wry`
        // turbofish rationale applies.
        web_browser::open_member_center_browser::<tauri::Wry>,
        web_browser::open_gash_recharge_browser::<tauri::Wry>,
    ])
}

#[cfg(test)]
mod bindings_file_tests {
    //! Guard against drift between the Rust command contract and the
    //! committed `bindings.ts` the frontend imports from.
    //!
    //! # Why a plain file grep (and not an in-process export)?
    //!
    //! The obvious design — spin up
    //! [`super::build_specta_builder`] in a `#[test]` and run
    //! [`tauri_specta::Builder::export`] against a `tempfile::TempDir`
    //! — pulls `tauri_specta::Builder<R>` (for *any* `R`, including
    //! [`tauri::test::MockRuntime`]) into the test binary's link
    //! closure, which transitively drags in `tauri-runtime-wry` →
    //! `webview2-com-sys`. That crate's build script links the
    //! WebView2 import lib as a regular DLL dependency (no
    //! `delayload`), so Windows refuses to load the test `.exe` with
    //! `STATUS_ENTRYPOINT_NOT_FOUND` whenever `WebView2Loader.dll`
    //! isn't on `PATH`. The existing 461 unit tests stay green
    //! because none of them statically reference the `Builder`
    //! symbol graph.
    //!
    //! Rather than fight the Tauri ecosystem's native-DLL story or
    //! duplicate the specta export pipeline behind a MockRuntime
    //! shim, this test treats `bindings.ts` as a committed artefact
    //! (the frontend imports from it at compile time anyway) and
    //! asserts its contents directly. The D8 production path keeps
    //! the file fresh on every `cargo tauri dev` boot; commits pick
    //! up the regenerated file alongside the Rust change.
    //!
    //! # Failure mode
    //!
    //! If someone renames a command or renames a [`CommandError`][]
    //! / [`VersionInfo`][] field without rerunning `cargo tauri dev`,
    //! CI catches the drift here with a pointer to the regenerate
    //! step.
    //!
    //! # Fresh-clone behaviour
    //!
    //! On a brand-new checkout `bindings.ts` legitimately does not
    //! exist (the production exporter only runs on debug-build
    //! *boot*, not on `cargo check`). The test treats a missing
    //! file as "not yet bootstrapped" and passes with a stderr
    //! hint instead of failing — CI pipelines that care about the
    //! contract should either commit `bindings.ts` to the repo or
    //! run a `cargo run --example export_bindings`-style bootstrap
    //! step before `cargo test`. The example binary
    //! ([`beanfun_lib::default_bindings_path`]) shares the
    //! target path with the debug-boot exporter and the test, so
    //! every regeneration path lands in the same canonical
    //! location by construction.
    //!
    //! [`CommandError`]: super::error::CommandError
    //! [`VersionInfo`]: super::system::VersionInfo

    use std::path::PathBuf;

    /// Path to the committed `bindings.ts` the frontend imports from.
    ///
    /// Thin wrapper over [`crate::default_bindings_path`] — the
    /// single source of truth shared with the debug-boot exporter
    /// in [`crate::run`] and the
    /// `src-tauri/examples/export_bindings.rs`
    /// standalone regenerate-bindings entry point. Kept as a
    /// wrapper (rather than inlining
    /// `crate::default_bindings_path()` at every call site) so this
    /// test file reads end-to-end without a single external jump
    /// and so a future change — e.g. falling back to a
    /// `TEST_BINDINGS_PATH` env var in CI — lands in one place.
    fn bindings_path() -> PathBuf {
        crate::default_bindings_path()
    }

    /// Tauri commands the frontend invokes via `commands.<name>(...)`.
    ///
    /// # Naming convention
    ///
    /// Listed in **camelCase** because that's how `tauri-specta`
    /// emits the wrapper methods on the
    /// `export const commands = {...}` const. Rust source uses
    /// `snake_case` (`#[tauri::command] pub async fn login_regular`)
    /// and specta auto-converts to camelCase (`async loginRegular`)
    /// when emitting TS — to match what the frontend actually
    /// imports, the assertion has to look for the camelCase form.
    ///
    /// # Search pattern
    ///
    /// Each entry is searched for via `async <name>(` against the
    /// full file contents — see [`bindings_file_contains_all_symbols`]
    /// for the rationale (the per-command method definitions live
    /// inside an `export const commands = {...}` block, so they're
    /// not on `export`-prefixed lines and need a non-export-only
    /// search surface; the `async ` prefix narrows the surface
    /// enough to avoid stray doc-comment matches).
    ///
    /// # P10.1 D8 design follow-up
    ///
    /// The original `REQUIRED_SYMBOLS` list lumped commands and
    /// DTOs together and searched for both within `export`-prefixed
    /// lines only. That worked for the first two commands
    /// (`version` / `ping` — single-word identities that survive
    /// the snake↔camel transform unchanged) but broke as soon as
    /// P10.3 introduced multi-word commands like `login_regular` /
    /// `open_url`. Splitting the list into commands + DTOs and
    /// using a dedicated pattern per group is the structural fix:
    /// each side gets the strictest possible match without false
    /// positives.
    const REQUIRED_COMMANDS: &[&str] = &[
        // --- P10.1 — system smoke ------------------------------------
        "version",
        "ping",
        // --- P10.2 — auth regular family -----------------------------
        "loginRegular",
        "loginTotp",
        // --- P10.2 — auth QR family ----------------------------------
        "loginQrStart",
        "loginQrCheck",
        // --- P12.1 D5 — auth GamePass family -------------------------
        "loginGamepassStart",
        "openGamepassWindow",
        // --- P10.2 — auth verify family ------------------------------
        "getVerifyPageInfo",
        "getVerifyCaptcha",
        "submitVerify",
        // --- P10.2 — logout ------------------------------------------
        "logout",
        // --- P10.2 — account base + management + info ----------------
        "getAccounts",
        "refresh",
        "addServiceAccount",
        "changeDisplayName",
        "getContract",
        "getEmail",
        "getRemainPoint",
        // --- P10.2 — otp ---------------------------------------------
        "getOtp",
        // --- P10.3 — D1 system ---------------------------------------
        "openUrl",
        // --- TitleBar minimize-to-tray bridge ------------------------
        "minimizeMainWindow",
        // --- P10.3 — D2 config ---------------------------------------
        "getConfigValue",
        "getAllConfig",
        "setConfig",
        // --- P10.3 — D3 storage --------------------------------------
        "loadAccounts",
        "saveAccount",
        "removeAccount",
        "importRecords",
        "exportRecords",
        // --- P12.2 D10.0 — storage AES backup ------------------------
        "backupExport",
        "backupRestore",
        // --- P10.3 — D4 update ---------------------------------------
        "checkUpdate",
        // --- P10.3 — D5 launcher -------------------------------------
        "launchGame",
        "setGamePath",
        "detectGamePath",
        "listGameProcesses",
        "killGameProcesses",
        "autoPaste",
        // --- P12.3 D2 — game catalogue ------------------------------
        "listGames",
        // --- P12.3 D3 — unconnected-game account flows --------------
        "unconnectedGameInitAddAccountPayload",
        "unconnectedGameAddAccountCheck",
        "unconnectedGameAddAccountCheckNickname",
        "unconnectedGameAddAccount",
        "unconnectedGameChangePassword",
        // --- P12.3 D8a — game switcher session sync -----------------
        "setActiveService",
        // --- P12.5 D1 — MapleStory cache cleanup --------------------
        "cleanMapleGameCache",
        // --- P12.4-followup-B — in-app browser window ---------------
        "openInAppBrowser",
        // --- P12.4-followup-B-fix F9 — Member Center ----------------
        "openMemberCenterBrowser",
        // --- Gash recharge (WPF bfb_Gash_Click parity) ---------------
        "openGashRechargeBrowser",
    ];

    /// DTO type names the frontend imports from `bindings.ts`.
    ///
    /// # Naming convention
    ///
    /// Listed in **PascalCase** because that's how Rust struct /
    /// enum names round-trip through `specta::Type` into TS
    /// (`export type CommandError = {...}` /
    /// `export type LoginRegion = "Tw" | "Hk"` etc.).
    ///
    /// # Search pattern
    ///
    /// Each entry is searched for via `export type <Name>` against
    /// the file contents — the prefix anchors the match to an
    /// actual top-level type alias declaration, so a renamed type
    /// with a leftover `// CommandError ...` doc comment can't
    /// slip past.
    ///
    /// # Notable omissions
    ///
    /// - `Records` is a `Vec<Account>` newtype on the Rust side;
    ///   tauri-specta inlines it as `Account[]` in the emitted TS
    ///   (no `export type Records = ...` alias is produced) so it
    ///   intentionally does **not** appear in this list. The
    ///   `Account` row type carries the meaningful schema for the
    ///   frontend.
    /// - `TotpChallengeInfo` derives `specta::Type` but is only
    ///   ever serialised into [`CommandError`]'s
    ///   [`details`][crate::commands::error::CommandError::details]
    ///   field via `serde_json::to_value(...)` — it never appears
    ///   in a `#[tauri::command]` signature, and `tauri-specta`
    ///   only emits types reachable from registered command
    ///   signatures. The frontend treats `auth.totp_required`'s
    ///   `details` payload as free-form JSON (matching the
    ///   `JsonValue` recursive type already in `bindings.ts`); a
    ///   future chunk may expose `TotpChallengeInfo` as a
    ///   first-class type by surfacing it through a dedicated
    ///   `tauri-specta::Builder::types()` registration.
    const REQUIRED_DTOS: &[&str] = &[
        // --- P10.1 ---------------------------------------------------
        "CommandError",
        "VersionInfo",
        // --- P10.2 — auth / session ---------------------------------
        "SessionInfo",
        "LoginRegion",
        "QrStart",
        "QrStatus",
        "VerifyPage",
        "VerifyCaptcha",
        "VerifySubmit",
        // --- P10.2 — account ----------------------------------------
        "ServiceAccount",
        "AccountListResult",
        "AmountLimitNotice",
        // --- P10.3 — storage row-shape DTO --------------------------
        "Account",
        // --- P10.3 — game / launcher --------------------------------
        "GameStartMode",
        "GameProcessInfo",
        "AutoPasteRequest",
        // --- P12.3 D2 — game catalogue ------------------------------
        "GameIniEntry",
        "GameService",
        "GameInfoBundle",
        // --- P12.3 D3 — unconnected-game DTOs ------------------------
        "AddAccountSession",
        "AddAccountInit",
        "CheckOutcome",
        "AddAccountOutcome",
        "ChangePasswordOutcome",
        // --- P10.3 — updater ----------------------------------------
        "Channel",
        "UpdateInfo",
    ];

    #[test]
    fn bindings_file_contains_all_symbols() {
        let path = bindings_path();
        let Ok(contents) = std::fs::read_to_string(&path) else {
            // Fresh clone / someone deleted the generated file —
            // treat as "not yet bootstrapped" rather than failing.
            // See the module-level note on fresh-clone behaviour.
            eprintln!(
                "[skip] bindings.ts not found at {}; run `cargo tauri dev` \
                 once (or `cargo run --example export_bindings`) to regenerate \
                 (see `crate::export_specta_bindings` / `crate::default_bindings_path`)",
                path.display()
            );
            return;
        };

        assert!(
            !contents.is_empty(),
            "bindings.ts at {} is empty — did the last `cargo tauri dev` / \
             `cargo run --example export_bindings` invocation crash before \
             `tauri_specta::Builder::export` finished?",
            path.display()
        );

        for cmd in REQUIRED_COMMANDS {
            let needle = format!("async {cmd}(");
            assert!(
                contents.contains(&needle),
                "bindings.ts is missing wrapper method `async {cmd}(` — rerun \
                 `cargo run --example export_bindings` to regenerate, then commit \
                 the updated file. Expected commands: {:?}",
                REQUIRED_COMMANDS
            );
        }

        for dto in REQUIRED_DTOS {
            let needle = format!("export type {dto}");
            assert!(
                contents.contains(&needle),
                "bindings.ts is missing exported type `{dto}` — rerun \
                 `cargo run --example export_bindings` to regenerate, then commit \
                 the updated file. Expected DTOs: {:?}",
                REQUIRED_DTOS
            );
        }
    }
}
