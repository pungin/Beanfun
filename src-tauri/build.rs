use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// LocaleRemulator assets shipped by the WPF tree, in the exact order
/// `MainWindow::startByLR` (L1904-1914) checks them. The runtime
/// `locale_remulator` module `include_bytes!`s the same files with the
/// same ordering, so this list is the single source of truth.
const LR_ASSETS: &[&str] = &[
    "LRConfig.xml",
    "LRHookx32.dll",
    "LRHookx64.dll",
    "LRProc.exe",
    "LRSubMenus.dll",
];

fn main() {
    let attributes = tauri_build::Attributes::new();
    #[cfg(windows)]
    let attributes = {
        embed_app_manifest_for_all_binaries();
        attributes.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    };
    tauri_build::try_build(attributes).expect("tauri_build::try_build failed");

    emit_lr_sha256();
}

/// Embed the Windows application manifest into **every** binary
/// produced by this crate (the main app, examples, and test
/// executables) instead of just the main `Beanfun.exe`.
///
/// # Why this exists (P10.3 D6)
///
/// `tauri-build`'s default Windows manifest path goes through
/// [`tauri_winres::WindowsResource::set_manifest`] →
/// `embed_resource::compile()`, which emits a `cargo:rustc-link-arg-bins`
/// directive. The `-bins` suffix scopes the linker arg to *bin*
/// targets only — example binaries (`cargo run --example
/// export_bindings`) and test binaries (`cargo test --lib`) are
/// excluded. Those binaries still get the **import** for
/// Common Controls v6 APIs (because the `tauri` rlib on the link
/// line carries a static dependency on `comctl32.dll` v6 entries),
/// but without a manifest declaring the Common Controls v6
/// `<dependentAssembly>`, Windows resolves `comctl32.dll` to the
/// stub v5 redirector that lacks those v6-only exports — so the
/// loader bails with `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139)
/// at process-start time.
///
/// Tauri tracks this as a known issue across several reports
/// (tauri-apps/tauri#11028 / #13419 / #13948 / #14580); the
/// official workaround — recommended by Tauri maintainer
/// `lucasfernog` — is exactly what this function does:
///
/// 1. Tell `tauri-build` to skip the default manifest embed via
///    [`tauri_build::WindowsAttributes::new_without_app_manifest`]
///    (otherwise the main binary would end up with two competing
///    manifests, and the linker emits `LNK4078` warnings).
/// 2. Re-embed the same manifest ourselves through
///    `cargo:rustc-link-arg=/MANIFEST:EMBED` +
///    `/MANIFESTINPUT:<path>` — `rustc-link-arg` (no `-bins`
///    suffix) propagates to **every** linker invocation in this
///    crate, so example and test binaries inherit the manifest
///    too.
///
/// The manifest content under
/// `src-tauri/windows-app-manifest.xml` is byte-identical to the
/// `tauri-build`-bundled `windows-app-manifest.xml` — we copied
/// it verbatim so production binaries see the exact same
/// Common Controls v6 dependency declaration they did before this
/// change. Other Windows resources `tauri-build` injects
/// (version info, icon, product name) are unaffected and continue
/// to land on the main binary only via `tauri-build`'s separate
/// `WindowsResource` call.
///
/// # Linker requirements
///
/// `/MANIFEST:EMBED` requires `mt.exe` (Windows SDK Manifest
/// Tool) on `PATH` for the linker to call. The MSVC toolchain
/// ships `mt.exe` alongside `link.exe`, so any developer with
/// MSVC build tools installed (a hard requirement for compiling
/// Tauri on Windows anyway) already has it.
#[cfg(windows)]
fn embed_app_manifest_for_all_binaries() {
    static WINDOWS_MANIFEST_FILE: &str = "windows-app-manifest.xml";

    let manifest = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo"),
    )
    .join(WINDOWS_MANIFEST_FILE);

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='highestAvailable' uiAccess='false'");
    }

    println!(
        "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
        manifest
            .to_str()
            .expect("manifest path is always valid UTF-8 on Windows host")
    );
}

/// Compute the SHA-256 of every LocaleRemulator asset referenced by
/// `LR_ASSETS` and write a Rust source snippet to `$OUT_DIR/lr_sha256.rs`
/// so the runtime module can `include!` a typed const array.
///
/// # Panics
///
/// Build fails if any asset is missing: without the hash we can't
/// enforce the SHA-256 integrity check P8 chunk 8.2 ships as an
/// upgrade over WPF's length-only comparison, and silently skipping
/// would let a tampered DLL sneak through.
fn emit_lr_sha256() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo"),
    );
    let lr_dir = manifest_dir.join("LocaleRemulator");

    println!("cargo:rerun-if-changed=build.rs");

    let mut entries: Vec<(String, [u8; 32])> = Vec::with_capacity(LR_ASSETS.len());
    for name in LR_ASSETS {
        let path = lr_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());

        let bytes = std::fs::read(&path).unwrap_or_else(|err| {
            panic!(
                "LocaleRemulator asset `{name}` is required for the runtime integrity check \
                 but could not be read from `{}`: {err}. The WPF tree at Beanfun/LocaleRemulator/ \
                 must contain this file for Beanfun to build.",
                path.display()
            )
        });

        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        entries.push((name.to_string(), digest));
    }

    let out_dir = PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR is always set by cargo for build scripts"),
    );
    let out_file = out_dir.join("lr_sha256.rs");
    std::fs::write(&out_file, render_sha256_table(&entries))
        .unwrap_or_else(|err| panic!("failed to write `{}`: {err}", out_file.display()));
}

/// Render the computed `(name, sha256)` pairs into a Rust source
/// snippet that the runtime module `include!`s. Kept as a separate
/// pure function so the format stays reviewable in one place.
fn render_sha256_table(entries: &[(String, [u8; 32])]) -> String {
    let mut out = String::new();
    out.push_str("// @generated by build.rs — LocaleRemulator SHA-256 table.\n");
    out.push_str("// Do not edit; regenerate by rebuilding with updated binaries under\n");
    out.push_str("// `LocaleRemulator/`.\n\n");
    out.push_str("pub(crate) const LR_SHA256: [(&str, [u8; 32]); ");
    out.push_str(&entries.len().to_string());
    out.push_str("] = [\n");
    for (name, digest) in entries {
        out.push_str("    (\"");
        out.push_str(name);
        out.push_str("\", [");
        for (idx, byte) in digest.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("0x{byte:02x}"));
        }
        out.push_str("]),\n");
    }
    out.push_str("];\n");
    out
}
