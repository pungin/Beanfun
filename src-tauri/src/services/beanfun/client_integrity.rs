//! Client-integrity triple (`CV` / `Hash` / `arch`) that beanfun's TW OTP
//! endpoint has required since Gamania Games Manager (GGM) 1.5.x.
//!
//! # Why this exists (issue #368)
//!
//! Gamania re-versioned the TW launch path in August 2026. The OTP endpoint
//! itself is unchanged — still
//! `beanfun_block/generic_handlers/get_webstart_otp.ashx` — but the current
//! GGM appends three extra query parameters that fingerprint the calling
//! client, and the server now rejects requests that omit them with the
//! opaque body `0;Query String Error`.
//!
//! Ground truth was taken from the shipping GGM 1.5.0.2 on 2026-08-17. The
//! `GGM.Shared.dll` that GGM 1.5.x installs alongside `GGMWebStart.dll` is
//! **not** name-obfuscated, and its `GGM.Shared.Beanfun.BeanfunUrlBuilder`
//! spells the contract out:
//!
//! ```text
//! BuildOtpUrl(sn, webToken, secretCode, ppppp, serviceCode, serviceRegion,
//!             serviceAccount, createTime, environment, cv, hash, arch)
//!   -> "{base}beanfun_block/generic_handlers/get_webstart_otp.ashx
//!       ?SN=…&WebToken=…&SecretCode=…&ppppp=…&ServiceCode=…&ServiceRegion=…
//!       &ServiceAccount=…&CreateTime=…&d={TickCount}
//!       &CV={cv}&Hash={hash}&arch={arch}"
//! ```
//!
//! where the trailing three are each `Uri.EscapeDataString`-encoded, and the
//! values come from GGM's own `ClientIntegrityInfo` initialiser:
//!
//! | Parameter | Origin in GGM                                         | Observed |
//! |-----------|-------------------------------------------------------|----------|
//! | `CV`      | `Assembly.GetExecutingAssembly().GetName().Version`   | `1.5.0.2` |
//! | `Hash`    | SHA-256 of `GGMWebStart.dll`, `b.ToString("x2")`      | `dfd568a6…101e06` |
//! | `arch`    | `Environment.Is64BitProcess ? "x64" : "x86"`          | `x64` |
//!
//! GGM's own log line (verified on a live launch) reads:
//!
//! ```text
//! ClientIntegrityInfo CV=1.5.0.2 arch=x64 Hash=dfd568a69d87abcd…101e06
//! ```
//!
//! # Resolution strategy
//!
//! [`ClientIntegrity::resolve`] describes the **locally installed** GGM,
//! falling back to the bundled constants. Whether that description or a
//! published one is used is decided by the caller — see
//! `services::beanfun::otp::resolve_client_integrity`, which takes
//! whichever names the newer build:
//!
//! 1. Locate `GGMWebStart.dll` — first via the `gamaniagames://` protocol
//!    handler the installer registers (authoritative even for non-default
//!    install locations), then via the conventional install roots.
//! 2. Hash it and read its version.
//! 3. If either half cannot be obtained, use [`ClientIntegrity::fallback`].
//!
//! Reading the live file keeps us current *if the file is current*. GGM
//! self-updates — it polls `CheckVersion.ashx` on launch and hands off to
//! `Patcher.exe` when it is behind — but only when it runs, and the people
//! this app exists for are the ones who never run it: they launch from here,
//! not from the official site. So an install can sit at whatever version it
//! was when it was last opened, which may be the version beanfun has since
//! stopped accepting.
//!
//! That is why the caller compares this against the published pair instead of
//! taking it on sight. The bundled constants remain the answer for a machine
//! with neither, and go stale the next time Gamania ships a GGM build —
//! refreshing them is a release-time chore, not a runtime one.
//!
//! # Deliberately all-or-nothing
//!
//! `CV` and `Hash` describe the *same* binary, so mixing a locally computed
//! hash with a bundled version (or vice versa) would describe a client that
//! does not exist. Every failure path therefore yields the complete
//! [`ClientIntegrity::fallback`] pair rather than a half-local hybrid.
//!
//! # Deliberately uncached
//!
//! Hashing a ~1.3 MB file costs single-digit milliseconds and OTP retrieval
//! is user-initiated (a button press), so the triple is recomputed per call.
//! Caching would risk pinning a pre-update hash for the lifetime of the
//! process, which is precisely the failure this module exists to avoid.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// File name of the GGM assembly that both supplies and is fingerprinted by
/// the `Hash` parameter.
const GGM_DLL_NAME: &str = "GGMWebStart.dll";

/// `CV` used when no local GGM install can be inspected.
///
/// Must always be paired with [`FALLBACK_HASH`] — see the module's
/// "all-or-nothing" note.
const FALLBACK_CV: &str = "1.5.0.2";

/// `Hash` used when no local GGM install can be inspected: the SHA-256 of
/// `GGMWebStart.dll` as shipped in GGM 1.5.0.2, lowercase hex.
const FALLBACK_HASH: &str = "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06";

/// `arch` reports the bitness of the *calling process*, mirroring GGM's
/// `Environment.Is64BitProcess` rather than the bitness of the OS.
#[cfg(target_pointer_width = "64")]
const ARCH: &str = "x64";
#[cfg(not(target_pointer_width = "64"))]
const ARCH: &str = "x86";

/// The `CV` / `Hash` / `arch` triple appended to the TW OTP request.
///
/// Values are stored raw (un-encoded); percent-encoding is the URL
/// builder's job so this type stays a plain description of the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIntegrity {
    /// GGM assembly version, e.g. `"1.5.0.2"`.
    pub cv: String,
    /// Lowercase-hex SHA-256 of `GGMWebStart.dll`.
    pub hash: String,
    /// `"x64"` or `"x86"`.
    pub arch: &'static str,
}

impl ClientIntegrity {
    /// The bundled constants, for hosts with no inspectable GGM install.
    pub fn fallback() -> Self {
        Self {
            cv: FALLBACK_CV.to_string(),
            hash: FALLBACK_HASH.to_string(),
            arch: ARCH,
        }
    }

    /// Describe the locally installed GGM, falling back to the bundled
    /// constants when it cannot be found or fully inspected.
    ///
    /// Never fails: the OTP request needs *some* triple, and a stale-but
    /// -plausible one gives the server a chance to accept where an absent
    /// one is guaranteed to be rejected.
    pub fn resolve() -> Self {
        match Self::resolve_local() {
            Some(found) => found,
            None => {
                tracing::debug!(
                    "no inspectable {GGM_DLL_NAME}; using bundled client-integrity constants"
                );
                Self::fallback()
            }
        }
    }

    /// The installed GGM's values, or `None` when there is no GGM to
    /// read.
    ///
    /// Split out from [`Self::resolve`] so the caller can try the
    /// published values in between rather than dropping straight to the
    /// compiled-in pair.
    pub fn resolve_local() -> Option<Self> {
        locate_ggm_dll().as_deref().and_then(Self::from_ggm_dll)
    }

    /// Build the triple from a `CV` / `Hash` pair someone published or
    /// pinned.
    ///
    /// `arch` is never published: it describes the binary asking, which
    /// is this build, not whatever machine produced the values.
    pub fn from_published(values: &crate::services::beanfun::ggm_hotfix::PublishedValues) -> Self {
        Self {
            cv: values.cv.clone(),
            hash: values.hash.clone(),
            arch: ARCH,
        }
    }

    /// Build the triple from a specific `GGMWebStart.dll`.
    ///
    /// `None` when the file cannot be hashed *or* its version cannot be
    /// read, so callers never assemble a half-local pair.
    fn from_ggm_dll(path: &Path) -> Option<Self> {
        let hash = sha256_lower_hex(path)?;
        let cv = file_version(path)?;
        tracing::debug!(cv = %cv, path = %path.display(), "resolved client integrity from local GGM");
        Some(Self {
            cv,
            hash,
            arch: ARCH,
        })
    }
}

/// Hash `path` into the lowercase-hex form GGM produces with
/// `b.ToString("x2")`.
fn sha256_lower_hex(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        // `write!` into a String is infallible; the result is discarded
        // rather than unwrapped to keep this loop panic-free.
        let _ = write!(hex, "{byte:02x}");
    }
    Some(hex)
}

/// Locate the installed `GGMWebStart.dll`, if any.
fn locate_ggm_dll() -> Option<PathBuf> {
    for dir in ggm_directories() {
        let candidate = dir.join(GGM_DLL_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Candidate GGM install directories, most authoritative first.
#[cfg(target_os = "windows")]
fn ggm_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = ggm_dir_from_protocol_handler() {
        dirs.push(dir);
    }
    // The installer's default location, per architecture-specific
    // Program Files root. Covers a registry that has been cleaned up (or
    // a hand-copied install) while the files are still in place.
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(root) = std::env::var(var) {
            dirs.push(
                PathBuf::from(root)
                    .join("gamania Games")
                    .join("gamania Games Manager"),
            );
        }
    }
    dirs
}

#[cfg(not(target_os = "windows"))]
fn ggm_directories() -> Vec<PathBuf> {
    // GGM is a Windows-only product; every non-Windows host takes the
    // bundled-constant path.
    Vec::new()
}

/// Read the directory of the executable registered for `gamaniagames://`.
///
/// The GGM installer writes
/// `HKCR\gamaniagames\shell\open\command` = `"<dir>\GGMWebStart.exe" "%1"`,
/// which tracks the real install location even when the user chose a
/// non-default path.
#[cfg(target_os = "windows")]
fn ggm_dir_from_protocol_handler() -> Option<PathBuf> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let command: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"gamaniagames\shell\open\command")
        .and_then(|key| key.get_value(""))
        .ok()?;

    let exe = handler_executable(&command)?;
    Path::new(&exe).parent().map(Path::to_path_buf)
}

/// Extract just the executable path from a registry handler command line.
///
/// Handles both the quoted form the GGM installer writes
/// (`"C:\…\GGMWebStart.exe" "%1"`) and a bare unquoted path. Unlike
/// `commands::classic`'s handler parser this deliberately stops at the
/// executable: we only want its directory, never an argument vector, so
/// there is no `%1` substitution to perform.
fn handler_executable(command: &str) -> Option<String> {
    let trimmed = command.trim();
    let exe = match trimmed.strip_prefix('"') {
        Some(rest) => rest.split_once('"').map(|(exe, _)| exe)?,
        None => trimmed.split_whitespace().next()?,
    };
    (!exe.is_empty()).then(|| exe.to_string())
}

/// Read the Win32 version resource's `FileVersion` as `a.b.c.d`.
///
/// GGM sends its **assembly** version, which Rust cannot read without
/// walking the CLI metadata tables. Every shipped GGM build observed so far
/// (1.0.0.0 and 1.5.0.2) carries an identical `FileVersion`, so the version
/// resource stands in for it; when the two ever diverge the caller falls
/// back to the bundled constants, which is also what happens if this read
/// fails outright.
#[cfg(target_os = "windows")]
fn file_version(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::{w, PCWSTR};
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let file = PCWSTR(wide.as_ptr());

    // SAFETY: `file` points at a NUL-terminated wide string that outlives
    // every call below. `block` is sized by the API itself and is only read
    // back through `VerQueryValueW`, whose out-pointer is validated for
    // null and for holding at least a whole `VS_FIXEDFILEINFO` before the
    // struct is dereferenced.
    unsafe {
        let size = GetFileVersionInfoSizeW(file, None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(file, 0, size, block.as_mut_ptr().cast()).ok()?;

        let mut value: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len: u32 = 0;
        // The root sub-block (`"\"`) yields the fixed-info struct.
        if !VerQueryValueW(block.as_ptr().cast(), w!("\\"), &mut value, &mut value_len).as_bool() {
            return None;
        }
        if value.is_null() || (value_len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() {
            return None;
        }

        let info = &*(value as *const VS_FIXEDFILEINFO);
        let most = info.dwFileVersionMS;
        let least = info.dwFileVersionLS;
        Some(format!(
            "{}.{}.{}.{}",
            most >> 16,
            most & 0xFFFF,
            least >> 16,
            least & 0xFFFF
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn file_version(_path: &Path) -> Option<String> {
    None
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── handler_executable ────────────────────────────────────────────

    #[test]
    fn handler_executable_reads_the_quoted_installer_form() {
        // Exactly what the GGM installer writes to HKCR.
        let command =
            r#""C:\Program Files\gamania Games\gamania Games Manager\GGMWebStart.exe" "%1""#;
        assert_eq!(
            handler_executable(command).as_deref(),
            Some(r"C:\Program Files\gamania Games\gamania Games Manager\GGMWebStart.exe"),
        );
    }

    #[test]
    fn handler_executable_reads_an_unquoted_path() {
        assert_eq!(
            handler_executable(r"C:\ggm\GGMWebStart.exe %1").as_deref(),
            Some(r"C:\ggm\GGMWebStart.exe"),
        );
    }

    #[test]
    fn handler_executable_keeps_spaces_inside_quotes() {
        // The unquoted branch would truncate at the first space, so this
        // pins that quoted paths are not split on whitespace.
        let exe = handler_executable(r#""C:\a b\c d\GGMWebStart.exe" "%1""#);
        assert_eq!(exe.as_deref(), Some(r"C:\a b\c d\GGMWebStart.exe"));
    }

    #[test]
    fn handler_executable_rejects_empty_or_degenerate_input() {
        assert_eq!(handler_executable(""), None);
        assert_eq!(handler_executable("   "), None);
        // Opening quote with an empty payload.
        assert_eq!(handler_executable(r#""" "%1""#), None);
        // Unterminated quote — no closing delimiter to split on.
        assert_eq!(handler_executable(r#""C:\ggm\GGMWebStart.exe"#), None);
    }

    // ── sha256_lower_hex ──────────────────────────────────────────────

    #[test]
    fn sha256_lower_hex_matches_the_known_digest_of_empty_input() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("empty.bin");
        std::fs::write(&path, b"").expect("write");

        // Well-known SHA-256 of the empty string, lowercase — the same
        // casing GGM's `ToString("x2")` produces.
        assert_eq!(
            sha256_lower_hex(&path).as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        );
    }

    #[test]
    fn sha256_lower_hex_is_64_lowercase_hex_chars() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("blob.bin");
        std::fs::write(&path, b"gamania").expect("write");

        let hex = sha256_lower_hex(&path).expect("hashable");
        assert_eq!(hex.len(), 64);
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "digest must be lowercase hex, got {hex}",
        );
    }

    #[test]
    fn sha256_lower_hex_returns_none_for_a_missing_file() {
        assert_eq!(
            sha256_lower_hex(Path::new("Z:/definitely/not/here.dll")),
            None
        );
    }

    // ── ClientIntegrity ───────────────────────────────────────────────

    #[test]
    fn fallback_pairs_the_bundled_constants_with_this_process_arch() {
        let integrity = ClientIntegrity::fallback();
        assert_eq!(integrity.cv, "1.5.0.2");
        assert_eq!(
            integrity.hash,
            "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06",
        );
        assert!(matches!(integrity.arch, "x64" | "x86"));
    }

    #[test]
    fn bundled_hash_is_a_plausible_sha256() {
        // Guards a future hand-edit of the constant against typos: the
        // server rejects anything that is not 64 lowercase hex chars.
        assert_eq!(FALLBACK_HASH.len(), 64);
        assert!(FALLBACK_HASH
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn from_ggm_dll_declines_a_missing_file() {
        assert_eq!(
            ClientIntegrity::from_ggm_dll(Path::new("Z:/definitely/not/here.dll")),
            None,
        );
    }

    #[test]
    fn from_ggm_dll_declines_when_the_version_resource_is_unreadable() {
        // A hashable file that carries no Win32 version resource: the
        // all-or-nothing rule must reject it rather than pair a real hash
        // with the bundled version.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("not-a-pe.dll");
        std::fs::write(&path, b"definitely not a portable executable").expect("write");

        assert_eq!(ClientIntegrity::from_ggm_dll(&path), None);
    }

    #[test]
    fn resolve_always_produces_a_usable_triple() {
        // Whether or not this machine has GGM installed, `resolve` must
        // hand the URL builder a complete triple.
        let integrity = ClientIntegrity::resolve();
        assert!(!integrity.cv.is_empty());
        assert_eq!(integrity.hash.len(), 64);
        assert!(matches!(integrity.arch, "x64" | "x86"));
    }
}
