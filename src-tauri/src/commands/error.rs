//! IPC error DTO — all Tauri commands surface failures through
//! [`CommandError`], a framework-facing struct that preserves the
//! stable wire contract **(`code`, `message`, optional `details`)**
//! across every domain.
//!
//! # Why a flat DTO instead of a tagged enum?
//!
//! Serializing each domain error directly (e.g. `Result<T, LoginError>`
//! across the IPC boundary) would leak internal structure (renamed
//! variants, new fields, nested `#[source]` chains) into
//! `bindings.ts`, forcing the frontend to branch on Rust-specific
//! shapes. A thin DTO keeps the contract stable while still carrying
//! enough information:
//!
//! - `code` — **stable identifier** the frontend pattern-matches on for
//!   i18n keys and flow control (`auth.totp_required`,
//!   `network.http_failed`, …). Naming convention is
//!   `<domain>.<variant_in_snake_case>` (see [code naming](#code-naming)
//!   below).
//! - `message` — human-readable Rust-side message derived from the
//!   domain error's `Display` impl; safe to log; **not** localized
//!   (frontend maps `code` → localized string, or falls back to
//!   `message` verbatim when no i18n key is defined).
//! - `details` — optional structured context
//!   ([`serde_json::Value`][`serde_json::Value`]) the domain can attach
//!   for richer UI affordances (e.g. `{"pid": 1234}` for
//!   `ShellExecute` failures, `{"http_status": 503}` for transient
//!   network blips, `{"url": "..."}` for the AdvanceCheck continuation
//!   URL). Keeps the DTO open-closed across domain evolution.
//!
//! # Code naming
//!
//! ```text
//! <domain>.<variant_in_snake_case>
//! ```
//!
//! The code granularity is **fine** — every domain error variant gets
//! its own code (P10.1 D4 decision "A"). Rationale:
//!
//! - `match` is exhaustive → adding a variant to a domain enum is a
//!   hard compile-fail reminder to extend the `From` impl and the
//!   tables below.
//! - Preserves the P8.2 R8.2-3 precedent ("expose enough signal for the
//!   UI to branch"); lossless at the boundary.
//! - i18n is opt-in: the frontend only defines keys for codes it wants
//!   to localize and falls back to `message` for the long tail.
//!
//! ## `LoginError` — `auth.*` / `network.*`
//!
//! | Variant                                     | Code                                               | `details` fields                              |
//! | ------------------------------------------- | -------------------------------------------------- | --------------------------------------------- |
//! | `MissingSessionKey`                         | `auth.missing_session_key`                         | —                                             |
//! | `EmptyResponse`                             | `auth.empty_response`                              | —                                             |
//! | `MissingVerificationToken`                  | `auth.missing_verification_token`                  | —                                             |
//! | `MissingViewState`                          | `auth.missing_view_state`                          | —                                             |
//! | `MissingViewStateGenerator`                 | `auth.missing_view_state_generator`                | —                                             |
//! | `MissingEventValidation`                    | `auth.missing_event_validation`                    | —                                             |
//! | `AdvanceCheckRequired { url }`              | `auth.advance_check_required`                      | `url` (nullable)                              |
//! | `TotpRequired(..)`                          | `auth.totp_required`                               | `challenge_display` (P10.2 upgrades to full)  |
//! | `ServerMessage(..)`                         | `auth.server_rejected`                             | `server_message`                              |
//! | `SendLoginNoFormData`                       | `auth.send_login_no_form_data`                     | —                                             |
//! | `MissingAkey`                               | `auth.missing_akey`                                | —                                             |
//! | `MissingWebToken`                           | `auth.missing_web_token`                           | —                                             |
//! | `QrInitResultError`                         | `auth.qr_init_result_error`                        | —                                             |
//! | `QrJsonParseFailed`                         | `auth.qr_json_parse_failed`                        | —                                             |
//! | `QrUnsupportedRegion`                       | `auth.qr_unsupported_region`                       | —                                             |
//! | `GamepassUnsupportedRegion`                 | `auth.gamepass_unsupported_region`                 | —                                             |
//! | `DeviceRegistrationRequired { .. }`         | `auth.device_registration_required`                | `poll_url` / `param`                          |
//! | `DeviceLoginTimeout`                        | `auth.device_login_timeout`                        | —                                             |
//! | `DeviceLoginRejected`                       | `auth.device_login_rejected`                       | —                                             |
//! | `OtpMissingLongPollingKey { snippet }`      | `auth.otp_missing_long_polling_key`                | `snippet`                                     |
//! | `OtpMissingUnkData`                         | `auth.otp_missing_unk_data`                        | —                                             |
//! | `OtpMissingCreateTime`                      | `auth.otp_missing_create_time`                     | —                                             |
//! | `OtpMissingSecretCode`                      | `auth.otp_missing_secret_code`                     | —                                             |
//! | `OtpEmptyResponse`                          | `auth.otp_empty_response`                          | —                                             |
//! | `OtpServerRejected { message }`             | `auth.otp_server_rejected`                         | `server_message`                              |
//! | `OtpDecryptionFailed { cause }`             | `auth.otp_decryption_failed`                       | `cause`                                       |
//! | `VerifyMissingViewState`                    | `auth.verify_missing_view_state`                   | —                                             |
//! | `VerifyMissingEventValidation`              | `auth.verify_missing_event_validation`             | —                                             |
//! | `VerifyMissingSampleCaptcha`                | `auth.verify_missing_sample_captcha`               | —                                             |
//! | `VerifyMissingLblAuthType`                  | `auth.verify_missing_lbl_auth_type`                | —                                             |
//! | `VerifyCaptchaImageTooSmall { actual }`     | `auth.verify_captcha_image_too_small`              | `actual_bytes`                                |
//! | `AccountMgmtMissingViewState`               | `auth.account_mgmt_missing_view_state`             | —                                             |
//! | `AccountMgmtMissingViewStateGenerator`      | `auth.account_mgmt_missing_view_state_generator`   | —                                             |
//! | `AccountMgmtMissingEventValidation`         | `auth.account_mgmt_missing_event_validation`       | —                                             |
//! | `AccountMgmtMissingGameName`                | `auth.account_mgmt_missing_game_name`              | —                                             |
//! | `AccountMgmtMissingAccountLen`              | `auth.account_mgmt_missing_account_len`            | —                                             |
//! | `Http(reqwest::Error)`                      | `network.http_failed`                              | `is_timeout` / `is_connect` / `status` / `url`|
//! | `BodyTooLarge { limit, actual }`            | `network.body_too_large`                           | `limit` / `actual`                            |
//! | `Json(serde_json::Error)`                   | `network.json_decode_failed`                       | `line` / `column`                             |
//! | `Parser(ParserError)`                       | `auth.html_parse_failed`                           | `parser_variant`                              |
//! | `InvalidUrl(..)`                            | `auth.invalid_url`                                 | `url`                                         |
//! | `InvalidUtf8`                               | `auth.invalid_utf8`                                | —                                             |
//! | `Unknown(..)`                               | `auth.unknown`                                     | `detail`                                      |
//!
//! ## Inline-raised codes — `auth.*`
//!
//! Codes raised directly inside [`crate::commands::auth`] command
//! handlers (not via a [`LoginError`] `From` impl) for precondition
//! violations the service layer can't express. Listed here so the
//! frontend i18n catalogue and operator log scrapers have a single
//! source of truth.
//!
//! | Where                                                         | Code                                  | `details` fields | Note                                                                     |
//! | ------------------------------------------------------------- | ------------------------------------- | ---------------- | ------------------------------------------------------------------------ |
//! | `open_gamepass_window` — prior WebView still alive            | `auth.gamepass_window_already_open`   | —                | Distinct from `auth.gamepass_not_started` so log attribution stays clean — see [`crate::commands::auth::GAMEPASS_WINDOW_ALREADY_OPEN_CODE`]. |
//! | `login_gamepass_start` — prior WebView still alive            | `auth.gamepass_window_already_open`   | —                | Same code surfaced from the race-guard on the *other* entry point so the i18n / toast text stays uniform (remediation is identical: close the window). |
//!
//! ## Inline-raised codes — `ui.*`
//!
//! Codes raised by UI-layer plumbing in [`crate::commands::auth`]
//! when a Tauri [`tauri::WebviewWindow`] operation fails after the
//! service layer has already done its work. Distinct namespace
//! (`ui.*` vs `auth.*`) so the frontend can special-case the two:
//! `auth.*` is a flow / server error, `ui.*` is a local renderer
//! fault that usually clears on retry.
//!
//! | Where                                                         | Code                                  | `details` fields | Note                                                                     |
//! | ------------------------------------------------------------- | ------------------------------------- | ---------------- | ------------------------------------------------------------------------ |
//! | `open_gamepass_window` — `window.navigate(login_url)` failed   | `ui.gamepass_navigate_failed`         | —                | Raised *after* the `about:blank` shell + cookie seed succeed, so the pending slot is cleared and the dangling window is closed before this is returned. |
//!
//! ## `StorageError` — `storage.*`
//!
//! | Variant                            | Code                             | `details` fields                   |
//! | ---------------------------------- | -------------------------------- | ---------------------------------- |
//! | `Dpapi { operation, message }`     | `storage.dpapi_failed`           | `operation` / `win32_message`      |
//! | `Registry(io::Error)`              | `storage.registry_io_failed`     | `io_kind`                          |
//! | `EntropyMissing`                   | `storage.entropy_missing`        | —                                  |
//! | `EntropyShape`                     | `storage.entropy_shape`          | —                                  |
//! | `Io(io::Error)`                    | `storage.io_failed`              | `io_kind`                          |
//! | `AppDataMissing`                   | `storage.app_data_missing`       | —                                  |
//! | `Json(serde_json::Error)`          | `storage.json_failed`            | `line` / `column`                  |
//! | `LegacyDataDetected { raw_bytes }` | `storage.legacy_data_detected`   | `byte_count` (raw bytes omitted)   |
//!
//! ## `BackupError` — `storage.aes_backup_*`
//!
//! AES-128-CBC backup/restore failures from
//! [`crate::services::storage::aes_backup`]. The `aes_backup_` prefix
//! sits inside the `storage.*` namespace so a single match arm in the
//! frontend can branch on `code.startsWith("storage.aes_backup_")` to
//! map all three variants to the WPF `MsgDecryptFailed` toast (post-
//! decrypt JSON / DPAPI failures fall back to the regular
//! `storage.*` codes above and surface as `RecoveryFailed` instead).
//!
//! | Variant                                   | Code                                       | `details` fields            |
//! | ----------------------------------------- | ------------------------------------------ | --------------------------- |
//! | `InvalidCiphertext(base64::DecodeError)`  | `storage.aes_backup_invalid_ciphertext`    | `reason`                    |
//! | `DecryptFailed`                           | `storage.aes_backup_decrypt_failed`        | — (opaque by design)        |
//! | `InvalidUtf8(FromUtf8Error)`              | `storage.aes_backup_invalid_utf8`          | `reason`                    |
//!
//! ## `ConfigError` — `config.*`
//!
//! | Variant                       | Code                        | `details` fields |
//! | ----------------------------- | --------------------------- | ---------------- |
//! | `Io(io::Error)`               | `config.io_failed`          | `io_kind`        |
//! | `XmlParse(quick_xml::Error)`  | `config.xml_parse_failed`   | —                |
//! | `XmlWrite(io::Error)`         | `config.xml_write_failed`   | `io_kind`        |
//! | `AppDataMissing`              | `config.app_data_missing`   | —                |
//!
//! ## `ProcessError` — `process.*`
//!
//! | Variant                          | Code                                | `details` fields                  |
//! | -------------------------------- | ----------------------------------- | --------------------------------- |
//! | `WmiInit(..)`                    | `process.wmi_init_failed`           | —                                 |
//! | `WmiConnect(..)`                 | `process.wmi_connect_failed`        | —                                 |
//! | `WmiQuery { query, .. }`         | `process.wmi_query_failed`          | `query`                           |
//! | `OpenProcess { pid, .. }`        | `process.open_process_failed`       | `pid`                             |
//! | `TerminateProcess { pid }`       | `process.terminate_process_failed`  | `pid`                             |
//! | `PostMessage { hwnd, .. }`       | `process.post_message_failed`       | `hwnd`                            |
//! | `NonAscii { offset, ch }`        | `process.non_ascii`                 | `offset` / `char`                 |
//! | `Win32Call { name, .. }`         | `process.win32_call_failed`         | `win32_function`                  |
//! | `WindowNotFound { primary, .. }` | `process.window_not_found`          | `primary_class` / `fallback_class` |
//!
//! ## `RegistryError` — `registry.*`
//!
//! | Variant                          | Code                             | `details` fields                     |
//! | -------------------------------- | -------------------------------- | ------------------------------------ |
//! | `OpenKey { hive, subkey, .. }`   | `registry.open_key_failed`       | `hive` / `subkey` / `io_kind`        |
//! | `ReadValue { .., value_name }`   | `registry.read_value_failed`     | `hive` / `subkey` / `value_name` / `io_kind` |
//!
//! ## `GameError` — `game.*`
//!
//! | Variant                                 | Code                                   | `details` fields                              |
//! | --------------------------------------- | -------------------------------------- | --------------------------------------------- |
//! | `PathEmpty`                             | `game.path_empty`                      | —                                             |
//! | `PathNotFound { path }`                 | `game.path_not_found`                  | `path`                                        |
//! | `PathNonAscii { path, ch, position }`   | `game.path_non_ascii`                  | `path` / `char` / `position`                  |
//! | `LocaleRemulatorRelease { name, .. }`   | `game.locale_remulator_release_failed` | `resource`                                    |
//! | `LocaleRemulatorSha256Mismatch { .. }`  | `game.locale_remulator_sha256_mismatch`| `resource`                                    |
//! | `ShellExecute { code, .. }` (windows)   | `game.shellexecute_failed`             | `shellexecute_code`                           |
//! | `Spawn(io::Error)`                      | `game.spawn_failed`                    | `io_kind`                                     |
//!
//! ## `UpdaterError` — `update.*`
//!
//! | Variant                     | Code                         | `details` fields                          |
//! | --------------------------- | ---------------------------- | ----------------------------------------- |
//! | `Probe(..)`                 | `update.probe_failed`        | `is_timeout` / `is_connect` / `status`    |
//! | `Fetch(..)`                 | `update.fetch_failed`        | `is_timeout` / `is_connect` / `status`    |
//! | `JsonDecode(..)`            | `update.json_decode_failed`  | `line` / `column`                         |
//! | `BodyTooLarge { limit, .. }`| `update.body_too_large`      | `limit` / `actual`                        |
//! | `UnsupportedTag(tag)`       | `update.unsupported_tag`     | `tag`                                     |
//!
//! ## `SystemError` — `system.*`
//!
//! | Variant                     | Code                           | `details` fields                     |
//! | --------------------------- | ------------------------------ | ------------------------------------ |
//! | `InvalidUrl { .. }`         | `system.invalid_url`           | `url` / `reason`                     |
//! | `OpenFailed { .. }`         | `system.open_url_failed`       | `url` / `io_kind`                    |
//! | `SpawnBlockingFailed(..)`   | `system.spawn_blocking_failed` | `is_panic` / `is_cancelled`          |
//!
//! ## `MapleCacheError` — `maple_cache.*`
//!
//! | Variant                          | Code                                  | `details` fields                  |
//! | -------------------------------- | ------------------------------------- | --------------------------------- |
//! | `PathEmpty`                      | `maple_cache.path_empty`              | —                                 |
//! | `PathNoParent { path }`          | `maple_cache.path_no_parent`          | `path`                            |
//! | `PathNotFound { path }`          | `maple_cache.path_not_found`          | `path`                            |
//! | `PathNotADir { path }`           | `maple_cache.path_not_a_dir`          | `path`                            |
//! | `ReadDirFailed { path, source }` | `maple_cache.read_dir_failed`         | `path` / `io_kind`                |
//! | `SpawnBlockingFailed(..)`        | `maple_cache.spawn_blocking_failed`   | `is_panic` / `is_cancelled`       |
//!
//! ## Command-layer `system.*` codes (no domain counterpart)
//!
//! Unlike the eight tables above, these codes are minted inside the
//! command / boot layer for failures that have no `services/*`
//! counterpart (boot-time resource resolution, ad-hoc `tokio` task
//! plumbing in [`super::system::ping`]). They share the `system.*`
//! namespace with [`SystemError`] so the `code` column stays
//! globally searchable; [`super::system::ping`] in particular
//! re-uses the `system.spawn_blocking_failed` code without going
//! through [`SystemError`] (the smoke command pre-dates the service
//! layer).
//!
//! | Code                            | Origin                                                                        | `details` fields |
//! | ------------------------------- | ----------------------------------------------------------------------------- | ---------------- |
//! | `system.app_data_missing`       | [`crate::run`] — `%APPDATA%` env var is unset or empty (Windows boot).        | —                |
//! | `system.spawn_blocking_failed`  | [`super::system::ping`] ad-hoc path — same code as [`SystemError::SpawnBlockingFailed`] but minted without constructing the service error. | — |
//!
//! ## Command-layer `launcher.*` codes (no domain counterpart)
//!
//! Minted inside [`super::launcher`] for orchestration failures
//! that happen **outside** the [`GameError`] surface — i.e. setup
//! steps the service layer doesn't reach. Every launch-time
//! business failure (path validation, locale remulator release,
//! ShellExecuteW, Command::spawn) still flows through the
//! [`GameError`] / [`game.*`](#gameerror--commanderror-servicesgame)
//! table; these command-only codes cover the edges.
//!
//! | Code                                 | Origin                                                                                 | `details` fields                |
//! | ------------------------------------ | -------------------------------------------------------------------------------------- | ------------------------------- |
//! | `launcher.target_dir_resolve_failed` | [`super::launcher::launch_game`] — [`default_target_dir`][dtd] returned `io::Error`.   | `io_kind`                       |
//! | `launcher.spawn_blocking_failed`     | [`super::launcher::launch_game`] — [`tokio::task::JoinError`] from `spawn_blocking`.    | `is_panic` / `is_cancelled`     |
//!
//! [dtd]: crate::services::game::default_target_dir
//!
//! # Usage at the command boundary
//!
//! ```ignore
//! # use beanfun_lib::commands::error::CommandError;
//! # use beanfun_lib::services::beanfun::error::LoginError;
//! #[tauri::command]
//! async fn login() -> Result<(), CommandError> {
//!     // `?` converts the domain error through the `Into<CommandError>`
//!     // blanket impl produced by the `From` impls in this module.
//!     do_login().await?;
//!     Ok(())
//! }
//!
//! # async fn do_login() -> Result<(), LoginError> { Err(LoginError::MissingSessionKey) }
//! ```

use std::io;

use serde::Serialize;
use serde_json::json;
use specta::Type;

use crate::services::beanfun::error::LoginError;
use crate::services::config::error::ConfigError;
use crate::services::game::error::GameError;
use crate::services::maple_cache::error::MapleCacheError;
use crate::services::process::error::ProcessError;
use crate::services::registry::error::RegistryError;
use crate::services::storage::aes_backup::BackupError;
use crate::services::storage::error::StorageError;
use crate::services::system::error::SystemError;
use crate::services::updater::error::UpdaterError;

/// IPC-facing error DTO. Preserves a stable `{ code, message, details }`
/// shape across every Tauri command.
///
/// # Serialization contract
///
/// Always serialized as a JSON object with exactly three keys:
/// `code` (string), `message` (string), `details` (nullable JSON value).
/// Frontend types live in
/// `src/types/bindings.ts` and are auto-generated by
/// `tauri-specta` (P10 chunk 10.1 D8).
///
/// # Construction
///
/// Use [`CommandError::new`] for the minimum required pair and chain
/// [`CommandError::with_details`] when the domain has extra structured
/// context worth exposing to the UI.
///
/// # Display / Error
///
/// [`Display`][std::fmt::Display] formats as `[code] message` for
/// `tracing` logs; the type also implements [`std::error::Error`] so
/// it composes with existing `?` / `anyhow` call sites if a command
/// needs to chain through additional fallible ops before surfacing.
#[derive(Debug, Clone, Serialize, Type)]
pub struct CommandError {
    /// Stable `<domain>.<variant_snake_case>` identifier — see
    /// [module-level docs](self#code-naming) for the full mapping table.
    pub code: String,
    /// Human-readable, non-localized Rust-side description sourced from
    /// the domain error's `Display` impl. Safe to
    /// `tracing::error!(%err)`; never contains secrets — the domain
    /// layer is responsible for redaction (see P8.2 R8.2-1
    /// `LaunchRequest` as the template).
    pub message: String,
    /// Optional structured context — the domain may attach any
    /// JSON-representable payload (flat objects preferred). The
    /// frontend treats this field as `unknown`-shaped and branches on
    /// `code` before reading fields.
    pub details: Option<serde_json::Value>,
}

impl CommandError {
    /// Build a new [`CommandError`] with no `details`.
    ///
    /// `code` **must** follow the module-level naming convention.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Attach (or overwrite) the `details` field, consuming the
    /// builder. Accepts anything that serializes via
    /// [`serde_json::to_value`]; serialization failure degrades
    /// gracefully to `details = None` rather than losing the error —
    /// the primary `{ code, message }` contract is preserved no matter
    /// what.
    pub fn with_details<T: Serialize>(mut self, details: T) -> Self {
        self.details = serde_json::to_value(details).ok();
        self
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

// ---------------------------------------------------------------------
// Small helpers shared by multiple `From` impls. Kept private so the
// encoding quirks (reqwest flag extraction, io_kind stringification)
// stay quarantined to this module.
// ---------------------------------------------------------------------

/// Stringify an [`io::ErrorKind`] via its `Debug` impl.
/// [`io::ErrorKind`] is `#[non_exhaustive]` and does not implement
/// `Display` nor serde; its `Debug` form ("NotFound",
/// "PermissionDenied", …) is stable enough for diagnostic display.
fn io_kind_str(err: &io::Error) -> String {
    format!("{:?}", err.kind())
}

/// Extract the standard reqwest signal flags into a flat JSON object
/// consumable by the frontend. Used by every variant that wraps a
/// [`reqwest::Error`] (LoginError::Http, UpdaterError::Probe/Fetch).
fn reqwest_details(err: &reqwest::Error) -> serde_json::Value {
    json!({
        "is_timeout": err.is_timeout(),
        "is_connect": err.is_connect(),
        "is_request": err.is_request(),
        "status": err.status().map(|s| s.as_u16()),
        "url": err.url().map(|u| u.as_str()),
    })
}

/// Extract line / column from a [`serde_json::Error`] (1-indexed, as
/// reported by the crate). Position is `Some(0, 0)` for category
/// mismatches where no offset is available — the frontend can branch
/// on `0/0` if it cares.
fn serde_json_details(err: &serde_json::Error) -> serde_json::Value {
    json!({
        "line": err.line(),
        "column": err.column(),
    })
}

// ---------------------------------------------------------------------
// LoginError → CommandError (services/beanfun)
// ---------------------------------------------------------------------

impl From<LoginError> for CommandError {
    fn from(e: LoginError) -> Self {
        // Capture Display output *before* moving `e` into the match so
        // every arm can reuse the same `message` string without
        // rebuilding it per arm.
        let message = e.to_string();
        match e {
            LoginError::MissingSessionKey => CommandError::new("auth.missing_session_key", message),
            LoginError::EmptyResponse => CommandError::new("auth.empty_response", message),
            LoginError::MissingVerificationToken => {
                CommandError::new("auth.missing_verification_token", message)
            }
            LoginError::MissingViewState => CommandError::new("auth.missing_view_state", message),
            LoginError::MissingViewStateGenerator => {
                CommandError::new("auth.missing_view_state_generator", message)
            }
            LoginError::MissingEventValidation => {
                CommandError::new("auth.missing_event_validation", message)
            }
            LoginError::AdvanceCheckRequired { url } => {
                CommandError::new("auth.advance_check_required", message)
                    .with_details(json!({ "url": url }))
            }
            LoginError::TotpRequired(challenge) => {
                // P10.1 keeps the payload compact (`challenge_display`
                // only). P10.2 will upgrade `TotpChallenge` to
                // `Serialize + specta::Type` and inline `viewstate` /
                // `url` so `login_totp` can round-trip structured data.
                CommandError::new("auth.totp_required", message)
                    .with_details(json!({ "challenge_display": format!("{challenge:?}") }))
            }
            LoginError::ServerMessage(text) => CommandError::new("auth.server_rejected", message)
                .with_details(json!({ "server_message": text })),
            LoginError::SendLoginNoFormData => {
                CommandError::new("auth.send_login_no_form_data", message)
            }
            LoginError::MissingAkey => CommandError::new("auth.missing_akey", message),
            LoginError::MissingWebToken => CommandError::new("auth.missing_web_token", message),
            LoginError::QrInitResultError => {
                CommandError::new("auth.qr_init_result_error", message)
            }
            LoginError::QrJsonParseFailed => {
                CommandError::new("auth.qr_json_parse_failed", message)
            }
            LoginError::QrUnsupportedRegion => {
                CommandError::new("auth.qr_unsupported_region", message)
            }
            LoginError::GamepassUnsupportedRegion => {
                CommandError::new("auth.gamepass_unsupported_region", message)
            }
            LoginError::DeviceRegistrationRequired {
                poll_url,
                param,
                ..
            } => CommandError::new("auth.device_registration_required", message).with_details(
                json!({
                    "poll_url": poll_url,
                    "param": param,
                }),
            ),
            LoginError::DeviceLoginTimeout => {
                CommandError::new("auth.device_login_timeout", message)
            }
            LoginError::DeviceLoginRejected => {
                CommandError::new("auth.device_login_rejected", message)
            }
            LoginError::OtpMissingLongPollingKey { snippet } => {
                CommandError::new("auth.otp_missing_long_polling_key", message)
                    .with_details(json!({ "snippet": snippet }))
            }
            LoginError::OtpMissingUnkData => {
                CommandError::new("auth.otp_missing_unk_data", message)
            }
            LoginError::OtpMissingCreateTime => {
                CommandError::new("auth.otp_missing_create_time", message)
            }
            LoginError::OtpMissingSecretCode => {
                CommandError::new("auth.otp_missing_secret_code", message)
            }
            LoginError::OtpEmptyResponse => CommandError::new("auth.otp_empty_response", message),
            LoginError::OtpServerRejected { message: server } => {
                CommandError::new("auth.otp_server_rejected", message)
                    .with_details(json!({ "server_message": server }))
            }
            LoginError::OtpDecryptionFailed { cause } => {
                CommandError::new("auth.otp_decryption_failed", message)
                    .with_details(json!({ "cause": cause }))
            }
            LoginError::VerifyMissingViewState => {
                CommandError::new("auth.verify_missing_view_state", message)
            }
            LoginError::VerifyMissingEventValidation => {
                CommandError::new("auth.verify_missing_event_validation", message)
            }
            LoginError::VerifyMissingSampleCaptcha => {
                CommandError::new("auth.verify_missing_sample_captcha", message)
            }
            LoginError::VerifyMissingLblAuthType => {
                CommandError::new("auth.verify_missing_lbl_auth_type", message)
            }
            LoginError::VerifyCaptchaImageTooSmall { actual } => {
                CommandError::new("auth.verify_captcha_image_too_small", message)
                    .with_details(json!({ "actual_bytes": actual }))
            }
            LoginError::AccountMgmtMissingViewState => {
                CommandError::new("auth.account_mgmt_missing_view_state", message)
            }
            LoginError::AccountMgmtMissingViewStateGenerator => {
                CommandError::new("auth.account_mgmt_missing_view_state_generator", message)
            }
            LoginError::AccountMgmtMissingEventValidation => {
                CommandError::new("auth.account_mgmt_missing_event_validation", message)
            }
            LoginError::AccountMgmtMissingGameName => {
                CommandError::new("auth.account_mgmt_missing_game_name", message)
            }
            LoginError::AccountMgmtMissingAccountLen => {
                CommandError::new("auth.account_mgmt_missing_account_len", message)
            }
            LoginError::GameListServiceListMissing => {
                CommandError::new("game.service_list_missing", message)
            }
            LoginError::Http(err) => {
                let details = reqwest_details(&err);
                CommandError::new("network.http_failed", message).with_details(details)
            }
            LoginError::BodyTooLarge { limit, actual } => {
                CommandError::new("network.body_too_large", message)
                    .with_details(json!({ "limit": limit, "actual": actual }))
            }
            LoginError::Json(err) => {
                let details = serde_json_details(&err);
                CommandError::new("network.json_decode_failed", message).with_details(details)
            }
            LoginError::Parser(err) => {
                // `ParserError` is a small unit-variant enum — encode
                // its variant name via `Debug` so the frontend can
                // branch on "MissingViewState" / "MissingAkey" /
                // "MissingRequestVerificationToken" without Specta
                // needing to know the full type.
                let variant = format!("{err:?}");
                CommandError::new("auth.html_parse_failed", message)
                    .with_details(json!({ "parser_variant": variant }))
            }
            LoginError::InvalidUrl(url) => {
                CommandError::new("auth.invalid_url", message).with_details(json!({ "url": url }))
            }
            LoginError::InvalidUtf8 => CommandError::new("auth.invalid_utf8", message),
            LoginError::Unknown(detail) => {
                CommandError::new("auth.unknown", message).with_details(json!({ "detail": detail }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// StorageError → CommandError (services/storage)
// ---------------------------------------------------------------------

impl From<StorageError> for CommandError {
    fn from(e: StorageError) -> Self {
        let message = e.to_string();
        match e {
            StorageError::Dpapi {
                operation,
                message: win32,
            } => CommandError::new("storage.dpapi_failed", message)
                .with_details(json!({ "operation": operation, "win32_message": win32 })),
            StorageError::Registry(err) => {
                let kind = io_kind_str(&err);
                CommandError::new("storage.registry_io_failed", message)
                    .with_details(json!({ "io_kind": kind }))
            }
            StorageError::EntropyMissing => CommandError::new("storage.entropy_missing", message),
            StorageError::EntropyShape => CommandError::new("storage.entropy_shape", message),
            StorageError::Io(err) => {
                let kind = io_kind_str(&err);
                CommandError::new("storage.io_failed", message)
                    .with_details(json!({ "io_kind": kind }))
            }
            StorageError::AppDataMissing => CommandError::new("storage.app_data_missing", message),
            StorageError::Json(err) => {
                let details = serde_json_details(&err);
                CommandError::new("storage.json_failed", message).with_details(details)
            }
            StorageError::LegacyDataDetected { raw_bytes } => {
                // Intentionally omit `raw_bytes` from `details` — the
                // legacy ciphertext can be large (hundreds of KB) and
                // may carry sensitive remnants. Only the byte count is
                // surfaced; the command layer retains the full buffer
                // for the subsequent NRBF migration pass.
                CommandError::new("storage.legacy_data_detected", message)
                    .with_details(json!({ "byte_count": raw_bytes.len() }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// BackupError → CommandError (services/storage/aes_backup)
// ---------------------------------------------------------------------

impl From<BackupError> for CommandError {
    fn from(e: BackupError) -> Self {
        let message = e.to_string();
        match e {
            BackupError::InvalidCiphertext(err) => {
                CommandError::new("storage.aes_backup_invalid_ciphertext", message)
                    .with_details(json!({ "reason": err.to_string() }))
            }
            // No `details` — the inner `cipher::block_padding::UnpadError`
            // is intentionally opaque (RustCrypto's design choice to
            // resist padding-oracle leakage). Adding a synthetic reason
            // here would re-introduce that signal.
            BackupError::DecryptFailed => {
                CommandError::new("storage.aes_backup_decrypt_failed", message)
            }
            BackupError::InvalidUtf8(err) => {
                CommandError::new("storage.aes_backup_invalid_utf8", message)
                    .with_details(json!({ "reason": err.to_string() }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// ConfigError → CommandError (services/config)
// ---------------------------------------------------------------------

impl From<ConfigError> for CommandError {
    fn from(e: ConfigError) -> Self {
        let message = e.to_string();
        match e {
            ConfigError::Io(err) => {
                let kind = io_kind_str(&err);
                CommandError::new("config.io_failed", message)
                    .with_details(json!({ "io_kind": kind }))
            }
            ConfigError::XmlParse(_) => CommandError::new("config.xml_parse_failed", message),
            ConfigError::XmlWrite(err) => {
                let kind = io_kind_str(&err);
                CommandError::new("config.xml_write_failed", message)
                    .with_details(json!({ "io_kind": kind }))
            }
            ConfigError::AppDataMissing => CommandError::new("config.app_data_missing", message),
        }
    }
}

// ---------------------------------------------------------------------
// ProcessError → CommandError (services/process)
// ---------------------------------------------------------------------

impl From<ProcessError> for CommandError {
    fn from(e: ProcessError) -> Self {
        let message = e.to_string();
        match e {
            ProcessError::WmiInit(_) => CommandError::new("process.wmi_init_failed", message),
            ProcessError::WmiConnect(_) => CommandError::new("process.wmi_connect_failed", message),
            ProcessError::WmiQuery { query, .. } => {
                CommandError::new("process.wmi_query_failed", message)
                    .with_details(json!({ "query": query }))
            }
            ProcessError::OpenProcess { pid, .. } => {
                CommandError::new("process.open_process_failed", message)
                    .with_details(json!({ "pid": pid }))
            }
            ProcessError::TerminateProcess { pid, .. } => {
                CommandError::new("process.terminate_process_failed", message)
                    .with_details(json!({ "pid": pid }))
            }
            ProcessError::PostMessage { hwnd, .. } => {
                CommandError::new("process.post_message_failed", message)
                    .with_details(json!({ "hwnd": hwnd }))
            }
            ProcessError::NonAscii { offset, ch } => {
                CommandError::new("process.non_ascii", message)
                    .with_details(json!({ "offset": offset, "char": ch.to_string() }))
            }
            ProcessError::Win32Call { name, .. } => {
                CommandError::new("process.win32_call_failed", message)
                    .with_details(json!({ "win32_function": name }))
            }
            ProcessError::WindowNotFound {
                primary_class,
                fallback_class,
            } => CommandError::new("process.window_not_found", message).with_details(json!({
                "primary_class": primary_class,
                "fallback_class": fallback_class,
            })),
        }
    }
}

// ---------------------------------------------------------------------
// RegistryError → CommandError (services/registry)
// ---------------------------------------------------------------------

impl From<RegistryError> for CommandError {
    fn from(e: RegistryError) -> Self {
        let message = e.to_string();
        match e {
            RegistryError::OpenKey {
                hive,
                subkey,
                source,
            } => {
                let kind = io_kind_str(&source);
                CommandError::new("registry.open_key_failed", message).with_details(json!({
                    "hive": hive.display_name(),
                    "subkey": subkey,
                    "io_kind": kind,
                }))
            }
            RegistryError::ReadValue {
                hive,
                subkey,
                value_name,
                source,
            } => {
                let kind = io_kind_str(&source);
                CommandError::new("registry.read_value_failed", message).with_details(json!({
                    "hive": hive.display_name(),
                    "subkey": subkey,
                    "value_name": value_name,
                    "io_kind": kind,
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// GameError → CommandError (services/game)
// ---------------------------------------------------------------------

impl From<GameError> for CommandError {
    fn from(e: GameError) -> Self {
        let message = e.to_string();
        match e {
            GameError::PathEmpty => CommandError::new("game.path_empty", message),
            GameError::PathNotFound { path } => CommandError::new("game.path_not_found", message)
                .with_details(json!({ "path": path.display().to_string() })),
            GameError::PathNonAscii {
                path,
                offending_char,
                position,
            } => CommandError::new("game.path_non_ascii", message).with_details(json!({
                "path": path.display().to_string(),
                "char": offending_char.to_string(),
                "position": position,
            })),
            GameError::LocaleRemulatorRelease { name, .. } => {
                CommandError::new("game.locale_remulator_release_failed", message)
                    .with_details(json!({ "resource": name }))
            }
            GameError::LocaleRemulatorSha256Mismatch { name } => {
                CommandError::new("game.locale_remulator_sha256_mismatch", message)
                    .with_details(json!({ "resource": name }))
            }
            #[cfg(windows)]
            GameError::ShellExecute { code, .. } => {
                CommandError::new("game.shellexecute_failed", message)
                    .with_details(json!({ "shellexecute_code": code }))
            }
            GameError::Spawn(err) => {
                let kind = io_kind_str(&err);
                CommandError::new("game.spawn_failed", message)
                    .with_details(json!({ "io_kind": kind }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// UpdaterError → CommandError (services/updater)
// ---------------------------------------------------------------------

impl From<UpdaterError> for CommandError {
    fn from(e: UpdaterError) -> Self {
        let message = e.to_string();
        match e {
            UpdaterError::Probe(err) => {
                let details = reqwest_details(&err);
                CommandError::new("update.probe_failed", message).with_details(details)
            }
            UpdaterError::Fetch(err) => {
                let details = reqwest_details(&err);
                CommandError::new("update.fetch_failed", message).with_details(details)
            }
            UpdaterError::JsonDecode(err) => {
                let details = serde_json_details(&err);
                CommandError::new("update.json_decode_failed", message).with_details(details)
            }
            UpdaterError::BodyTooLarge { limit, actual } => {
                CommandError::new("update.body_too_large", message)
                    .with_details(json!({ "limit": limit, "actual": actual }))
            }
            UpdaterError::UnsupportedTag(tag) => {
                CommandError::new("update.unsupported_tag", message)
                    .with_details(json!({ "tag": tag }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// SystemError → CommandError (services/system — open_url, future
// open_folder / reveal_in_finder)
// ---------------------------------------------------------------------

impl From<SystemError> for CommandError {
    fn from(e: SystemError) -> Self {
        let message = e.to_string();
        match e {
            SystemError::InvalidUrl { url, reason } => {
                CommandError::new("system.invalid_url", message)
                    .with_details(json!({ "url": url, "reason": reason }))
            }
            SystemError::OpenFailed { url, source } => {
                CommandError::new("system.open_url_failed", message).with_details(json!({
                    "url": url,
                    "io_kind": io_kind_str(&source),
                }))
            }
            SystemError::SpawnBlockingFailed(join_err) => {
                CommandError::new("system.spawn_blocking_failed", message).with_details(json!({
                    "is_panic": join_err.is_panic(),
                    "is_cancelled": join_err.is_cancelled(),
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------
// MapleCacheError → CommandError (services/maple_cache — sweep
// MapleStory's per-launch cache from the game install dir)
// ---------------------------------------------------------------------

impl From<MapleCacheError> for CommandError {
    fn from(e: MapleCacheError) -> Self {
        let message = e.to_string();
        match e {
            MapleCacheError::PathEmpty => CommandError::new("maple_cache.path_empty", message),
            MapleCacheError::PathNoParent { path } => {
                CommandError::new("maple_cache.path_no_parent", message)
                    .with_details(json!({ "path": path }))
            }
            MapleCacheError::PathNotFound { path } => {
                CommandError::new("maple_cache.path_not_found", message)
                    .with_details(json!({ "path": path.display().to_string() }))
            }
            MapleCacheError::PathNotADir { path } => {
                CommandError::new("maple_cache.path_not_a_dir", message)
                    .with_details(json!({ "path": path.display().to_string() }))
            }
            MapleCacheError::ReadDirFailed { path, source } => {
                let kind = io_kind_str(&source);
                CommandError::new("maple_cache.read_dir_failed", message).with_details(json!({
                    "path": path.display().to_string(),
                    "io_kind": kind,
                }))
            }
            MapleCacheError::SpawnBlockingFailed(join_err) => {
                CommandError::new("maple_cache.spawn_blocking_failed", message).with_details(
                    json!({
                        "is_panic": join_err.is_panic(),
                        "is_cancelled": join_err.is_cancelled(),
                    }),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // D3 — `CommandError` struct + builder (P10.1)
    // -----------------------------------------------------------------

    #[test]
    fn new_sets_code_and_message_with_no_details() {
        let err = CommandError::new("auth.invalid_credentials", "bad creds");
        assert_eq!(err.code, "auth.invalid_credentials");
        assert_eq!(err.message, "bad creds");
        assert!(err.details.is_none());
    }

    #[test]
    fn with_details_attaches_structured_context() {
        let err = CommandError::new("network.timeout", "request timed out")
            .with_details(json!({ "elapsed_ms": 30_000 }));
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("elapsed_ms")),
            Some(&json!(30_000))
        );
    }

    #[test]
    fn with_details_accepts_arbitrary_serialize() {
        #[derive(Serialize)]
        struct Ctx {
            pid: u32,
            exit_code: i32,
        }
        let err = CommandError::new("process.kill_failed", "terminate failed").with_details(Ctx {
            pid: 1234,
            exit_code: -1,
        });
        let details = err.details.expect("details should be Some");
        assert_eq!(details.get("pid"), Some(&json!(1234)));
        assert_eq!(details.get("exit_code"), Some(&json!(-1)));
    }

    #[test]
    fn display_is_bracketed_code_then_message() {
        let err = CommandError::new("storage.io_failed", "disk full");
        assert_eq!(format!("{err}"), "[storage.io_failed] disk full");
    }

    #[test]
    fn implements_std_error_trait() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        let err = CommandError::new("auth.invalid_credentials", "bad creds");
        assert_error(&err);
    }

    #[test]
    fn serializes_as_flat_json_with_three_keys() {
        let err = CommandError::new("network.timeout", "timeout")
            .with_details(json!({ "retry_after_s": 5 }));
        let jv = serde_json::to_value(&err).expect("serializable");
        assert_eq!(jv.get("code"), Some(&json!("network.timeout")));
        assert_eq!(jv.get("message"), Some(&json!("timeout")));
        assert_eq!(jv.get("details"), Some(&json!({ "retry_after_s": 5 })));
        let obj = jv.as_object().expect("object");
        assert_eq!(obj.len(), 3, "exactly three keys: code / message / details");
    }

    #[test]
    fn serializes_details_as_null_when_absent() {
        let err = CommandError::new("auth.logged_out", "session expired");
        let jv = serde_json::to_value(&err).expect("serializable");
        assert_eq!(jv.get("details"), Some(&json!(null)));
    }
}

#[cfg(test)]
mod from_impls_tests {
    //! Representative coverage for the seven domain `From` impls.
    //!
    //! One to three cases per domain, prioritizing:
    //!
    //! - **Unit variants** (no fields) — verifies
    //!   `code` + `message` + `details = None` baseline.
    //! - **Variants with structured fields** — verifies every field
    //!   appears in `details` under the documented name (the naming
    //!   table in the module-level doc is the source of truth).
    //! - **Variants with transport types** (`io::Error`,
    //!   `serde_json::Error`) — verifies the helper fns
    //!   (`io_kind_str`, `serde_json_details`) plug in correctly.
    //!
    //! Variants wrapping external errors that are genuinely hard to
    //! construct in tests (e.g. `reqwest::Error`, `windows::core::Error`,
    //! `wmi::WMIError`) are deferred to the integration tier
    //! (P10.1 D11 / future P10.2 HTTP smoke tests) — the code paths
    //! are exercised by the same shared helpers already covered here
    //! via `serde_json_details` / `io_kind_str`, so compile-time
    //! coverage is complete.

    use super::*;
    use crate::services::beanfun::error::LoginError;
    use crate::services::config::error::ConfigError;
    use crate::services::game::error::GameError;
    use crate::services::maple_cache::error::MapleCacheError;
    use crate::services::process::error::ProcessError;
    use crate::services::registry::error::RegistryError;
    use crate::services::registry::Hive;
    use crate::services::storage::aes_backup::BackupError;
    use crate::services::storage::error::StorageError;
    use crate::services::updater::error::UpdaterError;
    use std::io;

    // ----- LoginError ------------------------------------------------

    #[test]
    fn login_missing_session_key_has_no_details() {
        let err: CommandError = LoginError::MissingSessionKey.into();
        assert_eq!(err.code, "auth.missing_session_key");
        assert!(err.details.is_none());
    }

    /// P12.1 D5a CP1 wire-string contract: the GamePass region
    /// guard surfaces a dedicated typed code so the Vue layer can
    /// localize it differently from the QR sibling. Pin both the
    /// code and the absence of a `details` blob (no leaked region
    /// echo / context — same shape as `qr_unsupported_region`).
    #[test]
    fn login_gamepass_unsupported_region_has_no_details() {
        let err: CommandError = LoginError::GamepassUnsupportedRegion.into();
        assert_eq!(err.code, "auth.gamepass_unsupported_region");
        assert!(err.details.is_none());
    }

    #[test]
    fn login_advance_check_required_carries_url() {
        let err: CommandError = LoginError::AdvanceCheckRequired {
            url: Some("https://example.com/advance".to_string()),
        }
        .into();
        assert_eq!(err.code, "auth.advance_check_required");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("url")),
            Some(&json!("https://example.com/advance"))
        );
    }

    #[test]
    fn login_advance_check_required_with_no_url_serializes_null() {
        let err: CommandError = LoginError::AdvanceCheckRequired { url: None }.into();
        assert_eq!(err.code, "auth.advance_check_required");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("url")),
            Some(&json!(null))
        );
    }

    #[test]
    fn login_server_message_renames_to_server_message_field() {
        let err: CommandError = LoginError::ServerMessage("帳號已被鎖定".into()).into();
        assert_eq!(err.code, "auth.server_rejected");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("server_message")),
            Some(&json!("帳號已被鎖定"))
        );
    }

    #[test]
    fn login_body_too_large_carries_limit_and_actual() {
        let err: CommandError = LoginError::BodyTooLarge {
            limit: 1024,
            actual: 2048,
        }
        .into();
        assert_eq!(err.code, "network.body_too_large");
        let details = err.details.expect("details present");
        assert_eq!(details.get("limit"), Some(&json!(1024)));
        assert_eq!(details.get("actual"), Some(&json!(2048)));
    }

    #[test]
    fn login_json_decode_plugs_into_serde_json_details() {
        let serde_err = serde_json::from_str::<u32>("bad").unwrap_err();
        let err: CommandError = LoginError::Json(serde_err).into();
        assert_eq!(err.code, "network.json_decode_failed");
        let details = err.details.expect("details present");
        assert!(details.get("line").is_some(), "line must be set");
        assert!(details.get("column").is_some(), "column must be set");
    }

    #[test]
    fn login_otp_server_rejected_renames_message_to_server_message() {
        let err: CommandError = LoginError::OtpServerRejected {
            message: "OTP expired".into(),
        }
        .into();
        assert_eq!(err.code, "auth.otp_server_rejected");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("server_message")),
            Some(&json!("OTP expired"))
        );
    }

    #[test]
    fn device_registration_required_does_not_leak_login_token() {
        let err: CommandError = LoginError::DeviceRegistrationRequired {
            login_token: "SENSITIVE_TOKEN_VALUE".into(),
            poll_url: "https://beanfun.com/poll".into(),
            param: "some_param".into(),
        }
        .into();
        assert_eq!(err.code, "auth.device_registration_required");
        let details = err.details.expect("details present");
        assert!(
            details.get("login_token").is_none(),
            "login_token must not leak to frontend: {details}",
        );
        assert_eq!(details.get("poll_url"), Some(&json!("https://beanfun.com/poll")));
        assert_eq!(details.get("param"), Some(&json!("some_param")));
    }

    // ----- StorageError ----------------------------------------------

    #[test]
    fn storage_dpapi_carries_operation_and_win32_message() {
        let err: CommandError = StorageError::Dpapi {
            operation: "Protect",
            message: "NTE_BAD_DATA".into(),
        }
        .into();
        assert_eq!(err.code, "storage.dpapi_failed");
        let details = err.details.expect("details present");
        assert_eq!(details.get("operation"), Some(&json!("Protect")));
        assert_eq!(details.get("win32_message"), Some(&json!("NTE_BAD_DATA")));
    }

    #[test]
    fn storage_app_data_missing_has_no_details() {
        let err: CommandError = StorageError::AppDataMissing.into();
        assert_eq!(err.code, "storage.app_data_missing");
        assert!(err.details.is_none());
    }

    #[test]
    fn storage_legacy_data_detected_exposes_byte_count_without_raw_bytes() {
        // raw_bytes must never leak to the frontend — the legacy blob
        // may carry sensitive remnants and can be 100s of KB.
        let err: CommandError = StorageError::LegacyDataDetected {
            raw_bytes: vec![1_u8, 2, 3, 4, 5],
        }
        .into();
        assert_eq!(err.code, "storage.legacy_data_detected");
        let details = err.details.expect("details present");
        assert_eq!(details.get("byte_count"), Some(&json!(5)));
        assert!(
            details.get("raw_bytes").is_none(),
            "raw bytes must not be exposed to frontend"
        );
    }

    #[test]
    fn storage_io_failed_stringifies_io_kind() {
        let err: CommandError = StorageError::Io(io::Error::from(io::ErrorKind::NotFound)).into();
        assert_eq!(err.code, "storage.io_failed");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("io_kind")),
            Some(&json!("NotFound"))
        );
    }

    // ----- BackupError (storage.aes_backup_*) -------------------------

    #[test]
    fn backup_invalid_ciphertext_carries_reason_string() {
        let decode_err =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, "not!base64@")
                .unwrap_err();
        let err: CommandError = BackupError::InvalidCiphertext(decode_err).into();
        assert_eq!(err.code, "storage.aes_backup_invalid_ciphertext");
        let details = err.details.expect("details present");
        assert!(
            details
                .get("reason")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty()),
            "reason must be a non-empty string sourced from base64::DecodeError",
        );
    }

    #[test]
    fn backup_decrypt_failed_is_opaque() {
        let err: CommandError = BackupError::DecryptFailed.into();
        assert_eq!(err.code, "storage.aes_backup_decrypt_failed");
        assert!(
            err.details.is_none(),
            "DecryptFailed must surface no details — opaque by design (padding-oracle leakage avoidance)",
        );
    }

    #[test]
    fn backup_invalid_utf8_carries_reason_string() {
        let utf8_err = String::from_utf8(vec![0xff, 0xfe, 0xfd]).unwrap_err();
        let err: CommandError = BackupError::InvalidUtf8(utf8_err).into();
        assert_eq!(err.code, "storage.aes_backup_invalid_utf8");
        let details = err.details.expect("details present");
        assert!(
            details
                .get("reason")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty()),
            "reason must be a non-empty string sourced from FromUtf8Error",
        );
    }

    // ----- ConfigError -----------------------------------------------

    #[test]
    fn config_io_failed_stringifies_io_kind() {
        let err: CommandError =
            ConfigError::Io(io::Error::from(io::ErrorKind::PermissionDenied)).into();
        assert_eq!(err.code, "config.io_failed");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("io_kind")),
            Some(&json!("PermissionDenied"))
        );
    }

    #[test]
    fn config_app_data_missing_has_no_details() {
        let err: CommandError = ConfigError::AppDataMissing.into();
        assert_eq!(err.code, "config.app_data_missing");
        assert!(err.details.is_none());
    }

    // ----- ProcessError ----------------------------------------------

    #[test]
    fn process_non_ascii_carries_offset_and_char() {
        let err: CommandError = ProcessError::NonAscii {
            offset: 7,
            ch: '中',
        }
        .into();
        assert_eq!(err.code, "process.non_ascii");
        let details = err.details.expect("details present");
        assert_eq!(details.get("offset"), Some(&json!(7)));
        assert_eq!(details.get("char"), Some(&json!("中")));
    }

    #[test]
    fn process_window_not_found_carries_primary_and_fallback_classes() {
        let err: CommandError = ProcessError::WindowNotFound {
            primary_class: "MapleStoryClass".into(),
            fallback_class: Some("MapleStoryClassTW".into()),
        }
        .into();
        assert_eq!(err.code, "process.window_not_found");
        let details = err.details.expect("details present");
        assert_eq!(
            details.get("primary_class"),
            Some(&json!("MapleStoryClass"))
        );
        assert_eq!(
            details.get("fallback_class"),
            Some(&json!("MapleStoryClassTW"))
        );
    }

    #[test]
    fn process_window_not_found_serializes_null_fallback_when_absent() {
        let err: CommandError = ProcessError::WindowNotFound {
            primary_class: "NexonGameClass".into(),
            fallback_class: None,
        }
        .into();
        assert_eq!(err.code, "process.window_not_found");
        let details = err.details.expect("details present");
        assert_eq!(details.get("primary_class"), Some(&json!("NexonGameClass")));
        assert_eq!(details.get("fallback_class"), Some(&json!(null)));
    }

    // ----- RegistryError ---------------------------------------------

    #[test]
    fn registry_open_key_carries_hive_display_name_and_io_kind() {
        let err: CommandError = RegistryError::OpenKey {
            hive: Hive::CurrentUser,
            subkey: r"Software\Beanfun".into(),
            source: io::Error::from(io::ErrorKind::NotFound),
        }
        .into();
        assert_eq!(err.code, "registry.open_key_failed");
        let details = err.details.expect("details present");
        assert_eq!(details.get("hive"), Some(&json!("HKEY_CURRENT_USER")));
        assert_eq!(details.get("subkey"), Some(&json!(r"Software\Beanfun")));
        assert_eq!(details.get("io_kind"), Some(&json!("NotFound")));
    }

    // ----- GameError -------------------------------------------------

    #[test]
    fn game_path_empty_has_no_details() {
        let err: CommandError = GameError::PathEmpty.into();
        assert_eq!(err.code, "game.path_empty");
        assert!(err.details.is_none());
    }

    #[test]
    fn game_path_not_found_carries_stringified_path() {
        let err: CommandError = GameError::PathNotFound {
            path: std::path::PathBuf::from(r"C:\Games\missing.exe"),
        }
        .into();
        assert_eq!(err.code, "game.path_not_found");
        let details = err.details.expect("details present");
        assert_eq!(details.get("path"), Some(&json!(r"C:\Games\missing.exe")));
    }

    // ----- MapleCacheError -------------------------------------------

    #[test]
    fn maple_cache_path_empty_has_no_details() {
        let err: CommandError = MapleCacheError::PathEmpty.into();
        assert_eq!(err.code, "maple_cache.path_empty");
        assert!(err.details.is_none());
    }

    #[test]
    fn maple_cache_path_not_found_carries_stringified_path() {
        let err: CommandError = MapleCacheError::PathNotFound {
            path: std::path::PathBuf::from(r"C:\Games\MapleStory"),
        }
        .into();
        assert_eq!(err.code, "maple_cache.path_not_found");
        let details = err.details.expect("details present");
        assert_eq!(details.get("path"), Some(&json!(r"C:\Games\MapleStory")));
    }

    #[test]
    fn maple_cache_read_dir_failed_stringifies_io_kind_and_path() {
        let err: CommandError = MapleCacheError::ReadDirFailed {
            path: std::path::PathBuf::from(r"C:\Games\MapleStory"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        }
        .into();
        assert_eq!(err.code, "maple_cache.read_dir_failed");
        let details = err.details.expect("details present");
        assert_eq!(details.get("path"), Some(&json!(r"C:\Games\MapleStory")));
        assert_eq!(details.get("io_kind"), Some(&json!("PermissionDenied")));
    }

    // ----- UpdaterError ----------------------------------------------

    #[test]
    fn updater_body_too_large_carries_limit_and_actual() {
        let err: CommandError = UpdaterError::BodyTooLarge {
            limit: 5_242_880,
            actual: 10_000_000,
        }
        .into();
        assert_eq!(err.code, "update.body_too_large");
        let details = err.details.expect("details present");
        assert_eq!(details.get("limit"), Some(&json!(5_242_880)));
        assert_eq!(details.get("actual"), Some(&json!(10_000_000)));
    }

    #[test]
    fn updater_unsupported_tag_carries_tag() {
        let err: CommandError = UpdaterError::UnsupportedTag("v100".into()).into();
        assert_eq!(err.code, "update.unsupported_tag");
        assert_eq!(
            err.details.as_ref().and_then(|v| v.get("tag")),
            Some(&json!("v100"))
        );
    }
}
