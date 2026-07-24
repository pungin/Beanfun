//! MapleStory Classic (新楓之谷經典版 / "mstc") login + launch (issue: 懷舊服).
//!
//! Classic runs on the Gamania **galaxy** login gateway + Nexon Game
//! Manager (NGM) — a completely different path from the regular game's
//! LR/OTP launch. Ported from MapleLink's working `classic_service.rs`,
//! simplified for Beanfun's single-session model: the already-logged-in
//! `BeanfunClient` cookie jar (carrying `bfWebToken`) is seeded into a
//! hidden webview which drives the whole SSO without user interaction.
//!
//! # Flow
//!
//! 1. Hidden webview navigates to the galaxy entry
//!    (`/webapi/view/login/mstc?redirect_url=…`), which 302s to the OTT
//!    init page (`/login/init/mstc/OTT:944:Login:<token>`; the page
//!    stores the OTT in `localStorage['LOGIN_OTT_mstc']`).
//! 2. The init page offers two sign-in buttons; the injected script
//!    auto-clicks the one matching the session's region —
//!    `.btnLogin-beanfun` (HK, service `610076_T0`) or
//!    `.btnLogin-gamapass` (TW GamaPass).
//! 3. The seeded `bfWebToken` makes beanfun SSO straight through
//!    (`return.aspx` → galaxy `…/login/result/mstc/ghk?WebToken=…`,
//!    which binds the WebToken to the OTT server-side) and on to
//!    `maplestoryclassic.beanfun.com/Main?OTT=<ott>`.
//! 4. The Main page POSTs `api/Login/GetOneTimeWebInfo` itself and
//!    auto-fires an `ngm://launch/…` URL. WebView2 would show an "open
//!    Nexon Game Manager?" prompt for that — and our window is hidden —
//!    so `cookie_native::register_external_uri_handler` cancels the
//!    prompt and [`launch_ngm`] starts NGM directly from its registered
//!    handler (`HKCR\ngm\shell\open\command`), no manual click.
//!
//! # Degradation ladder
//!
//! - Interception unavailable (WebView2 runtime < 111): reveal the
//!   window; the user completes the prompt by hand.
//! - NGM not installed: the portal shows the official install guide
//!   (detected via the `NGMMISSING` title marker set by the injected
//!   script) — reveal the window so the user gets that guide, and emit
//!   `classic-launch-failed`.
//! - A user-provided NGM path (Config.xml `classicNgmPath`) overrides
//!   registry lookup when auto-detection fails (set in Settings).

use tauri::Manager;

use super::error::CommandError;
use super::state::AppState;

/// Galaxy classic (mstc) login entry. Issues a fresh OTT, stores it in
/// the page's localStorage and redirects to the init page (whose login
/// button the injected script auto-clicks); SSO via the seeded
/// `bfWebToken` then flows through to the portal, which fires its own
/// `ngm://` launch on arrival.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// Window label for the (usually hidden) classic portal webview.
const CLASSIC_WINDOW_LABEL: &str = "classic-login";

/// Title marker the injected script sets when the portal shows the NGM
/// install guide (Nexon Game Manager isn't installed), so the poll task
/// can fail fast instead of waiting out the timeout.
const MISSING_MARKER: &str = "NGMMISSING";

/// Event emitted when NGM was started successfully (portal closed).
pub const CLASSIC_LAUNCHED_EVENT: &str = "classic-launched";
/// Event emitted when the launch failed (NGM missing / spawn error).
pub const CLASSIC_FAILED_EVENT: &str = "classic-launch-failed";
/// Event emitted when no launch fired within the poll window.
pub const CLASSIC_TIMEOUT_EVENT: &str = "classic-launch-timeout";

/// `ui.window_create_failed` twin for the classic portal window.
const WINDOW_FAILED_CODE: &str = "classic.window_create_failed";

/// Injected on every navigation of the classic portal window. On the
/// OTT init page it clicks the region's login button to drive the SSO;
/// on the portal Main page it watches for the NGM install guide (shown
/// instead of the launch when NGM is missing) and flags it via the
/// window title. A no-op on every other page.
#[cfg(target_os = "windows")]
fn auto_login_script(region: crate::services::beanfun::client::LoginRegion) -> String {
    use crate::services::beanfun::client::LoginRegion;
    // HK accounts use the HK-beanfun button; TW (GamaPass) uses Gama Pass.
    let selector = match region {
        LoginRegion::HK => ".btnLogin-beanfun",
        LoginRegion::TW => ".btnLogin-gamapass",
    };
    format!(
        r#"
(function () {{
  var clicked = false, flagged = false;
  function tick() {{
    var href = location.href;
    if (href.indexOf('/login/init/mstc/') !== -1) {{
      if (!clicked) {{
        var btn = document.querySelector('{selector}');
        if (btn) {{ btn.click(); clicked = true; }}
      }}
    }} else if (href.indexOf('maplestoryclassic.beanfun.com/Main') !== -1) {{
      if (!flagged &&
          (document.getElementById('ngmBtnStart') ||
           document.getElementById('ngmInstallLayerClose'))) {{
        flagged = true;
        document.title = '{MISSING_MARKER}';
      }}
    }}
  }}
  setInterval(tick, 300);
}})();
"#
    )
}

/// Parse a registered protocol handler command (`"exe" "%1"` / `exe %1`)
/// into the executable and its arguments, substituting the URL for
/// every `%1`. Returns `None` for an empty/garbled command string.
#[cfg(target_os = "windows")]
fn parse_handler_command(command: &str, url: &str) -> Option<(String, Vec<String>)> {
    let command = command.trim();
    let (exe, rest) = if let Some(after) = command.strip_prefix('"') {
        let end = after.find('"')?;
        (after[..end].to_string(), &after[end + 1..])
    } else {
        let end = command.find(' ').unwrap_or(command.len());
        (command[..end].to_string(), &command[end..])
    };
    if exe.is_empty() {
        return None;
    }
    let args = rest
        .split_whitespace()
        .map(|a| a.trim_matches('"').replace("%1", url))
        .collect::<Vec<_>>();
    // Handler declares no %1 slot → pass the URL as a trailing argument.
    let args = if args.iter().any(|a| a.contains(url)) {
        args
    } else {
        vec![url.to_string()]
    };
    Some((exe, args))
}

/// Start Nexon Game Manager for a captured `ngm://` URL by invoking its
/// registered handler directly (`HKCR\ngm\shell\open\command`).
///
/// Deliberately no `ShellExecute` fallback: we are called from inside
/// the intercept that just cancelled WebView2's prompt, and handing the
/// URL back to the shell would only pop the same prompt again. If NGM
/// isn't registered this fails and the caller reveals the portal (which
/// shows the official install guide).
#[cfg(target_os = "windows")]
fn launch_ngm(url: &str, manual_path: Option<&str>) -> Result<(), String> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    // A user-provided NGM path (Settings → classicNgmPath) wins.
    if let Some(path) = manual_path {
        if !path.is_empty() && std::path::Path::new(path).exists() {
            std::process::Command::new(path)
                .arg(url)
                .spawn()
                .map_err(|e| format!("failed to launch NGM ({path}): {e}"))?;
            tracing::info!("classic: launched NGM (manual path {path})");
            return Ok(());
        }
    }

    let command: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"ngm\shell\open\command")
        .and_then(|k| k.get_value(""))
        .map_err(|e| {
            format!("ngm handler not registered (is Nexon Game Manager installed?): {e}")
        })?;

    let (exe, args) = parse_handler_command(&command, url)
        .ok_or_else(|| format!("could not parse ngm handler command: {command}"))?;

    std::process::Command::new(&exe)
        .args(&args)
        .spawn()
        .map_err(|e| format!("failed to launch NGM ({exe}): {e}"))?;
    tracing::info!("classic: launched NGM directly ({exe})");
    Ok(())
}

/// Result of the classic-readiness self-check (Settings → Classic).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClassicCheck {
    /// Nexon Game Manager's `ngm://` protocol handler is registered
    /// (or a valid manual path is configured).
    pub ngm_registered: bool,
    /// The handler's executable path, if readable.
    pub ngm_exe: Option<String>,
    /// That executable actually exists on disk.
    pub ngm_exe_exists: bool,
}

/// Check the local prerequisites for the classic launch. A non-empty,
/// existing `classicNgmPath` from Config.xml counts as NGM available.
#[tauri::command]
#[specta::specta]
pub async fn classic_self_check(state: tauri::State<'_, AppState>) -> Result<ClassicCheck, ()> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CLASSES_ROOT;
        use winreg::RegKey;

        let mut check = ClassicCheck::default();
        if let Ok(command) = RegKey::predef(HKEY_CLASSES_ROOT)
            .open_subkey(r"ngm\shell\open\command")
            .and_then(|k| k.get_value::<String, _>(""))
        {
            check.ngm_registered = true;
            if let Some((exe, _)) = parse_handler_command(&command, "") {
                check.ngm_exe_exists = std::path::Path::new(&exe).exists();
                check.ngm_exe = Some(exe);
            }
        }

        // Fall back to the user-provided path when detection came up empty.
        if !(check.ngm_registered && check.ngm_exe_exists) {
            let manual = manual_ngm_path(&state);
            if !manual.is_empty() && std::path::Path::new(&manual).exists() {
                check.ngm_registered = true;
                check.ngm_exe = Some(manual);
                check.ngm_exe_exists = true;
            }
        }
        Ok(check)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        Ok(ClassicCheck::default())
    }
}

/// Read the user-configured NGM executable path (empty when unset).
#[cfg(target_os = "windows")]
fn manual_ngm_path(state: &AppState) -> String {
    crate::services::config::get_value_sync(
        &state.storage_root.join("Config.xml"),
        "classicNgmPath",
    )
    .unwrap_or_default()
}

/// Open the classic portal for the already-authenticated beanfun
/// session and auto-launch the game once the SSO lands.
///
/// Beanfun is single-session, so unlike MapleLink there is no session
/// id to resolve — the one `AppState.auth` context is the session, and
/// its cookie jar (with `bfWebToken`) is seeded straight into the
/// portal webview. Works for HK (account/password) and TW GamaPass
/// sessions; the frontend gates the button accordingly (a TW
/// account/password or QR session can't drive the galaxy SSO).
#[tauri::command]
#[specta::specta]
pub async fn open_classic_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        open_classic_login_windows(app, state).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state);
        Err(CommandError::new(
            "classic.platform_unsupported",
            "MapleStory Classic launch is only supported on Windows",
        ))
    }
}

#[cfg(target_os = "windows")]
async fn open_classic_login_windows<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    use tauri::Emitter;

    use super::cookie_native;

    // Launch state shared between the intercept callback and the poll task.
    const PENDING: u8 = 0;
    const LAUNCHED: u8 = 1;
    const FAILED: u8 = 2;

    let (client, session) = super::session::require_auth(&state).await?;
    let init_script = auto_login_script(session.region);
    let manual_ngm = manual_ngm_path(&state);

    // A previous portal window (e.g. a launch the user re-triggered)
    // must go first — labels are unique.
    if let Some(existing) = app.get_webview_window(CLASSIC_WINDOW_LABEL) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let about_blank: tauri::Url = "about:blank".parse().expect("about:blank is a valid URL");
    let mut builder = tauri::WebviewWindowBuilder::new(
        &app,
        CLASSIC_WINDOW_LABEL,
        tauri::WebviewUrl::External(about_blank),
    )
    .title("新楓之谷：經典版")
    .inner_size(1024.0, 720.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false)
    .initialization_script(&init_script);
    // Same per-instance WebView2 profile as the main window (issue
    // #340) so multi-instance users can't hit the shared-browser-process
    // ERROR_INVALID_STATE on this window either.
    if let Some(dir) = crate::current_instance_webview_dir() {
        builder = builder.data_directory(dir);
    }
    let window = builder.build().map_err(|e| {
        CommandError::new(
            WINDOW_FAILED_CODE,
            format!("failed to create classic portal window: {e}"),
        )
    })?;

    // Seed the session's beanfun cookies (incl. HttpOnly bfWebToken) so
    // the SSO step skips re-login, and keep popups in-window.
    cookie_native::register_new_window_handler(&window);
    let seeded = cookie_native::seed_cookies_native(&window, &client);
    tracing::info!("classic: seeded {seeded} cookies into portal webview");

    // Intercept the portal's own ngm:// launch: cancel WebView2's
    // prompt and start NGM ourselves. The flag lets the poll task react
    // (close on success, reveal for manual launch on failure).
    let flag = Arc::new(AtomicU8::new(PENDING));
    let flag_cb = flag.clone();
    let intercept_ok = cookie_native::register_external_uri_handler(&window, move |url| {
        if !(url.starts_with("ngm:") || url.starts_with("nexonplug:")) {
            return;
        }
        let manual = (!manual_ngm.is_empty()).then_some(manual_ngm.as_str());
        let outcome = match launch_ngm(url, manual) {
            Ok(()) => LAUNCHED,
            Err(e) => {
                tracing::warn!("classic: ngm launch failed: {e}");
                FAILED
            }
        };
        flag_cb.store(outcome, Ordering::SeqCst);
    })
    .inspect_err(|e| tracing::warn!("classic: external-uri interception unavailable: {e}"))
    .is_ok();

    let entry: tauri::Url = CLASSIC_ENTRY_URL
        .parse()
        .expect("classic entry URL is valid");
    if let Err(e) = window.navigate(entry) {
        let _ = window.destroy();
        return Err(CommandError::new(
            WINDOW_FAILED_CODE,
            format!("failed to navigate classic portal: {e}"),
        ));
    }

    // Without interception the prompt can't be suppressed — show the
    // window so the user can complete the launch by hand.
    if !intercept_ok {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    // Hidden auto-launch: wait for the intercept to fire, then close
    // (success) or reveal for manual completion (failure / timeout).
    tauri::async_runtime::spawn(async move {
        tracing::info!("classic portal running (hidden), waiting for launch");
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let Ok(title) = window.title() else {
                return; // window gone (user closed a revealed portal)
            };
            // NGM isn't installed — the portal shows the official
            // install guide instead of launching. Reveal it (the guide
            // includes the official download) and report now instead of
            // waiting out the timeout.
            if title == MISSING_MARKER {
                tracing::warn!("classic: NGM install guide shown — not installed");
                let _ = window.app_handle().emit(CLASSIC_FAILED_EVENT, ());
                let _ = window.show();
                let _ = window.set_focus();
                return;
            }
            match flag.load(Ordering::SeqCst) {
                LAUNCHED => {
                    let _ = window.app_handle().emit(CLASSIC_LAUNCHED_EVENT, ());
                    let _ = window.destroy();
                    return;
                }
                FAILED => {
                    let _ = window.app_handle().emit(CLASSIC_FAILED_EVENT, ());
                    let _ = window.show();
                    let _ = window.set_focus();
                    return;
                }
                _ => {}
            }
        }
        tracing::warn!("classic: no launch within timeout — revealing portal");
        let _ = window.app_handle().emit(CLASSIC_TIMEOUT_EVENT, ());
        let _ = window.show();
        let _ = window.set_focus();
    });

    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::services::beanfun::client::LoginRegion;

    #[test]
    fn parses_quoted_and_bare_handler_commands() {
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe" "%1""#, "ngm://x").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://x".to_string()]);

        let (exe, args) = parse_handler_command(r"C:\NGM\ngm.exe %1", "ngm://z").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://z".to_string()]);

        // No %1 slot → the URL is appended as a trailing argument.
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe""#, "ngm://y").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://y".to_string()]);
    }

    #[test]
    fn rejects_empty_handler_commands() {
        assert!(parse_handler_command("", "ngm://x").is_none());
        assert!(parse_handler_command(r#""""#, "ngm://x").is_none());
    }

    #[test]
    fn auto_login_script_targets_the_region_button() {
        let hk = auto_login_script(LoginRegion::HK);
        assert!(hk.contains(".btnLogin-beanfun"));
        assert!(!hk.contains(".btnLogin-gamapass"));

        let tw = auto_login_script(LoginRegion::TW);
        assert!(tw.contains(".btnLogin-gamapass"));
        assert!(!tw.contains(".btnLogin-beanfun"));

        // Both variants watch for the NGM install guide marker.
        assert!(hk.contains(MISSING_MARKER));
        assert!(tw.contains(MISSING_MARKER));
    }
}
