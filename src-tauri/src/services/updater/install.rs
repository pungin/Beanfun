//! Installation detection and the portable executable replacement hand-off.
//!
//! Only the helper touches the running executable, after acquiring and waiting
//! on its parent's process handle. The helper authenticates cached TUF metadata
//! against the embedded bootstrap; update plans cannot supply trust roots.
//! The previous executable is retained beside the staged download for recovery.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::manifest::{
    validate_signed_package, MAX_BUNDLE_BYTES, MAX_EXECUTABLE_BYTES, MAX_MANIFEST_BYTES,
};
use super::verification::{self, TrustedRoot, VerifiedManifest};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod nsis;
pub use nsis::{prepare_nsis, PreparedNsis};

const HELPER_SWITCH: &str = "--beanfun-portable-update";
const STAGE_PREFIX: &str = ".beanfun-update-";
const HELPER_NAME: &str = "helper.exe";
const PAYLOAD_NAME: &str = "replacement.exe";
const BACKUP_NAME: &str = "previous.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum InstallationKind {
    Portable,
    Nsis,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Automatic installation is only available on Windows")]
    UnsupportedPlatform,
    #[error("Another Beanfun instance is using this executable; close it and try again")]
    OtherInstances,
    #[error("The downloaded update could not be authenticated: {0}")]
    Signature(String),
    #[error("Update trust material could not be authenticated: {0}")]
    Trust(String),
    #[error("The portable update plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("Portable update I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("The update helper could not start: {0}")]
    Helper(String),
    #[error("The updated executable could not start; the previous version was restored: {0}")]
    Restart(String),
    #[error("Update recovery failed; the previous executable is preserved at {backup}: {reason}")]
    Recovery { backup: PathBuf, reason: String },
}

impl InstallError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OtherInstances => "update.multiple_instances",
            Self::Signature(_) => "update.signature_invalid",
            Self::Trust(_) => "update.trust_unavailable",
            Self::UnsupportedPlatform => "update.platform_unsupported",
            _ => "update.install_failed",
        }
    }
}

/// Match Tauri's NSIS uninstall registration to this executable's actual path.
/// An installed copy elsewhere must never turn a portable copy into an install.
pub fn detect_installation() -> Result<InstallationKind, InstallError> {
    #[cfg(target_os = "windows")]
    {
        windows_support::detect(&std::env::current_exe()?.canonicalize()?)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(InstallError::UnsupportedPlatform)
    }
}

/// Refuse installation while another process uses this exact executable.
/// NSIS callers use the same check before handing control to the installer.
pub fn ensure_single_instance() -> Result<(), InstallError> {
    let target = std::env::current_exe()?.canonicalize()?;
    ensure_no_other_instances(&target, std::process::id())
}

/// The pinned NSIS template closes applications by executable name, including
/// portable copies in other directories. Refuse to disturb those instances.
fn ensure_installer_single_instance() -> Result<(), InstallError> {
    #[cfg(target_os = "windows")]
    {
        windows_support::ensure_no_other_named_instances(
            &PathBuf::from("beanfun.exe"),
            std::process::id(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(InstallError::UnsupportedPlatform)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdatePlan {
    parent_pid: u32,
    target_name: PathBuf,
    original_sha256: String,
    nonce: String,
    trust_cache: PathBuf,
}

pub struct PreparedPortable {
    stage: PathBuf,
    nonce: String,
    original_sha256: String,
    manifest_sha256: String,
    bundle_sha256: String,
}

/// Stage a verified download without modifying the current executable.
/// Callers must only exit after `launch` confirms the helper is ready.
pub fn prepare_portable(
    bytes: &[u8],
    manifest: &[u8],
    bundle: &[u8],
    root: &TrustedRoot,
    trust_cache: &Path,
) -> Result<PreparedPortable, InstallError> {
    if !cfg!(target_os = "windows") {
        return Err(InstallError::UnsupportedPlatform);
    }
    verify_download(bytes, manifest, bundle, root, InstallationKind::Portable)?;
    if detect_installation()? != InstallationKind::Portable {
        return Err(InstallError::InvalidPlan(
            "this is an NSIS installation".into(),
        ));
    }
    let target = std::env::current_exe()?.canonicalize()?;
    reject_link(&target)?;
    ensure_no_other_instances(&target, std::process::id())?;
    stage_download(&target, bytes, manifest, bundle, trust_cache)
}

fn stage_download(
    target: &Path,
    bytes: &[u8],
    manifest: &[u8],
    bundle: &[u8],
    trust_cache: &Path,
) -> Result<PreparedPortable, InstallError> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid("missing executable directory"))?;
    let mut random = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut random);
    let nonce = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let stage = parent.join(format!("{STAGE_PREFIX}{nonce}"));
    // Creating the staging directory on the same volume also tests whether the
    // portable directory is writable. Existing files are never overwritten.
    fs::create_dir(&stage)?;
    let original = read_limited(target, MAX_EXECUTABLE_BYTES)?;
    write_new(&stage.join(HELPER_NAME), &original)?;
    write_new(&stage.join(PAYLOAD_NAME), bytes)?;
    write_new(&stage.join("latest.json"), manifest)?;
    write_new(&stage.join("latest.json.sigstore.json"), bundle)?;
    let plan = UpdatePlan {
        parent_pid: std::process::id(),
        target_name: PathBuf::from(
            target
                .file_name()
                .ok_or_else(|| invalid("missing filename"))?,
        ),
        original_sha256: sha256(&original),
        nonce: nonce.clone(),
        trust_cache: trust_cache.to_owned(),
    };
    let serialised = serde_json::to_vec(&plan).map_err(|e| invalid(e.to_string()))?;
    write_new(&stage.join("plan.json"), &serialised)?;
    Ok(PreparedPortable {
        stage,
        nonce,
        original_sha256: plan.original_sha256,
        manifest_sha256: sha256(manifest),
        bundle_sha256: sha256(bundle),
    })
}

impl PreparedPortable {
    /// Start a separate copy of this executable in helper mode and wait until
    /// it has validated its parent and download. No shell parses these paths.
    pub fn launch(self) -> Result<(), InstallError> {
        // Authenticate the helper before executing it. Its later self-check
        // cannot protect a helper that has already been replaced on disk.
        let _guard =
            lock_verified_executable(&self.stage.join(HELPER_NAME), &self.original_sha256)?;
        // Keep the approved release metadata fixed until the helper has read
        // and authenticated it, including against another valid signed release.
        let _manifest_guard =
            lock_verified_executable(&self.stage.join("latest.json"), &self.manifest_sha256)?;
        let _bundle_guard = lock_verified_executable(
            &self.stage.join("latest.json.sigstore.json"),
            &self.bundle_sha256,
        )?;
        let mut command = Command::new(self.stage.join(HELPER_NAME));
        command
            .arg(HELPER_SWITCH)
            .arg(&self.nonce)
            .current_dir(&self.stage);
        configure_process(&mut command);
        let mut child = command
            .spawn()
            .map_err(|e| InstallError::Helper(e.to_string()))?;
        let started = Instant::now();
        loop {
            if self.stage.join("ready").is_file() {
                return Ok(());
            }
            if let Ok(error) = fs::read_to_string(self.stage.join("error.txt")) {
                return Err(InstallError::Helper(error));
            }
            if let Some(status) = child.try_wait()? {
                return Err(InstallError::Helper(format!("helper exited with {status}")));
            }
            if started.elapsed() > Duration::from_secs(20) {
                // A helper that has not acknowledged readiness must never be
                // left waiting to replace the application at a later shutdown.
                let _ = child.kill();
                let _ = child.wait();
                return Err(InstallError::Helper("helper readiness timed out".into()));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Handle the private helper entry point before constructing Tauri/WebView2.
pub fn run_helper_if_requested() -> Option<i32> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(HELPER_SWITCH)) {
        return None;
    }
    let result = match (arguments.next(), arguments.next()) {
        (Some(nonce), None) => nonce
            .to_str()
            .ok_or_else(|| invalid("non-text nonce"))
            .and_then(run_helper),
        _ => Err(invalid("invalid helper arguments")),
    };
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            if let Ok(executable) = std::env::current_exe() {
                if let Some(stage) = executable.parent() {
                    // The stage is created by this update attempt. Keep the
                    // failure and previous executable available for recovery.
                    let _ = write_new(&stage.join("error.txt"), error.to_string().as_bytes());
                }
            }
            show_failure(&error.to_string());
            1
        }
    })
}

fn run_helper(nonce: &str) -> Result<(), InstallError> {
    if !cfg!(target_os = "windows") {
        return Err(InstallError::UnsupportedPlatform);
    }
    let helper = std::env::current_exe()?.canonicalize()?;
    let stage = helper
        .parent()
        .ok_or_else(|| invalid("missing helper directory"))?;
    let plan: UpdatePlan = serde_json::from_slice(&read_limited(&stage.join("plan.json"), 16384)?)
        .map_err(|e| invalid(e.to_string()))?;
    let target = validate_plan(stage, &helper, nonce, &plan)?;
    let parent = ParentProcess::open(plan.parent_pid, &target)?;
    ensure_no_other_instances(&target, plan.parent_pid)?;
    let payload = read_limited(&stage.join(PAYLOAD_NAME), MAX_EXECUTABLE_BYTES)?;
    let manifest = read_limited(&stage.join("latest.json"), MAX_MANIFEST_BYTES)?;
    let bundle = read_limited(&stage.join("latest.json.sigstore.json"), MAX_BUNDLE_BYTES)?;
    // The helper works offline, but still revalidates the complete cached TUF
    // chain and expiry against its compiled bootstrap before acknowledging.
    let root = cached_trust_root(&plan.trust_cache)?;
    verify_download(
        &payload,
        &manifest,
        &bundle,
        &root,
        InstallationKind::Portable,
    )?;
    write_new(&stage.join("ready"), b"ready")?;
    parent.wait()?;
    ensure_no_other_instances(&target, std::process::id())?;
    // Re-read after the wait. A download altered while the application closed
    // must not be installed merely because its previous bytes verified.
    let payload = read_limited(&stage.join(PAYLOAD_NAME), MAX_EXECUTABLE_BYTES)?;
    verify_download(
        &payload,
        &manifest,
        &bundle,
        &root,
        InstallationKind::Portable,
    )?;
    validate_plan(stage, &helper, nonce, &plan)?;
    replace_and_restart(
        &target,
        stage,
        &plan.original_sha256,
        &sha256(&payload),
        start_application,
    )
}

fn validate_plan(
    stage: &Path,
    helper: &Path,
    nonce: &str,
    plan: &UpdatePlan,
) -> Result<PathBuf, InstallError> {
    if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) || plan.nonce != nonce {
        return Err(invalid("nonce mismatch"));
    }
    if stage.file_name() != Some(std::ffi::OsStr::new(&format!("{STAGE_PREFIX}{nonce}")))
        || helper != stage.join(HELPER_NAME)
    {
        return Err(invalid("unexpected helper location"));
    }
    let mut components = plan.target_name.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(invalid(
            "target must be a filename in the original directory",
        ));
    }
    if !plan
        .target_name
        .extension()
        .is_some_and(|e| e.as_encoded_bytes().eq_ignore_ascii_case(b"exe"))
    {
        return Err(invalid("target is not an executable"));
    }
    reject_link(stage)?;
    if !plan.trust_cache.is_absolute() {
        return Err(invalid("trust cache path must be absolute"));
    }
    for name in [
        HELPER_NAME,
        PAYLOAD_NAME,
        "plan.json",
        "latest.json",
        "latest.json.sigstore.json",
    ] {
        reject_link(&stage.join(name))?;
    }
    let target = stage
        .parent()
        .ok_or_else(|| invalid("missing target directory"))?
        .join(&plan.target_name);
    reject_link(&target)?;
    if sha256(&read_limited(helper, MAX_EXECUTABLE_BYTES)?) != plan.original_sha256
        || sha256(&read_limited(&target, MAX_EXECUTABLE_BYTES)?) != plan.original_sha256
    {
        return Err(invalid("original executable changed"));
    }
    Ok(target)
}

/// The filesystem transaction is separate from process handling so failure
/// recovery is exercised with ordinary files, never the real application.
fn replace_and_restart(
    target: &Path,
    stage: &Path,
    expected_original: &str,
    expected_replacement: &str,
    mut start: impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), InstallError> {
    if sha256(&read_limited(target, MAX_EXECUTABLE_BYTES)?) != expected_original {
        return Err(invalid("original executable changed before replacement"));
    }
    let backup = stage.join(BACKUP_NAME);
    let replacement = stage.join(PAYLOAD_NAME);
    if backup.exists() || stage.join("failed.exe").exists() {
        return Err(invalid("recovery files already exist"));
    }
    if let Err(error) = fs::rename(target, &backup) {
        // The parent has already exited. If a file lock prevents replacement,
        // restart the authenticated original rather than leave it closed.
        start_verified(target, expected_original, &mut start).map_err(|restart| {
            InstallError::Recovery {
                backup: target.to_owned(),
                reason: format!("replacement: {error}; restart original: {restart}"),
            }
        })?;
        return Err(error.into());
    }
    if let Err(error) = fs::rename(&replacement, target) {
        restore_backup(&backup, target, expected_original).map_err(|restore| {
            InstallError::Recovery {
                backup: backup.clone(),
                reason: format!("replacement: {error}; restore: {restore}"),
            }
        })?;
        start_verified(target, expected_original, &mut start).map_err(|restart| {
            InstallError::Recovery {
                backup: backup.clone(),
                reason: format!("replacement: {error}; restart original: {restart}"),
            }
        })?;
        return Err(InstallError::Io(error));
    }
    // The pathname may have changed since signature verification. Authenticate
    // the actual target through an exclusive-against-write/delete handle and
    // retain it until CreateProcess has opened the same executable.
    if let Err(error) = start_verified(target, expected_replacement, &mut start) {
        fs::rename(target, stage.join("failed.exe")).map_err(|restore| InstallError::Recovery {
            backup: backup.clone(),
            reason: format!("start: {error}; move failed update: {restore}"),
        })?;
        // Copy instead of consuming the backup so manual recovery remains
        // possible even if the restored application cannot be started.
        restore_backup(&backup, target, expected_original).map_err(|restore| {
            InstallError::Recovery {
                backup: backup.clone(),
                reason: format!("start: {error}; restore: {restore}"),
            }
        })?;
        if let Err(restart) = start_verified(target, expected_original, &mut start) {
            return Err(InstallError::Recovery {
                backup,
                reason: format!("start: {error}; restart previous version: {restart}"),
            });
        }
        return Err(InstallError::Restart(error.to_string()));
    }
    Ok(())
}

fn restore_backup(backup: &Path, target: &Path, expected: &str) -> Result<(), InstallError> {
    let original = read_limited(backup, MAX_EXECUTABLE_BYTES)?;
    if sha256(&original) != expected {
        return Err(invalid("the recovery executable changed"));
    }
    // Refuse to follow or overwrite a new file placed at the target meanwhile.
    // The retained backup remains available even if restoration cannot finish.
    write_new(target, &original)?;
    Ok(())
}

fn start_verified(
    target: &Path,
    expected: &str,
    start: &mut impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), InstallError> {
    let _guard = lock_verified_executable(target, expected)?;
    start(target)?;
    Ok(())
}

fn lock_verified_executable(target: &Path, expected: &str) -> Result<File, InstallError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ};
        // Deny writes and deletion for this file object, including renaming
        // it away and replacing the path between hashing and CreateProcess.
        options
            .share_mode(FILE_SHARE_READ.0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
    }
    let mut file = options.open(target)?;
    let metadata = file.metadata()?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("the executable is a reparse point"));
        }
    }
    if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_BYTES {
        return Err(invalid("unexpected executable size or type"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_EXECUTABLE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_EXECUTABLE_BYTES || sha256(&bytes) != expected {
        return Err(invalid("the executable changed before launch"));
    }
    Ok(file)
}

fn cached_trust_root(cache: &Path) -> Result<TrustedRoot, InstallError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime
        .block_on(verification::load_cached_trust_root(cache))
        .map_err(|e| InstallError::Trust(e.to_string()))
}

fn verify_download(
    bytes: &[u8],
    manifest: &[u8],
    bundle: &[u8],
    root: &TrustedRoot,
    kind: InstallationKind,
) -> Result<String, InstallError> {
    let verified = VerifiedManifest::verify(manifest, bundle, root)
        .map_err(|e| InstallError::Signature(e.to_string()))?;
    let package = validate_signed_package(manifest, &verified, kind, env!("CARGO_PKG_VERSION"))
        .map_err(|e| InstallError::Signature(e.to_string()))?;
    let digest = sha256(bytes);
    if bytes.len() as u64 != package.size || digest != package.sha256 {
        return Err(InstallError::Signature(
            "download digest or size differs from the signed manifest".into(),
        ));
    }
    validate_executable(bytes)?;
    Ok(digest)
}

fn validate_executable(bytes: &[u8]) -> Result<(), InstallError> {
    if bytes.len() < 64 || bytes.len() as u64 > MAX_EXECUTABLE_BYTES || &bytes[..2] != b"MZ" {
        return Err(invalid("download is not a Windows executable"));
    }
    let offset = u32::from_le_bytes(bytes[60..64].try_into().expect("four bytes")) as usize;
    if bytes.get(offset..offset.saturating_add(4)) != Some(b"PE\0\0") {
        return Err(invalid("download has no PE header"));
    }
    Ok(())
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, InstallError> {
    reject_link(path)?;
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit {
        return Err(invalid("unexpected update file size or type"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("update file grew beyond its limit"));
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn reject_link(path: &Path) -> Result<(), InstallError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid("symbolic links are not allowed in an update plan"));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("reparse points are not allowed in an update plan"));
        }
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn invalid(message: impl Into<String>) -> InstallError {
    InstallError::InvalidPlan(message.into())
}

fn configure_process(command: &mut Command) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}

fn start_application(path: &Path) -> io::Result<()> {
    let mut command = Command::new(path);
    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }
    configure_process(&mut command);
    let mut child = command.spawn()?;
    // Catch immediate loader failures or crashes, without waiting for the
    // user's next application exit or imposing a new application protocol.
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(1) {
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "application exited during startup: {status}"
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
use windows_support::{ensure_no_other_instances, show_failure, ParentProcess};

#[cfg(not(target_os = "windows"))]
fn ensure_no_other_instances(_: &Path, _: u32) -> Result<(), InstallError> {
    Err(InstallError::UnsupportedPlatform)
}
#[cfg(not(target_os = "windows"))]
struct ParentProcess;
#[cfg(not(target_os = "windows"))]
impl ParentProcess {
    fn open(_: u32, _: &Path) -> Result<Self, InstallError> {
        Err(InstallError::UnsupportedPlatform)
    }
    fn wait(&self) -> Result<(), InstallError> {
        Err(InstallError::UnsupportedPlatform)
    }
}
#[cfg(not(target_os = "windows"))]
fn show_failure(message: &str) {
    eprintln!("Beanfun update failed: {message}");
}

#[cfg(target_os = "windows")]
mod windows_support {
    use super::*;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
    use winreg::RegKey;

    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns a successfully opened handle.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    pub(super) fn detect(executable: &Path) -> Result<InstallationKind, InstallError> {
        // These keys/values come from Tauri's official NSIS installer.nsi:
        // Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}.
        // Both registry views are read because older installers may use either.
        const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Beanfun";
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                let key = match RegKey::predef(hive).open_subkey_with_flags(KEY, KEY_READ | view) {
                    Ok(key) => key,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                let location: String = key.get_value("InstallLocation")?;
                let icon: String = key.get_value("DisplayIcon")?;
                let uninstaller: String = key.get_value("UninstallString")?;
                if registration_matches(executable, &location, &icon, &uninstaller) {
                    return Ok(InstallationKind::Nsis);
                }
            }
        }
        Ok(InstallationKind::Portable)
    }

    fn path_matches(a: &Path, b: &Path) -> bool {
        match (a.canonicalize(), b.canonicalize()) {
            (Ok(a), Ok(b)) => name_matches(a.as_os_str(), b.as_os_str()),
            _ => false,
        }
    }

    fn name_matches(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
        let a: Vec<u16> = a.encode_wide().collect();
        let b: Vec<u16> = b.encode_wide().collect();
        // SAFETY: the borrowed UTF-16 slices remain valid throughout the call.
        unsafe { CompareStringOrdinal(&a, &b, true) == CSTR_EQUAL }
    }

    pub(super) fn registration_matches(
        executable: &Path,
        location: &str,
        icon: &str,
        uninstaller: &str,
    ) -> bool {
        let location = Path::new(location.trim_matches('"'));
        let icon = Path::new(icon.trim_matches('"'));
        let uninstaller = Path::new(uninstaller.trim_matches('"'));
        executable
            .parent()
            .is_some_and(|parent| path_matches(parent, location))
            && path_matches(executable, icon)
            && path_matches(uninstaller, &location.join("uninstall.exe"))
    }

    fn image_path(handle: HANDLE) -> Result<PathBuf, InstallError> {
        let mut buffer = vec![0_u16; 32768];
        let mut length = buffer.len() as u32;
        // SAFETY: the buffer and length remain valid for the duration of the call.
        unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|e| InstallError::Helper(e.to_string()))?;
        Ok(PathBuf::from(std::ffi::OsString::from_wide(
            &buffer[..length as usize],
        )))
    }

    pub(super) struct ParentProcess(OwnedHandle);
    impl ParentProcess {
        pub(super) fn open(pid: u32, target: &Path) -> Result<Self, InstallError> {
            if pid == 0 || pid == std::process::id() {
                return Err(invalid("invalid parent process"));
            }
            if actual_parent_pid()? != pid {
                return Err(invalid(
                    "update plan does not identify the helper's actual parent",
                ));
            }
            // SAFETY: the resulting handle is owned and closed by OwnedHandle.
            let handle = OwnedHandle(
                unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                        false,
                        pid,
                    )
                }
                .map_err(|e| InstallError::Helper(e.to_string()))?,
            );
            if !path_matches(&image_path(handle.0)?, target) {
                return Err(invalid("parent process does not own the target executable"));
            }
            Ok(Self(handle))
        }

        pub(super) fn wait(&self) -> Result<(), InstallError> {
            // Waiting on the existing handle, rather than polling a PID,
            // prevents PID reuse from being mistaken for the original process.
            let status = unsafe { WaitForSingleObject(self.0 .0, 60_000) };
            if status != WAIT_OBJECT_0 {
                return Err(InstallError::Helper(
                    "application did not exit within 60 seconds".into(),
                ));
            }
            Ok(())
        }
    }

    fn actual_parent_pid() -> Result<u32, InstallError> {
        // SAFETY: the snapshot is owned below and entry has its required size.
        let snapshot = OwnedHandle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|e| InstallError::Helper(e.to_string()))?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        unsafe { Process32FirstW(snapshot.0, &mut entry) }
            .map_err(|e| InstallError::Helper(e.to_string()))?;
        loop {
            if entry.th32ProcessID == std::process::id() {
                return Ok(entry.th32ParentProcessID);
            }
            if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
                break;
            }
        }
        Err(invalid(
            "helper process is absent from its process snapshot",
        ))
    }

    pub(super) fn ensure_no_other_instances(
        target: &Path,
        allowed_pid: u32,
    ) -> Result<(), InstallError> {
        check_instances(target, allowed_pid, false)
    }

    pub(super) fn ensure_no_other_named_instances(
        target: &Path,
        allowed_pid: u32,
    ) -> Result<(), InstallError> {
        check_instances(target, allowed_pid, true)
    }

    fn check_instances(
        target: &Path,
        allowed_pid: u32,
        any_matching_name: bool,
    ) -> Result<(), InstallError> {
        // SAFETY: the snapshot is owned below and entry has its required size.
        let snapshot = OwnedHandle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|e| InstallError::Helper(e.to_string()))?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        unsafe { Process32FirstW(snapshot.0, &mut entry) }
            .map_err(|e| InstallError::Helper(e.to_string()))?;
        loop {
            if entry.th32ProcessID != allowed_pid && entry.th32ProcessID != std::process::id() {
                let length = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = std::ffi::OsString::from_wide(&entry.szExeFile[..length]);
                if target
                    .file_name()
                    .is_some_and(|expected| name_matches(expected, &name))
                {
                    if any_matching_name {
                        return Err(InstallError::OtherInstances);
                    }
                    let handle = unsafe {
                        OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION,
                            false,
                            entry.th32ProcessID,
                        )
                    };
                    // Fail closed for an inaccessible process with the same
                    // image name; it may be another elevated Beanfun instance.
                    let handle = handle
                        .map(OwnedHandle)
                        .map_err(|_| InstallError::OtherInstances)?;
                    if path_matches(&image_path(handle.0)?, target) {
                        return Err(InstallError::OtherInstances);
                    }
                }
            }
            if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn show_failure(message: &str) {
        let body: Vec<u16> = std::ffi::OsStr::new(&format!("Beanfun update failed.\n\n{message}"))
            .encode_wide()
            .chain(Some(0))
            .collect();
        let title: Vec<u16> = std::ffi::OsStr::new("Beanfun")
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: both strings are NUL-terminated and live through MessageBoxW.
        unsafe {
            let _ = MessageBoxW(
                None,
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
