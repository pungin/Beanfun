//! Tauri IPC command surface — the thin async boundary between the
//! frontend (`invoke("...", {...})`) and the service layer.
//!
//! This module is the **only** place that should expose Rust APIs to
//! JavaScript. Upstream crates under [`crate::services`] stay
//! framework-agnostic; downstream consumers under
//! `beanfun-next/src/types/bindings.ts` are auto-generated from the
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
//!   DTO to `beanfun-next/src/types/bindings.ts` on every debug build
//!   (P10-Q4 = A, P10.1-Q6/Q8).
//!
//! # Chunk layout
//!
//! | Chunk | Status   | Focus                                                                                            |
//! |-------|----------|--------------------------------------------------------------------------------------------------|
//! | 10.1  | done     | IPC infrastructure + `version` / `ping` smoke                                                    |
//! | 10.2  | **done** | `auth` (regular / QR / verify / logout) + `account` (base / management / info) + `otp` (this PR) |
//! | 10.3  | pending  | `launcher` / `storage` / `config` / `update` / `system` (extends 10.1)                            |
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
pub mod dto;
pub mod error;
pub mod otp;
pub mod session;
pub mod state;
pub mod system;

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
///    to regenerate `beanfun-next/src/types/bindings.ts`, keeping
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
///    `beanfun-next/src/types/bindings.ts`. Commit the regenerated
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
        // otp (P10.2)
        otp::get_otp,
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
    //! exist (D8 only regenerates on debug-build *boot*, not on
    //! `cargo check`). The test treats a missing file as "not yet
    //! bootstrapped" and passes with a stderr hint instead of
    //! failing — CI pipelines that care about the contract should
    //! either commit `bindings.ts` to the repo or run a `cargo tauri
    //! dev`-style bootstrap step before `cargo test`.
    //!
    //! [`CommandError`]: super::error::CommandError
    //! [`VersionInfo`]: super::system::VersionInfo

    use std::path::PathBuf;

    /// Path to the committed `bindings.ts` the frontend imports from.
    ///
    /// Resolved at compile time via [`env!`] on `CARGO_MANIFEST_DIR`
    /// so the test never hard-codes a relative assumption about the
    /// working directory `cargo test` happens to run from. Mirrors
    /// the production target computed in
    /// [`crate::export_specta_bindings`].
    fn bindings_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri always has a parent (the Tauri project root)")
            .join("src")
            .join("types")
            .join("bindings.ts")
    }

    /// Symbols the frontend imports from `bindings.ts`. The assertion
    /// only matches against lines whose first non-whitespace token is
    /// `export` — full regex would be more targeted but is fragile
    /// against specta's evolving output formatting (semicolons,
    /// `export type` vs `export interface`, trailing commas), while a
    /// bare `contents.contains` matches comments and doc strings that
    /// legitimately mention a type name without exporting it.
    const REQUIRED_SYMBOLS: &[&str] = &[
        // --- commands (P10.1 — system smoke) ------------------------
        "version",
        "ping",
        // --- commands (P10.2 — auth regular family) -----------------
        "login_regular",
        "login_totp",
        // --- commands (P10.2 — auth QR family) ----------------------
        "login_qr_start",
        "login_qr_check",
        // --- commands (P10.2 — auth verify family) ------------------
        "get_verify_page_info",
        "get_verify_captcha",
        "submit_verify",
        // --- commands (P10.2 — logout) -------------------------------
        "logout",
        // --- commands (P10.2 — account base + management + info) ----
        "get_accounts",
        "refresh",
        "add_service_account",
        "change_display_name",
        "get_contract",
        "get_email",
        "get_remain_point",
        // --- commands (P10.2 — otp) ---------------------------------
        "get_otp",
        // --- DTOs (P10.1) -------------------------------------------
        "CommandError",
        "VersionInfo",
        // --- DTOs (P10.2 — auth / session DTOs) ---------------------
        "SessionInfo",
        "LoginRegion",
        "TotpChallengeInfo",
        "QrStart",
        "QrStatus",
        "VerifyPage",
        "VerifyCaptcha",
        "VerifySubmit",
        // --- DTOs (P10.2 — account DTOs) ----------------------------
        "ServiceAccount",
        "AccountListResult",
        "AmountLimitNotice",
    ];

    #[test]
    fn bindings_file_contains_all_p101_symbols() {
        let path = bindings_path();
        let Ok(contents) = std::fs::read_to_string(&path) else {
            // Fresh clone / someone deleted the generated file —
            // treat as "not yet bootstrapped" rather than failing.
            // See the module-level note on fresh-clone behaviour.
            eprintln!(
                "[skip] bindings.ts not found at {}; run `cargo tauri dev` \
                 once to regenerate (see `crate::export_specta_bindings`)",
                path.display()
            );
            return;
        };

        assert!(
            !contents.is_empty(),
            "bindings.ts at {} is empty — did the last `cargo tauri dev` boot crash \
             before `tauri_specta::Builder::export` finished?",
            path.display()
        );

        // Narrow the search surface to `export`-prefixed lines so
        // stray comments / docblocks that mention a symbol by name
        // don't fool the check (a renamed command with a leftover
        // `// CommandError ...` comment would otherwise slip past).
        let export_lines: String = contents
            .lines()
            .filter(|line| line.trim_start().starts_with("export"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            !export_lines.is_empty(),
            "bindings.ts at {} contains no `export` declaration — file is malformed \
             or truncated; rerun `cargo tauri dev` to regenerate",
            path.display()
        );

        for symbol in REQUIRED_SYMBOLS {
            assert!(
                export_lines.contains(symbol),
                "bindings.ts is missing exported `{symbol}` — rerun `cargo tauri dev` \
                 to regenerate, then commit the updated file. Expected symbols: {:?}",
                REQUIRED_SYMBOLS
            );
        }
    }
}
