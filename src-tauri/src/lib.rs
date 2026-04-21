//! Beanfun-next Tauri runtime entry point.
//!
//! Wires the three module trees into a running desktop app:
//!
//! - [`core`] — framework-agnostic primitives (parsers, DPAPI,
//!   NRBF, TLV, …).
//! - [`services`] — framework-agnostic domain layer (Beanfun HTTP,
//!   storage, config, process, registry, game, updater).
//! - [`commands`] — thin IPC boundary between the frontend and the
//!   services, surfaced via `#[tauri::command]`.
//!
//! # Boot sequence (P10.1 D7)
//!
//! ```text
//! main()
//!   └─ run()
//!       1. resolve_storage_root()      → PathBuf or fatal exit
//!       2. AppState::new(root)         → shared runtime state
//!       3. commands::build_specta_builder() → tauri-specta Builder
//!       4. tauri::Builder::default()
//!           .plugin(tauri_plugin_opener)   ← cross-platform URL opener
//!           .plugin(tauri_plugin_dialog)   ← native open / save file picker
//!           .manage(app_state)             ← State<'_, AppState> in every cmd
//!           .invoke_handler(specta.invoke_handler())
//!           .run(tauri::generate_context!())
//! ```

pub mod commands;
pub mod core;
pub mod services;
pub mod tray;

use std::path::{Path, PathBuf};

use commands::error::CommandError;
use commands::state::AppState;
use tauri::Manager;
use tracing_subscriber::{fmt, EnvFilter};

/// Canonical location of the auto-generated `bindings.ts` the
/// frontend imports from.
///
/// Resolves to `<CARGO_MANIFEST_DIR>/../src/types/bindings.ts` — i.e.
/// the Tauri project root's `src/types/bindings.ts`. Cargo guarantees
/// `CARGO_MANIFEST_DIR` points at the crate root (`src-tauri/`), so
/// the parent is always the project root regardless of where the
/// caller happens to run `cargo` from.
///
/// # Why a public helper (P10.3 D6)
///
/// Three independent code paths need the same target path:
///
/// 1. `export_specta_bindings` — private debug-build boot export
///    inside [`run`] (kept private; navigate via the `lib.rs` source
///    if the implementation matters).
/// 2. `src-tauri/examples/export_bindings.rs` — the
///    standalone regenerate-bindings entry point a developer runs
///    via `cargo run --example export_bindings` when they don't want
///    to spin up `cargo tauri dev` just to refresh types.
/// 3. `commands::bindings_file_tests::bindings_path` — the drift
///    guard that greps the committed file for required symbols.
///
/// Keeping the path computation in one place means renaming the
/// target (future restructure: `src/types/` → `src/api/`) is a
/// one-line edit. Previous Tauri prototypes in this repo already
/// drifted once when the path was duplicated across the boot helper
/// and the test; this helper is the structural fix so we don't
/// repeat that mistake.
pub fn default_bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent (the Tauri project root)")
        .join("src")
        .join("types")
        .join("bindings.ts")
}

/// Resolve the production storage root directory.
///
/// # Windows (production target)
///
/// Reads `%APPDATA%` via [`std::env::var_os`] and appends `Beanfun`,
/// matching the legacy WPF client's `SpecialFolder.ApplicationData`
/// convention so on-disk state (Users.dat, Config.xml, logs, update
/// cache) lands in the same place the old binary used. `APPDATA` is
/// set by the OS on every normal user session; the env-var-missing
/// case is surfaced as `CommandError` with
/// `code = "system.app_data_missing"` rather than panicking so
/// [`run`] can emit a readable fatal message.
///
/// Mirrors [`crate::services::storage::default_users_dat_path`] and
/// [`crate::services::config::xml::default_config_xml_path`], which
/// each resolve the same env var and join their respective filename.
/// Duplicating the resolver here (rather than reusing one of those
/// helpers) keeps the boot path independent of any single service's
/// path convention — the storage root is an app-level concern, not
/// a storage-layer concern.
///
/// # Non-Windows builds
///
/// Falls back to `std::env::temp_dir().join("Beanfun")` so the crate
/// compiles on macOS / Linux for smoke testing (integration tests,
/// developer laptops running `cargo check`). The production target
/// remains Windows.
#[cfg(target_os = "windows")]
fn resolve_storage_root() -> Result<PathBuf, CommandError> {
    let appdata = std::env::var_os("APPDATA")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            CommandError::new(
                "system.app_data_missing",
                "APPDATA environment variable is missing or empty",
            )
        })?;
    Ok(PathBuf::from(appdata).join("Beanfun"))
}

#[cfg(not(target_os = "windows"))]
fn resolve_storage_root() -> Result<PathBuf, CommandError> {
    Ok(std::env::temp_dir().join("Beanfun"))
}

/// Detects whether the host is running Windows 10 (build number < 22000).
///
/// Used by [`run`] as a gate for the `set_shadow(false)` workaround for
/// upstream tauri-apps/tauri#11654 / #13176 — the bug only manifests on
/// Windows 10 because the undecorated-shadow inset calculation in `tao`
/// paints a 1px artefact border after DWM redraws. Windows 11 (build
/// number >= 22000) renders the same shadow correctly since tao #1052
/// landed, and the shadow there is desirable (it's what gives the rounded
/// glass panel its depth), so we keep the default behaviour for it.
///
/// Returns `false` on any non-Windows host (defensive — the caller is
/// already behind `#[cfg(target_os = "windows")]`, this mirrors that
/// contract) and on any Windows host whose `os_info` reports a non-
/// semantic version (`Unknown` / `Rolling` / `Custom`). "Report uncertain
/// version → leave shadow alone" is the safer default: keeping the
/// default shadow on an unknown host is a visual downside at worst,
/// whereas wrongly disabling it on a Win11 host would cause a visible
/// regression there.
#[cfg(target_os = "windows")]
fn is_windows_10() -> bool {
    let info = os_info::get();
    if info.os_type() != os_info::Type::Windows {
        return false;
    }
    match info.version() {
        os_info::Version::Semantic(_, _, build) => *build < 22000,
        _ => false,
    }
}

/// Regenerate `src/types/bindings.ts` from the live
/// `tauri-specta` builder.
///
/// Runs on every debug-build boot so `cargo tauri dev` / `npm run
/// tauri dev` transparently keeps the frontend types in sync with
/// the Rust command signatures — the most common drift source in
/// early Tauri projects is hand-edited `bindings.ts` falling behind
/// a renamed parameter or a new `CommandError` variant.
///
/// # Release builds
///
/// The no-op stub under `#[cfg(not(debug_assertions))]` keeps the
/// release path clean:
///
/// - Shipped installers have `bindings.ts` already committed and
///   bundled into the JS chunk, so runtime regeneration is wasted
///   I/O.
/// - End-user install directories are often locked down
///   (`Program Files`); writing into the source tree from the
///   running binary would surface spurious "access denied" noise.
///
/// # Failure behaviour
///
/// Export errors are **non-fatal**: the app keeps booting with
/// whatever `bindings.ts` is already on disk. A stale binding only
/// affects frontend developers (who will notice immediately when a
/// new command fails to resolve); shipping the app itself has no
/// dependency on this path succeeding.
///
/// # Header injection (frontend lint suppression)
///
/// The header prepended by [`default_typescript_exporter`] disables
/// `@ts-nocheck` and `eslint`/`prettier` for the whole file. The
/// frontend repo runs `vue-tsc --noEmit` and `eslint .` in CI; the
/// generated bindings file otherwise trips dozens of `noUnusedLocals`
/// (TS6133) and `@typescript-eslint/no-explicit-any` errors that come
/// from `tauri-specta`'s framework prelude (e.g. `TAURI_CHANNEL` /
/// `__makeEvents__` declared but currently unreferenced because we
/// don't emit any specta events yet). Suppressing the entire generated
/// artefact is the standard treatment — we already use the same
/// pattern for the `vite-env.d.ts` shim in `eslint.config.js`.
///
/// # Target path
///
/// [`default_bindings_path`] — see that helper for the resolution
/// rule and the DRY rationale shared with the
/// `export_bindings` example binary and the
/// `bindings_file_tests` drift guard.
/// [`tauri_specta::Builder::export`] auto-creates the parent
/// directory via `fs::create_dir_all`, so the `types/` folder does
/// not need to pre-exist.
#[cfg(debug_assertions)]
fn export_specta_bindings<R: tauri::Runtime>(builder: &tauri_specta::Builder<R>) {
    let target = default_bindings_path();

    if let Err(err) = builder.export(default_typescript_exporter(), &target) {
        eprintln!(
            "[dev] tauri-specta export failed: {err} (target={})",
            target.display()
        );
    }
}

#[cfg(not(debug_assertions))]
fn export_specta_bindings<R: tauri::Runtime>(_: &tauri_specta::Builder<R>) {}

/// Build the `specta-typescript` exporter used by every code path that
/// regenerates `bindings.ts` (the dev-mode boot helper above and the
/// `export_bindings` example binary).
///
/// Centralizing the exporter config means any future change — bigint
/// behavior, comment style, formatter — applies in one place. The
/// header suppresses `vue-tsc` / ESLint warnings on the generated
/// artefact; see the [`export_specta_bindings`] doc comment above for
/// the full rationale.
pub fn default_typescript_exporter() -> specta_typescript::Typescript {
    specta_typescript::Typescript::default().header(
        "// @ts-nocheck\n\
         /* eslint-disable */\n\
         /* prettier-ignore */\n",
    )
}

/// Initialize the global `tracing` subscriber so every
/// `tracing::info!` / `warn!` / `error!` / `debug!` emitted by the
/// services and command layers actually reaches stderr.
///
/// # Why this exists (P12.1 D4 live-test postmortem)
///
/// `tracing` is built on a global `Dispatch`; if no subscriber is
/// installed the macros compile but resolve to a no-op writer. From
/// project bootstrap up through P12.1 D4 we shipped progressively
/// richer diagnostic logging (regex-miss `body_preview`s in
/// `session_key.rs` / `send_login.rs`, the `step = "LoginCompleted"`
/// marker in `completed.rs`, every `verify.rs` retry / cooldown
/// trace) but never wired a subscriber, so the diagnostics were all
/// silently dropped. The QR finalize live-test loop surfaced this:
/// after the `auth.missing_web_token` Option B fix landed and the
/// flow started succeeding end-to-end, the user reported "log 完全
/// 沒東西" — confirming the entire observability investment had been
/// invisible the whole time.
///
/// # Filter directive
///
/// Reads `RUST_LOG` first via [`EnvFilter::try_from_default_env`] so
/// developers can override per-session
/// (`RUST_LOG=trace cargo tauri dev`, `RUST_LOG=beanfun_lib=trace`,
/// `RUST_LOG=hyper=warn,beanfun_lib=debug`, …).
///
/// Falls back to `"info,beanfun_lib=debug"`:
///
/// - **Third-party crates (`hyper`, `reqwest`, `tauri`, `tao`, …) at
///   `INFO`** — quiet by default. These libraries emit dense
///   per-request `DEBUG` traces that drown the signal we care about
///   (our service-layer narrative).
/// - **Our own `beanfun_lib` crate at `DEBUG`** — surfaces every
///   diagnostic we have already invested in (login step markers,
///   cookie jar dumps, regex preview bodies) without forcing a
///   `RUST_LOG` env var on every contributor's shell.
///
/// # Output destination
///
/// Default [`tracing_subscriber::fmt`] writes to **stderr**. In
/// `cargo tauri dev` that's the same terminal that runs the dev
/// server, so logs appear interleaved with Vite's HMR output and
/// Cargo's compile messages. For the production Windows build the
/// `windows_subsystem = "windows"` attribute in `main.rs` suppresses
/// the console window, so stderr lands in the void; future P-stage
/// work can route to a file via [`tauri-plugin-log`] or a custom
/// rolling-file appender if production observability becomes a
/// requirement (none today — WPF parity scope only covers dev-time
/// debugging).
///
/// # Test isolation
///
/// `cargo test` does not invoke `run()`, so this initializer is
/// never reached during any unit / integration / specta test.
/// Tests that need tracing visibility install their own subscriber
/// via `tracing_subscriber::fmt::try_init()` in the test body.
/// Calling `init()` twice in a process panics; this single call site
/// in the production boot path is the canonical owner of the global
/// subscriber.
///
/// # WPF parity (none)
///
/// This is observability infrastructure, not business logic. The
/// legacy WPF client wrote ad-hoc `Console.WriteLine` calls plus a
/// hand-rolled file logger; we deliberately swap that for the
/// industry-standard `tracing` ecosystem. The structured fields
/// (`step = "LoginCompleted"`, `url = %url`, …) that the WPF code
/// could only flatten into prose are now first-class and, with this
/// subscriber installed, queryable via downstream JSON exporters
/// when we eventually need them.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,beanfun_lib=debug"));

    fmt().with_env_filter(filter).with_target(true).init();
}

/// Tauri application entry point.
///
/// On storage-root resolution failure the process exits with code 1
/// after writing a single-line diagnostic to stderr (chosen over
/// `expect`/`panic` so the user-facing fatal message is concise;
/// there's no reasonable recovery when `%APPDATA%` is unresolvable).
///
/// # Boot ordering
///
/// [`init_tracing`] is the very first call so even the storage-root
/// resolver gets a live subscriber if it ever grows a `tracing::warn!`
/// (today it short-circuits via [`eprintln!`] for the fatal path so
/// the tracing layer is unused there, but the ordering is the
/// safe default for future additions).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    let storage_root = resolve_storage_root().unwrap_or_else(|err| {
        eprintln!("fatal: cannot resolve storage root — {err}");
        std::process::exit(1);
    });

    // Read `disableHardwareAcceleration` from Config.xml before the
    // WebView2 runtime initialises. WPF does the same in
    // `WebBrowser.xaml.cs` / `GamePassBrowser.xaml.cs` via
    // `CoreWebView2EnvironmentOptions.AdditionalBrowserArguments`.
    // Tauri/wry reads `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` at
    // WebView2 environment creation time — setting it here (before
    // `tauri::Builder::default()`) is the equivalent.
    #[cfg(target_os = "windows")]
    {
        let config_path = storage_root.join("Config.xml");
        let disable_hw_accel =
            services::config::get_value_sync(&config_path, "disableHardwareAcceleration")
                .unwrap_or_default()
                .eq_ignore_ascii_case("true");
        if disable_hw_accel {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--disable-gpu --disable-gpu-compositing",
            );
            tracing::info!("hardware acceleration disabled via Config.xml");
        }
    }

    let app_state = AppState::new(storage_root);
    let specta_builder = commands::build_specta_builder::<tauri::Wry>();
    export_specta_bindings(&specta_builder);
    let invoke_handler = specta_builder.invoke_handler();

    let storage_root_for_tray = app_state.storage_root.clone();
    let tray_state_arc =
        std::sync::Arc::new(std::sync::Mutex::new(None::<tauri::tray::TrayIconId>));
    let tray_state_for_setup = tray_state_arc.clone();
    let tray_state_for_event = tray_state_arc.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        // Tauri-managed handle to the tray ID so commands (e.g.
        // `system::minimize_main_window`) can drive the tray without
        // re-discovering the ID. Same `Arc` the setup / window-event
        // closures hold — single source of truth.
        .manage(tray::TrayState(tray_state_arc))
        .invoke_handler(invoke_handler)
        .setup(move |app| {
            if let Some(tray_id) = tray::build_tray(app) {
                *tray_state_for_setup.lock().unwrap() = Some(tray_id);
            }

            // Issue #235 workaround — on Windows 10 the undecorated-window
            // shadow inset calculation paints a 1px coloured border on DWM
            // redraws (upstream tauri-apps/tauri#11654 / #13176). Disabling
            // the shadow removes the artefact. Windows 11 (build >= 22000)
            // keeps the default shadow because the tao #1052 fix renders
            // correctly there and the shadow gives the rounded glass panel
            // its depth. No-op on non-Windows targets.
            #[cfg(target_os = "windows")]
            if is_windows_10() {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(err) = window.set_shadow(false) {
                        tracing::warn!(
                            "set_shadow(false) on Windows 10 main window failed: {err}"
                        );
                    }
                }
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if window.label() != "main" {
                return;
            }
            let tray_id = tray_state_for_event.lock().unwrap().clone();
            if let Some(tray_id) = tray_id {
                tray::handle_minimize_to_tray(
                    window.app_handle().clone(),
                    tray_id,
                    storage_root_for_tray.clone(),
                    event,
                );
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
