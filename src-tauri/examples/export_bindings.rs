//! Standalone `bindings.ts` regenerator.
//!
//! Run via:
//!
//! ```text
//! cargo run --example export_bindings
//! ```
//!
//! from `src-tauri/`. Writes the regenerated
//! `bindings.ts` to the canonical location resolved by
//! [`beanfun_lib::default_bindings_path`] (i.e.
//! `src/types/bindings.ts`).
//!
//! # Why a dedicated example instead of `cargo tauri dev`?
//!
//! `cargo tauri dev` also regenerates `bindings.ts` on every debug
//! boot (see [`beanfun_lib::run`]'s `export_specta_bindings`
//! call), but it also:
//!
//! - spins up Vite's frontend dev server,
//! - launches a WebView2 window,
//! - blocks the terminal until the user closes the window.
//!
//! That's overkill when the only thing you need is a refreshed
//! `bindings.ts` after editing a command signature. This example
//! bypasses all the UI machinery and exits as soon as the file is
//! written — typical wall-clock time is a couple of seconds on a
//! warm build.
//!
//! # Shared plumbing with `run()`
//!
//! Target path comes from [`beanfun_lib::default_bindings_path`]
//! — the same helper [`beanfun_lib::run`]'s debug-boot exporter
//! calls, so this binary and the live app can never disagree on
//! where `bindings.ts` lives. The [`beanfun_lib::commands::build_specta_builder`]
//! helper is likewise the single source of truth for which commands
//! get exported, so drift between runtime dispatch and emitted TS
//! is impossible by construction. The TypeScript exporter (header
//! injection, comment style, formatter) comes from
//! [`beanfun_lib::default_typescript_exporter`] so this binary
//! and the dev-mode auto-export emit byte-identical output.
//!
//! # Runtime type parameter
//!
//! Instantiates [`build_specta_builder`] with `tauri::Wry` (the
//! production runtime) so the emitted TS exactly matches what
//! `cargo tauri dev` would produce on the next boot. Swapping in
//! `tauri::test::MockRuntime` would re-link `tauri-runtime-wry`
//! anyway (via `tauri-specta`'s transitive deps on Windows — see
//! the module docs on `commands::bindings_file_tests` for the
//! `webview2-com-sys` linkage analysis) and would not meaningfully
//! shrink the build closure, so the MockRuntime detour buys
//! nothing here.
//!
//! # Exit codes
//!
//! - `0` — success.
//! - `1` — `tauri_specta::Builder::export` returned an error (TS
//!   emission failed or the target path couldn't be written to).
//!   The error is printed to stderr with the target path so CI
//!   logs pinpoint the failure cause.
//!
//! Uses `std::process::exit` rather than propagating through
//! `main() -> Result<_, _>` so the stderr line stays free of the
//! default `Error:` prefix `?` would inject — keeping the output
//! consistent with the existing `export_specta_bindings` stderr
//! format in `lib.rs`.

use beanfun_lib::{
    commands::build_specta_builder, default_bindings_path, default_typescript_exporter,
};

fn main() {
    let builder = build_specta_builder::<tauri::Wry>();
    let target = default_bindings_path();

    if let Err(err) = builder.export(default_typescript_exporter(), &target) {
        eprintln!(
            "export_bindings: tauri-specta export failed: {err} (target={})",
            target.display()
        );
        std::process::exit(1);
    }

    println!("export_bindings: wrote {}", target.display());
}
