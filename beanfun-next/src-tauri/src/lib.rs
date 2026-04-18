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
//!           .plugin(tauri_plugin_opener)
//!           .manage(app_state)         ← State<'_, AppState> in every cmd
//!           .invoke_handler(specta.invoke_handler())
//!           .run(tauri::generate_context!())
//! ```

pub mod commands;
pub mod core;
pub mod services;

use std::path::PathBuf;

use commands::error::CommandError;
use commands::state::AppState;

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

/// Regenerate `beanfun-next/src/types/bindings.ts` from the live
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
/// # Target path
///
/// `<CARGO_MANIFEST_DIR>/../src/types/bindings.ts`, resolved at
/// compile time via [`env!("CARGO_MANIFEST_DIR")`][std::env!]. Cargo
/// guarantees this constant points at the crate root (i.e.
/// `src-tauri/`), whose parent is the Tauri project root.
/// [`tauri_specta::Builder::export`] auto-creates the parent
/// directory via `fs::create_dir_all`, so the `types/` folder does
/// not need to pre-exist.
#[cfg(debug_assertions)]
fn export_specta_bindings<R: tauri::Runtime>(builder: &tauri_specta::Builder<R>) {
    use specta_typescript::Typescript;
    use std::path::Path;

    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri always has a parent (the Tauri project root)")
        .join("src")
        .join("types")
        .join("bindings.ts");

    if let Err(err) = builder.export(Typescript::default(), &target) {
        eprintln!(
            "[dev] tauri-specta export failed: {err} (target={})",
            target.display()
        );
    }
}

#[cfg(not(debug_assertions))]
fn export_specta_bindings<R: tauri::Runtime>(_: &tauri_specta::Builder<R>) {}

/// Tauri application entry point.
///
/// On storage-root resolution failure the process exits with code 1
/// after writing a single-line diagnostic to stderr (chosen over
/// `expect`/`panic` so the user-facing fatal message is concise;
/// there's no reasonable recovery when `%APPDATA%` is unresolvable).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage_root = resolve_storage_root().unwrap_or_else(|err| {
        eprintln!("fatal: cannot resolve storage root — {err}");
        std::process::exit(1);
    });

    let app_state = AppState::new(storage_root);
    let specta_builder = commands::build_specta_builder::<tauri::Wry>();
    export_specta_bindings(&specta_builder);
    let invoke_handler = specta_builder.invoke_handler();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
