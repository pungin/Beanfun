//! Launch an authenticated NSIS installer using Tauri's documented update flags.

use super::*;

pub struct PreparedNsis {
    installer: PathBuf,
    manifest: Vec<u8>,
    bundle: Vec<u8>,
    trust_cache: PathBuf,
}

pub fn prepare_nsis(
    bytes: &[u8],
    manifest: &[u8],
    bundle: &[u8],
    root: &TrustedRoot,
    trust_cache: &Path,
) -> Result<PreparedNsis, InstallError> {
    if detect_installation()? != InstallationKind::Nsis {
        return Err(invalid("this is not an NSIS installation"));
    }
    verify_download(bytes, manifest, bundle, root, InstallationKind::Nsis)?;
    ensure_installer_single_instance()?;
    let mut random = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut random);
    let nonce = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let stage = std::env::temp_dir().join(format!(".beanfun-install-{nonce}"));
    fs::create_dir(&stage)?;
    let installer = stage.join("Beanfun-setup.exe");
    write_new(&installer, bytes)?;
    Ok(PreparedNsis {
        installer,
        manifest: manifest.to_vec(),
        bundle: bundle.to_vec(),
        trust_cache: trust_cache.to_owned(),
    })
}

impl PreparedNsis {
    /// Recheck the staged bytes and keep them locked until Windows has opened
    /// the installer. Cancellation or launch failure leaves the application open.
    pub fn launch(self) -> Result<(), InstallError> {
        ensure_installer_single_instance()?;
        if detect_installation()? != InstallationKind::Nsis {
            return Err(invalid("installation registration changed"));
        }
        let root = cached_trust_root(&self.trust_cache)?;
        let bytes = read_limited(&self.installer, MAX_EXECUTABLE_BYTES)?;
        let digest = verify_download(
            &bytes,
            &self.manifest,
            &self.bundle,
            &root,
            InstallationKind::Nsis,
        )?;
        let _guard = lock_verified_executable(&self.installer, &digest)?;
        launch_installer(&self.installer)
    }
}

#[cfg(not(target_os = "windows"))]
fn launch_installer(_: &Path) -> Result<(), InstallError> {
    Err(InstallError::UnsupportedPlatform)
}

#[cfg(target_os = "windows")]
fn launch_installer(path: &Path) -> Result<(), InstallError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut execution = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("open"),
        lpFile: PCWSTR(path.as_ptr()),
        // Passive UI, preserve existing data and restart after installation.
        // No renderer input or arbitrary application arguments enter this string.
        lpParameters: w!("/P /UPDATE /R"),
        nShow: SW_SHOW.0,
        ..Default::default()
    };
    // SAFETY: all strings live until the synchronous launch returns. The file
    // remains locked against writes and replacement throughout this operation.
    unsafe { ShellExecuteExW(&mut execution) }
        .map_err(|e| InstallError::Helper(format!("installer launch failed: {e}")))?;
    if execution.hProcess.is_invalid() {
        return Err(InstallError::Helper(
            "installer process was not created".into(),
        ));
    }
    // SAFETY: ShellExecuteEx returned this owned process handle.
    unsafe {
        let _ = CloseHandle(execution.hProcess);
    }
    Ok(())
}
