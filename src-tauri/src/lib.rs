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

/// Per-instance WebView2 user data folders (issue #340).
///
/// # Why not the Tauri default (one shared folder)
///
/// Tauri's default puts every instance's webview in the same user data
/// folder (`%LOCALAPPDATA%\tw.beanfun.app\EBWebView`). All WebView2
/// instances sharing a user data folder share **one** browser process,
/// and creating a `CoreWebView2Environment` against a folder whose
/// browser process is already running fails with
/// `HRESULT_FROM_WIN32(ERROR_INVALID_STATE)` whenever the new
/// environment doesn't match the running one exactly — different
/// environment options *or* a different WebView2 Runtime version.
///
/// That mismatch is exactly what multi-instance users hit (#340): with
/// several launchers left running, the Evergreen WebView2 Runtime
/// auto-updates in the background (or the user updates Beanfun and
/// launches a build with different browser args), and every launcher
/// started *after* that fails webview creation. Because the main window
/// is `transparent: true` + `decorations: false`, a window with no
/// webview paints nothing — the process shows up in Task Manager but no
/// UI ever appears. Cross-process sharing is fragile even without a
/// version skew (MicrosoftEdge/WebView2Feedback#4751, closed
/// "not planned").
///
/// Nothing in this app needs webview persistence: settings live in
/// `Config.xml` via the backend (`pinia-plugin-persistedstate` is
/// deliberately not installed), and login cookies live in the backend
/// HTTP client's jar and are re-seeded into in-app browser windows per
/// launch. So each instance gets its own throwaway folder —
/// `%LOCALAPPDATA%\tw.beanfun.app\webview-instances\<pid>` — and no two
/// Beanfun processes ever negotiate over a shared browser process.
///
/// # Cleanup protocol
///
/// Folders are reclaimed on the next boot by [`sweep_stale_webview_dirs`]
/// rather than on exit (an exiting instance can't reliably delete its
/// own folder while its browser subprocesses are still shutting down).
/// Liveness is detected with the filesystem, not the PID table (PIDs
/// get reused): a directory is only deleted after it has been
/// **renamed** away, and Windows refuses to rename a directory while
/// any file inside is open. A live instance always holds an open handle
/// inside its folder — the `.instance-lock` file created by
/// [`prepare_webview_data_dir`] the moment the folder exists, before
/// WebView2 opens its own files — so the rename test can never claim a
/// running instance's profile. If a rename succeeds but the delete then
/// fails halfway, the leftover keeps its `.stale` name and the delete
/// is simply retried on every subsequent boot.
#[cfg(target_os = "windows")]
fn webview_instance_base_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|s| !s.is_empty())
        .map(|d| {
            PathBuf::from(d)
                .join("tw.beanfun.app")
                .join("webview-instances")
        })
}

/// The per-instance WebView2 data directory prepared at boot, for
/// runtime-created webview windows (e.g. the MapleStory Classic portal
/// in `commands::classic`) to share with the main window. Sharing the
/// directory means sharing the already-running per-instance browser
/// process — a second window must NOT fall back to Tauri's default
/// shared folder, or it reintroduces the cross-instance
/// `ERROR_INVALID_STATE` failure (#340) for that window.
///
/// Returns `None` when boot fell back to the shared default (the
/// window builder then simply omits `data_directory`, matching the
/// main window's fallback).
/// Browser arguments the main window's WebView2 environment was built
/// with, for runtime-created windows to reuse.
///
/// # Why this must be shared (issue #356)
///
/// Every WebView2 environment sharing a user data folder must be
/// configured **identically**; a second environment with different
/// options is refused with `ERROR_INVALID_STATE` (0x8007139F). The
/// MapleStory Classic portal reused the per-instance folder (#340) but
/// not the args, so its webview failed to create and the launch button
/// did nothing — measured, not theorised:
///
/// ```text
/// ERROR tauri_runtime_wry: failed to create webview:
///   WebView2 error: WindowsError(HRESULT(0x8007139F))
/// WARN  classic: portal webview not usable … rebuilding once
/// ```
///
/// Any window that passes [`current_instance_webview_dir`] must pass
/// these too — they are two halves of one environment identity.
#[cfg(target_os = "windows")]
static BROWSER_ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The main window's browser arguments; `None` before boot has set them
/// (and on non-Windows, where the concept doesn't apply).
#[cfg(target_os = "windows")]
pub(crate) fn current_browser_args() -> Option<&'static str> {
    BROWSER_ARGS.get().map(String::as_str)
}

#[cfg(target_os = "windows")]
pub(crate) fn current_instance_webview_dir() -> Option<PathBuf> {
    let dir = webview_instance_base_dir()?.join(std::process::id().to_string());
    dir.is_dir().then_some(dir)
}

/// Create (and lock) this process's WebView2 user data folder.
///
/// Returns `None` — falling back to Tauri's shared default folder — if
/// `%LOCALAPPDATA%` is unresolvable or the directory can't be created;
/// a launcher that boots like 6.0.5 did is better than one that
/// doesn't boot at all. See [`webview_instance_base_dir`] for the full
/// rationale.
#[cfg(target_os = "windows")]
fn prepare_webview_data_dir() -> Option<PathBuf> {
    let dir = webview_instance_base_dir()?.join(std::process::id().to_string());
    if let Err(err) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            "could not create per-instance webview dir {}: {err}; falling back to the shared default",
            dir.display()
        );
        return None;
    }
    // Hold an open handle inside the folder for the whole process
    // lifetime so a concurrently booting instance's sweep can never
    // rename this folder out from under us (Windows fails a directory
    // rename while any file within is open). Leaked deliberately — the
    // OS closes it at process exit, which is exactly the lock's scope.
    match std::fs::File::create(dir.join(".instance-lock")) {
        Ok(guard) => {
            Box::leak(Box::new(guard));
        }
        Err(err) => tracing::warn!(
            "could not create .instance-lock in {}: {err} (sweep may race a cold boot)",
            dir.display()
        ),
    }
    Some(dir)
}

/// Best-effort removal of webview folders left behind by dead instances.
///
/// Every directory under `base` other than `own_pid`'s is renamed to
/// `<name>.stale` first; only when the rename succeeds (= no file inside
/// is open ⇒ no live instance) is it deleted. Directories already
/// carrying a `.stale` suffix are leftovers from a previously
/// interrupted delete and are retried directly. Runs on a background
/// thread at boot — see [`webview_instance_base_dir`] for the protocol.
#[cfg(target_os = "windows")]
fn sweep_stale_webview_dirs(base: &Path, own_pid: u32) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let own_name = own_pid.to_string();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == own_name {
            continue;
        }
        let doomed = if name.ends_with(".stale") {
            path
        } else {
            let renamed = base.join(format!("{name}.stale"));
            match std::fs::rename(&path, &renamed) {
                Ok(()) => renamed,
                // Rename refused ⇒ some file inside is open ⇒ a live
                // instance owns this folder (or a previous `.stale`
                // target still exists — retried after that's gone).
                Err(_) => continue,
            }
        };
        if let Err(err) = std::fs::remove_dir_all(&doomed) {
            tracing::debug!(
                "webview dir sweep: could not remove {}: {err} (will retry next boot)",
                doomed.display()
            );
        }
    }
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
pub(crate) fn is_windows_10() -> bool {
    let info = os_info::get();
    if info.os_type() != os_info::Type::Windows {
        return false;
    }
    match info.version() {
        os_info::Version::Semantic(_, _, build) => *build < 22000,
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_windows_10() -> bool {
    false
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

    // WebView2 browser arguments — attached to the main window builder in
    // `setup` (runtime windows share the per-instance environment the main
    // window creates, so its args govern the whole browser process).
    #[cfg(target_os = "windows")]
    let browser_args: String = {
        let config_path = storage_root.join("Config.xml");
        let disable_hw_accel =
            services::config::get_value_sync(&config_path, "disableHardwareAcceleration")
                .unwrap_or_default()
                .eq_ignore_ascii_case("true");

        // NOTE: we deliberately do NOT pass any scale-related browser flags.
        //
        // `--force-device-scale-factor=1` (from #257's physical-px
        // "DPI-immune" layout) pinned the webview to 1:1 physical pixels.
        // But #326 switched the router's `fitWindow` to size the OS window
        // with `LogicalSize`, which Tauri multiplies by the window's OS
        // scale factor. With the webview still rendering 1:1, at >100%
        // Windows scaling the window came out `scale`× larger than the 1:1
        // content → the reported "window too big, empty space around the
        // content" bug (#337). Letting the webview's `devicePixelRatio`
        // follow the OS scale makes `LogicalSize` and the CSS-px
        // measurements in `fitWindow` consistent at every display scaling.
        //
        // `--force-text-scale-factor=1` (which #337 kept as a "Text size"
        // guard) is NOT a real Chromium switch — it matches nothing in the
        // Chromium source and was silently ignored, so Windows
        // Accessibility "Text size" ≥ 130% clipped the app again. The text
        // scale reaches the webview via WebView2's rasterization scale
        // (monitor DPI × text scale, surfaced as `devicePixelRatio`) and is
        // now compensated in the frontend resizer instead: `fitWindow`
        // multiplies its `LogicalSize` by the measured text-scale ratio so
        // the window grows to fit the enlarged text rather than fighting it
        // (see src/services/windowFit.ts).
        // IMPORTANT: these must go through the window builder's
        // `additional_browser_args`, NOT the
        // `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` env var. wry passes
        // explicit `AdditionalBrowserArguments` environment options, and
        // the WebView2 loader then ignores the env var entirely —
        // verified empirically: with the env var set, the spawned
        // browser process carried none of its flags (which also means
        // the old env-var-based hw-accel toggle silently never worked).
        //
        // The builder REPLACES wry's default args, so wry's defaults
        // (mini-menu/smart-screen feature disables + autoplay) must be
        // restated here or they'd silently vanish.
        let mut args: Vec<String> = vec![
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection".to_string(),
            "--autoplay-policy=no-user-gesture-required".to_string(),
        ];

        if disable_hw_accel {
            args.push("--disable-gpu".to_string());
            args.push("--disable-gpu-compositing".to_string());
            tracing::info!("hardware acceleration disabled via Config.xml");
        }

        // Dev/testing affordance: expose the webview's Chrome DevTools
        // Protocol endpoint so automated tooling can drive the app
        // (`BEANFUN_REMOTE_DEBUGGING_PORT=9222 beanfun.exe`). The flag
        // applies to the whole per-instance browser process, so runtime
        // windows (classic portal, in-app browsers) are inspectable too.
        if let Ok(port) = std::env::var("BEANFUN_REMOTE_DEBUGGING_PORT") {
            if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() {
                args.push(format!("--remote-debugging-port={port}"));
                tracing::info!("webview remote debugging enabled on port {port}");
            }
        }

        let joined = args.join(" ");
        // Runtime windows sharing the per-instance user data folder must
        // build their environment with exactly these args — see
        // `current_browser_args`.
        let _ = BROWSER_ARGS.set(joined.clone());
        joined
    };

    // Per-instance WebView2 profile (issue #340) — see
    // `webview_instance_base_dir` for why instances must not share one.
    // The sweep of dead instances' folders runs off the boot path; it
    // never touches live folders (rename-guard) or our own (pid guard).
    #[cfg(target_os = "windows")]
    let webview_data_dir = prepare_webview_data_dir();
    #[cfg(target_os = "windows")]
    if let Some(base) = webview_instance_base_dir() {
        let own_pid = std::process::id();
        std::thread::spawn(move || sweep_stale_webview_dirs(&base, own_pid));
    }
    #[cfg(not(target_os = "windows"))]
    let webview_data_dir: Option<PathBuf> = None;

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
        // Remember-window-position: restores the user's last screen
        // position on launch (e.g. after dragging Beanfun onto a
        // secondary monitor). `StateFlags::POSITION` intentionally
        // *excludes* SIZE / MAXIMIZED / DECORATIONS — `tauri.conf.json`
        // ships `resizable: false` and the router's per-route
        // `fitWindow` (`src/router/index.ts`) is the canonical owner
        // of the window's width/height. Persisting size here would
        // race those handlers and the user would see the window snap
        // to the saved size before snapping again to the route-driven
        // size on the first navigation. The state file lives under
        // the standard `appConfigDir` (Windows: `%APPDATA%\tw.beanfun.app\`).
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(tauri_plugin_window_state::StateFlags::POSITION)
                .build(),
        )
        .manage(app_state)
        // Tauri-managed handle to the tray ID so commands (e.g.
        // `system::minimize_main_window`) can drive the tray without
        // re-discovering the ID. Same `Arc` the setup / window-event
        // closures hold — single source of truth.
        .manage(tray::TrayState(tray_state_arc))
        .invoke_handler(invoke_handler)
        .setup(move |app| {
            // The main window is built here instead of `tauri.conf.json`
            // so the per-instance WebView2 data directory (issue #340)
            // can be injected — the static config can't carry a
            // PID-dependent path. Everything else mirrors the old
            // config entry verbatim (label defaults to "main" there).
            let mut win_builder =
                tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                    .title("Beanfun")
                    .inner_size(560.0, 480.0)
                    .decorations(false)
                    .transparent(true)
                    .resizable(false);
            if let Some(dir) = webview_data_dir.clone() {
                win_builder = win_builder.data_directory(dir);
            }
            // Browser args must ride the builder — the WebView2 loader
            // ignores the WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS env var
            // when explicit environment options are passed (see the
            // `browser_args` construction above).
            #[cfg(target_os = "windows")]
            {
                win_builder = win_builder.additional_browser_args(&browser_args);
            }
            win_builder.build()?;

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
                        tracing::warn!("set_shadow(false) on Windows 10 main window failed: {err}");
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

#[cfg(all(test, target_os = "windows"))]
mod webview_dir_tests {
    use super::sweep_stale_webview_dirs;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Fresh, empty base directory unique to one test (parallel-safe).
    fn temp_base(tag: &str) -> PathBuf {
        let base = std::env::temp_dir()
            .join("beanfun-webview-sweep-tests")
            .join(format!("{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create temp base");
        base
    }

    fn make_profile(base: &Path, name: &str) -> PathBuf {
        let dir = base.join(name);
        fs::create_dir_all(&dir).expect("create profile dir");
        fs::write(dir.join("data.bin"), b"x").expect("write profile file");
        dir
    }

    #[test]
    fn removes_unlocked_stale_profile() {
        let base = temp_base("unlocked");
        let dead = make_profile(&base, "12345");

        sweep_stale_webview_dirs(&base, 99999);

        assert!(!dead.exists(), "dead instance profile must be removed");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn keeps_own_profile() {
        let base = temp_base("own");
        let own_pid = std::process::id();
        let own = make_profile(&base, &own_pid.to_string());

        sweep_stale_webview_dirs(&base, own_pid);

        assert!(own.exists(), "own profile must never be swept");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn keeps_profile_with_open_file() {
        // A live instance always holds an open handle inside its folder
        // (the `.instance-lock`); Windows then refuses the rename that
        // gates deletion, so the sweep must leave the folder alone.
        let base = temp_base("locked");
        let live = make_profile(&base, "23456");
        let _guard = fs::File::create(live.join(".instance-lock")).expect("lock");

        sweep_stale_webview_dirs(&base, 99999);

        assert!(live.exists(), "profile with an open handle must survive");
        drop(_guard);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn retries_leftover_stale_dirs() {
        // A previous sweep renamed the folder but died before deleting
        // it; the next sweep must pick the `.stale` leftover up directly.
        let base = temp_base("leftover");
        let leftover = make_profile(&base, "34567.stale");

        sweep_stale_webview_dirs(&base, 99999);

        assert!(!leftover.exists(), "renamed leftovers must be retried");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn ignores_plain_files_in_base() {
        let base = temp_base("files");
        let file = base.join("not-a-dir.txt");
        fs::write(&file, b"x").expect("write");

        sweep_stale_webview_dirs(&base, 99999);

        assert!(
            file.exists(),
            "non-directory entries are not the sweep's business"
        );
        let _ = fs::remove_dir_all(&base);
    }
}
