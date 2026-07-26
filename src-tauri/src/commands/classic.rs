//! MapleStory Classic (新楓之谷經典版 / 懷舊服 / "mstc") login + launch.
//!
//! Classic does **not** use the regular service's LR/OTP launch. It runs
//! on Gamania's **galaxy** login gateway and is ultimately started by
//! Nexon Game Manager (NGM) through the `ngm://` protocol.
//!
//! # Regions are NOT equivalent (measured)
//!
//! - **HK** — the same beanfun login covers both services, so a logged-in
//!   session's cookies (incl. `bfWebToken`) drive the whole SSO silently.
//! - **TW** — classic is a **separate login** from the regular service.
//!   A regular-service session grants nothing here; the portal always
//!   returns to a login form. So the TW flow shows the window from the
//!   start and the user signs in inside it. After that sign-in the
//!   GamaPass path has **two extra steps** (authorization consent, then
//!   game-account selection) that HK never sees.
//!
//! Classic also does not support +86 (mainland China) beanfun accounts —
//! HK accounts only. The UI states this up front.
//!
//! # Flow
//!
//! 1. Portal webview navigates the galaxy entry
//!    (`/webapi/view/login/mstc?redirect_url=…`), which mints a one-shot
//!    OTT, stores it in `localStorage` and redirects to
//!    `/login/init/mstc/OTT:944:Login:<token>`.
//! 2. That page offers two sign-in buttons; the injected script clicks
//!    the region's — `.btnLogin-beanfun` (HK, service `610076_T0`) or
//!    `.btnLogin-gamapass` (TW, `openid.beanfun.com`).
//! 3. HK: seeded cookies SSO straight through. TW: the user signs in,
//!    then the script accepts the authorization consent and picks the
//!    game account (auto when there is exactly one; otherwise the window
//!    is revealed so the user picks).
//! 4. galaxy exchanges a one-shot `access_token` for the OTT and lands on
//!    `maplestoryclassic.beanfun.com/Main`, which fires an
//!    `ngm://launch/…` URL itself.
//! 5. WebView2 would pop an "open Nexon Game Manager?" prompt for that,
//!    so [`super::cookie_native::register_external_uri_handler`] cancels
//!    the prompt and [`launch_ngm`] starts NGM directly from its
//!    registered handler (`HKCR\ngm\shell\open\command`). No shell
//!    fallback — that would just pop the prompt back.
//!
//! # Measured pitfalls this module encodes
//!
//! - **`document.title` does not reach the native window title.** The
//!   page→host signal goes through `WebMessageReceived`
//!   (`window.chrome.webview.postMessage`) instead; see
//!   [`super::cookie_native::register_web_message_handler`].
//! - **A webview built while the previous one is still tearing down**
//!   fails with `ERROR_INVALID_STATE`, and the builder may still hand
//!   back a window whose webview is dead — every handler then times out
//!   with no error. [`build_portal_window`] waits for the old window to
//!   actually disappear and treats the first COM handler registration as
//!   a liveness probe, rebuilding once if it fails.
//! - **Timeouts must be generous and must not lie.** An observed launch
//!   took 37 s. The poll keeps watching past the soft deadline: at that
//!   point it only reveals the window and reports "taking longer", and
//!   still emits success if the launch lands afterwards.
//! - **"Needs login" is detected by a password field, not the URL** —
//!   with a valid session the login hops redirect straight through.

use tauri::Manager;

use super::error::CommandError;
use super::state::AppState;
use crate::services::beanfun::client::LoginRegion;

/// Galaxy classic (mstc) login entry — mints the OTT and redirects to
/// the init page whose region button the injected script clicks.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// User-Agent for the portal webview. beanfun / galaxy pages behave
/// differently under the default WebView2 UA (Edg/WebView markers); this
/// plain-Chrome UA is the one the flow was verified against.
const CLASSIC_PORTAL_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// Window label for the classic portal webview.
const CLASSIC_WINDOW_LABEL: &str = "classic-login";

/// Official Nexon Game Manager installer, surfaced when NGM is missing.
pub const NGM_INSTALLER_URL: &str = "https://platform.nexon.com/NGM/Bin/Install_NGM.exe";

/// Emitted when NGM was started successfully.
pub const CLASSIC_LAUNCHED_EVENT: &str = "classic-launched";
/// Emitted when the launch definitively failed (NGM missing / spawn error).
pub const CLASSIC_FAILED_EVENT: &str = "classic-launch-failed";
/// Emitted when the launch is taking longer than the soft deadline. NOT
/// a failure — the poll keeps watching and may still emit
/// [`CLASSIC_LAUNCHED_EVENT`] afterwards.
pub const CLASSIC_SLOW_EVENT: &str = "classic-launch-slow";
/// Emitted when the portal needs an interactive sign-in (always the case
/// for TW, whose classic login is separate from the regular service).
pub const CLASSIC_NEEDS_LOGIN_EVENT: &str = "classic-needs-login";

/// `ui.window_create_failed` twin for the classic portal window.
const WINDOW_FAILED_CODE: &str = "classic.window_create_failed";

/// How long the poll keeps watching before giving up entirely. Well past
/// the observed 37 s worst case plus a slow-network margin.
#[cfg(target_os = "windows")]
const HARD_DEADLINE_TICKS: u32 = 360; // 360 × 500 ms = 180 s
/// When to reveal the window and report "taking longer" — while still
/// watching for a late launch.
#[cfg(target_os = "windows")]
const SOFT_DEADLINE_TICKS: u32 = 120; // 60 s
/// Injected into every navigation of the portal window. Drives the whole
/// flow and reports state back over `WebMessageReceived`.
///
/// # Why the action button is chosen by text, never by position
///
/// The consent / account-selection pages put several
/// `<a class="ui-btn">` in one fixed action bar and the **first** one is
/// typically 返回. Grabbing it with a bare `querySelector` navigated the
/// page back, the script re-injected on the reloaded page and clicked it
/// again — an invisible loop that showed up as a repeated
/// `account-auto-selected` message and a flow that never left the
/// selection step. The primary action is matched by its label (with a
/// last-button fallback, since action bars put the primary last), other
/// candidates are retried, and every attempt is reported so a stuck
/// flow says which button it pressed.
#[cfg(target_os = "windows")]
fn portal_script(region: LoginRegion) -> String {
    // HK signs in with the HK-beanfun button, TW with GamaPass.
    let selector = match region {
        LoginRegion::HK => ".btnLogin-beanfun",
        LoginRegion::TW => ".btnLogin-gamapass",
    };
    format!(
        r#"
(function () {{
  function post(msg) {{
    try {{ window.chrome.webview.postMessage(JSON.stringify(msg)); }} catch (e) {{}}
  }}
  var reported = '';
  function say(kind, extra) {{
    if (reported === kind) return;
    reported = kind;
    var m = extra || {{}};
    m.kind = kind;
    post(m);
  }}
  function textOf(el) {{
    return ((el.innerText || el.textContent || el.value || '') + '').trim();
  }}

  var PRIMARY = /繼續|继续|下一步|確定|确定|送出|同意|允許|允许|Continue|Next|Accept|Agree|Confirm|Submit/i;
  var BACK = /返回|上一步|取消|Back|Cancel|Previous/i;

  /** Candidate action buttons, primary-first and never a back button. */
  function actionCandidates() {{
    var area = document.querySelector('.bottom-fixed-action-area');
    var nodes = area ? area.querySelectorAll('a.ui-btn, button, input[type=submit]') : null;
    if (!nodes || nodes.length === 0) {{
      nodes = document.querySelectorAll('a.ui-btn, button, input[type=submit]');
    }}
    var all = [];
    for (var i = 0; i < nodes.length; i++) {{
      var el = nodes[i];
      if (el.disabled) continue;
      if (BACK.test(textOf(el))) continue;
      all.push(el);
    }}
    var primary = [], rest = [];
    for (var j = 0; j < all.length; j++) {{
      (PRIMARY.test(textOf(all[j])) ? primary : rest).push(all[j]);
    }}
    // Action bars put the primary action last, so try from the end.
    rest.reverse();
    return primary.concat(rest);
  }}

  // Per-page submit bookkeeping: retry a different candidate when the
  // page hasn't moved, and stop after a few so we never hammer a page.
  var stepHref = '', tries = 0, lastTry = -99, ticks = 0;
  function submitStep(step) {{
    if (stepHref !== location.href) {{ stepHref = location.href; tries = 0; lastTry = -99; }}
    if (tries >= 3) return false;
    // ~2s between attempts so a real navigation has time to happen.
    if (ticks - lastTry < 5) return false;
    var candidates = actionCandidates();
    if (candidates.length === 0) {{
      post({{ kind: step + '-no-button', href: location.href }});
      tries = 3;
      return false;
    }}
    var btn = candidates[Math.min(tries, candidates.length - 1)];
    post({{ kind: step + '-click', label: textOf(btn), attempt: tries + 1 }});
    btn.click();
    tries++;
    lastTry = ticks;
    return true;
  }}

  var clickedLogin = false;

  function tick() {{
    ticks++;
    var href = location.href;

    // (1) galaxy OTT init page — pick the region's sign-in route.
    if (href.indexOf('/login/init/mstc/') !== -1) {{
      if (!clickedLogin) {{
        var b = document.querySelector('{selector}');
        if (b) {{ b.click(); clickedLogin = true; say('signin-clicked'); }}
      }}
      return;
    }}

    // (2) A password field means an interactive sign-in is required —
    // URL alone lies, because a valid session redirects straight past
    // these hops.
    if (document.querySelector('input[type=password]')) {{
      say('need-login');
      return;
    }}

    // (3) GamaPass game-account selection (TW only). One account →
    // select + continue; several → hand the list to the host so the
    // user isn't forced to choose inside the web page.
    var radios = document.querySelectorAll('input[type=radio][name=account]');
    if (radios.length > 0) {{
      if (radios.length > 1) {{
        var list = [];
        for (var i = 0; i < radios.length; i++) {{
          var label = radios[i].closest('label') || radios[i].parentElement;
          list.push({{ value: radios[i].value, name: label ? textOf(label) : radios[i].value }});
        }}
        say('account-choice', {{ accounts: list }});
        return;
      }}
      // The page tracks selection through its own handler on the label
      // wrapper (`.ui-radio-button.selected`), so click that — poking
      // `input.checked` alone leaves their state untouched. Usually the
      // lone account is pre-selected and this is a no-op.
      if (!radios[0].checked) {{
        var wrap = radios[0].closest('label') || radios[0].parentElement;
        (wrap || radios[0]).click();
      }}
      submitStep('account');
      return;
    }}

    // (4) GamaPass authorization consent (TW only) — no password field,
    // no radios, still on the GamaPass host.
    if (href.indexOf('/GamaPassLogin/') !== -1) {{
      submitStep('consent');
      return;
    }}

    // (5) Portal — NGM missing shows the official install guide instead
    // of firing the launch.
    if (href.indexOf('maplestoryclassic.beanfun.com') !== -1) {{
      if (document.getElementById('ngmBtnStart') ||
          document.getElementById('ngmInstallLayerClose')) {{
        say('ngm-missing');
      }} else {{
        say('portal-reached');
      }}
    }}
  }}
  setInterval(tick, 400);
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
/// Deliberately no `ShellExecute` fallback: we are called from inside the
/// intercept that just cancelled WebView2's prompt, and handing the URL
/// back to the shell would only pop the same prompt again.
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

/// Executable NGM installs for the classic client.
#[cfg(target_os = "windows")]
const CLASSIC_CLIENT_EXE: &str = "Maplestory_Classic.exe";

/// Locate the installed classic client.
///
/// NGM records each title's install folder under
/// `HKLM\SOFTWARE\Nexon\<encoded title id>\RootPath`, where the subkey
/// name is an encoded id (e.g. `Mjk4Ml8yMTQxX2xpdmVfODM3` = base64 of
/// `2982_2141_live_837`). Guessing that id is fragile, so every subkey is
/// enumerated and the one whose `RootPath` actually contains the client
/// executable wins. NGM is 32-bit, so its keys land under
/// `WOW6432Node` — **both** registry views are read.
///
/// Falls back to scanning the usual Gamania install roots on every drive
/// letter, because users do install to D:.
#[cfg(target_os = "windows")]
fn detect_classic_client() -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
        let Ok(nexon) = hklm.open_subkey_with_flags(r"SOFTWARE\Nexon", KEY_READ | view) else {
            continue;
        };
        for name in nexon.enum_keys().flatten() {
            let Ok(sub) = nexon.open_subkey_with_flags(&name, KEY_READ | view) else {
                continue;
            };
            let Ok(root) = sub.get_value::<String, _>("RootPath") else {
                continue;
            };
            let exe = std::path::Path::new(&root).join(CLASSIC_CLIENT_EXE);
            if exe.is_file() {
                tracing::info!("classic: client found via registry ({})", exe.display());
                return Some(exe.to_string_lossy().into_owned());
            }
        }
    }

    // Registry miss (manual copy, broken install record) — probe the
    // conventional roots on every drive.
    const SUFFIXES: &[&str] = &[
        r"Program Files\Gamania\maplestory_classic",
        r"Program Files (x86)\Gamania\maplestory_classic",
        r"Gamania\maplestory_classic",
    ];
    for letter in b'C'..=b'Z' {
        for suffix in SUFFIXES {
            let exe = std::path::PathBuf::from(format!("{}:\\", letter as char))
                .join(suffix)
                .join(CLASSIC_CLIENT_EXE);
            if exe.is_file() {
                tracing::info!("classic: client found by scan ({})", exe.display());
                return Some(exe.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Result of the classic-readiness self-check.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClassicCheck {
    /// Nexon Game Manager's `ngm://` handler is registered (or a valid
    /// manual path is configured).
    pub ngm_registered: bool,
    /// The handler's executable path, if readable.
    pub ngm_exe: Option<String>,
    /// That executable actually exists on disk.
    pub ngm_exe_exists: bool,
    /// Path of the installed classic client, if found.
    pub client_path: Option<String>,
}

/// Check the local prerequisites for the classic launch.
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

        check.client_path = detect_classic_client();
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

/// Open the classic portal and launch the game.
///
/// `region` selects which sign-in route the portal takes and therefore
/// how much can happen silently:
///
/// - **HK** — a live beanfun session's cookies are seeded, the window
///   stays hidden and the whole SSO completes unattended.
/// - **TW** — classic is a separate login, so the window is shown from
///   the start and the user signs in inside it; the script then handles
///   the GamaPass consent + account-selection steps. No prior beanfun
///   session is required (and one would not help).
#[tauri::command]
#[specta::specta]
pub async fn open_classic_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    region: LoginRegion,
) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        open_classic_login_windows(app, state, region).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state, region);
        Err(CommandError::new(
            "classic.platform_unsupported",
            "MapleStory Classic launch is only supported on Windows",
        ))
    }
}

/// Construct the portal window. Kept **synchronous** on purpose: the
/// Tauri window builder is not `Send`, so it must never be alive across
/// an `.await` (the async caller only ever holds the built window).
#[cfg(target_os = "windows")]
fn spawn_portal_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    script: &str,
    visible: bool,
) -> Result<tauri::WebviewWindow<R>, CommandError> {
    let about_blank: tauri::Url = "about:blank".parse().expect("about:blank is a valid URL");
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        CLASSIC_WINDOW_LABEL,
        tauri::WebviewUrl::External(about_blank),
    )
    .title("新楓之谷：經典版")
    .inner_size(1024.0, 720.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(visible)
    .user_agent(CLASSIC_PORTAL_USER_AGENT)
    .initialization_script(script);
    // Share the per-instance WebView2 profile (issue #340) so this window
    // can't hit the cross-instance ERROR_INVALID_STATE either.
    if let Some(dir) = crate::current_instance_webview_dir() {
        builder = builder.data_directory(dir);
    }
    builder.build().map_err(|e| {
        CommandError::new(
            WINDOW_FAILED_CODE,
            format!("failed to create classic portal window: {e}"),
        )
    })
}

/// Build the portal window, waiting out any previous instance and
/// verifying the webview is actually alive.
///
/// The first COM handler registration doubles as the liveness probe: a
/// window built while the previous webview was still tearing down comes
/// back looking fine but with a dead webview (`ERROR_INVALID_STATE`), so
/// every later handler would silently time out. One rebuild is attempted
/// before giving up.
#[cfg(target_os = "windows")]
async fn build_portal_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    script: &str,
    visible: bool,
    on_message: impl Fn(&str) + Send + Sync + Clone + 'static,
) -> Result<tauri::WebviewWindow<R>, CommandError> {
    for attempt in 0..2 {
        // Wait for any previous portal window to actually disappear —
        // `destroy()` is asynchronous and building over a half-torn-down
        // webview is exactly what trips ERROR_INVALID_STATE.
        if let Some(existing) = app.get_webview_window(CLASSIC_WINDOW_LABEL) {
            let _ = existing.destroy();
        }
        for _ in 0..30 {
            if app.get_webview_window(CLASSIC_WINDOW_LABEL).is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        // Even once the window is gone the webview's COM teardown can
        // still be in flight; a short settle beats a failed rebuild.
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        let window = spawn_portal_window(app, script, visible)?;

        // Liveness probe (round-trips through COM).
        match super::cookie_native::register_web_message_handler(&window, on_message.clone()) {
            Ok(()) => return Ok(window),
            Err(e) if attempt == 0 => {
                tracing::warn!("classic: portal webview not usable ({e}); rebuilding once");
                let _ = window.destroy();
                tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            }
            Err(e) => {
                let _ = window.destroy();
                return Err(CommandError::new(
                    WINDOW_FAILED_CODE,
                    format!("classic portal webview failed to initialise: {e}"),
                ));
            }
        }
    }
    unreachable!("loop returns or errors on the second attempt")
}

#[cfg(target_os = "windows")]
async fn open_classic_login_windows<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    region: LoginRegion,
) -> Result<(), CommandError> {
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use std::sync::Arc;

    use tauri::Emitter;

    use super::cookie_native;

    // Launch state shared between the intercept callback and the poll.
    const PENDING: u8 = 0;
    const LAUNCHED: u8 = 1;
    const FAILED: u8 = 2;

    // A session is optional: HK uses it to skip the sign-in, TW cannot
    // use one at all (classic is a separate login there).
    let client = {
        let guard = state.auth.read().await;
        guard.as_ref().map(|ctx| ctx.client.clone())
    };
    let manual_ngm = manual_ngm_path(&state);
    // TW always needs an interactive sign-in — show the window from the
    // start rather than hiding it and waiting for a detection round-trip.
    let start_visible = region == LoginRegion::TW || client.is_none();

    let flag = Arc::new(AtomicU8::new(PENDING));
    let needs_login = Arc::new(AtomicBool::new(false));
    let ngm_missing = Arc::new(AtomicBool::new(false));
    let needs_pick = Arc::new(AtomicBool::new(false));

    let app_for_msg = app.clone();
    let needs_login_cb = needs_login.clone();
    let ngm_missing_cb = ngm_missing.clone();
    let needs_pick_cb = needs_pick.clone();
    let window = build_portal_window(&app, &portal_script(region), start_visible, move |raw| {
        tracing::info!("classic portal message: {raw}");
        // Tiny hand-rolled matching — the payload shapes are ours and a
        // serde round-trip here would buy nothing.
        if raw.contains("\"need-login\"") {
            if !needs_login_cb.swap(true, Ordering::SeqCst) {
                let _ = app_for_msg.emit(CLASSIC_NEEDS_LOGIN_EVENT, ());
            }
        } else if raw.contains("\"ngm-missing\"") {
            ngm_missing_cb.store(true, Ordering::SeqCst);
        } else if raw.contains("\"account-choice\"") {
            needs_pick_cb.store(true, Ordering::SeqCst);
        }
    })
    .await?;

    // Seed the session cookies (HK SSO) and keep popups in-window.
    cookie_native::register_new_window_handler(&window);
    if let Some(client) = client.as_ref() {
        let seeded = cookie_native::seed_cookies_native(&window, client);
        tracing::info!("classic: seeded {seeded} cookies into portal webview");
    }

    // Intercept the portal's own ngm:// launch: cancel WebView2's prompt
    // and start NGM ourselves.
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

    // Without interception the prompt can't be suppressed — the user has
    // to confirm it, so the window must be visible.
    if !intercept_ok {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    tauri::async_runtime::spawn(async move {
        tracing::info!("classic portal running (visible={start_visible}), waiting for launch");
        let mut revealed = start_visible;
        let reveal = |window: &tauri::WebviewWindow<R>, revealed: &mut bool| {
            if !*revealed {
                let _ = window.show();
                let _ = window.set_focus();
                *revealed = true;
            }
        };

        for tick in 0..HARD_DEADLINE_TICKS {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if app.get_webview_window(CLASSIC_WINDOW_LABEL).is_none() {
                return; // user closed the portal
            }

            match flag.load(Ordering::SeqCst) {
                LAUNCHED => {
                    let _ = window.app_handle().emit(CLASSIC_LAUNCHED_EVENT, ());
                    let _ = window.destroy();
                    return;
                }
                FAILED => {
                    let _ = window.app_handle().emit(CLASSIC_FAILED_EVENT, ());
                    reveal(&window, &mut revealed);
                    return;
                }
                _ => {}
            }

            // NGM missing is definitive — the portal is showing the
            // official install guide, so reveal it and stop waiting.
            if ngm_missing.load(Ordering::SeqCst) {
                tracing::warn!("classic: NGM install guide shown — not installed");
                let _ = window.app_handle().emit(CLASSIC_FAILED_EVENT, ());
                reveal(&window, &mut revealed);
                return;
            }

            // Interactive steps: the user has to act inside the portal,
            // so show it — but keep watching, the launch still comes.
            if needs_login.load(Ordering::SeqCst) || needs_pick.load(Ordering::SeqCst) {
                reveal(&window, &mut revealed);
            }

            // Soft deadline: reveal + say it's slow. Deliberately NOT a
            // failure — a launch measured at 37 s once landed 7 s after
            // an earlier build had already cried failure.
            if tick == SOFT_DEADLINE_TICKS {
                tracing::info!("classic: past the soft deadline, still watching");
                let _ = window.app_handle().emit(CLASSIC_SLOW_EVENT, ());
                reveal(&window, &mut revealed);
            }
        }

        tracing::warn!("classic: no launch within the hard deadline");
        let _ = window.app_handle().emit(CLASSIC_FAILED_EVENT, ());
        let _ = window.show();
        let _ = window.set_focus();
    });

    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

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
    fn portal_script_targets_the_region_button() {
        let hk = portal_script(LoginRegion::HK);
        assert!(hk.contains(".btnLogin-beanfun"));
        assert!(!hk.contains(".btnLogin-gamapass"));

        let tw = portal_script(LoginRegion::TW);
        assert!(tw.contains(".btnLogin-gamapass"));
        assert!(!tw.contains(".btnLogin-beanfun"));
    }

    #[test]
    fn portal_script_signals_over_web_message_not_the_title() {
        // Regression guard: `document.title` never reaches the native
        // window title, so the script must post messages instead.
        let script = portal_script(LoginRegion::TW);
        assert!(script.contains("chrome.webview.postMessage"));
        assert!(!script.contains("document.title"));
    }

    #[test]
    fn portal_script_never_picks_the_action_button_by_position() {
        // Regression: taking the FIRST `.ui-btn` in the action bar hit
        // 返回, which bounced the page back, re-injected the script and
        // clicked it again — a silent loop that stalled the flow at the
        // account step. The primary action must be matched by label and
        // back buttons must be filtered out.
        let script = portal_script(LoginRegion::TW);
        assert!(script.contains("PRIMARY"), "primary-label matcher");
        assert!(script.contains("BACK"), "back-label filter");
        assert!(script.contains("繼續"));
        assert!(script.contains("返回"));
        assert!(
            !script.contains(".bottom-fixed-action-area a.ui-btn')"),
            "must not grab the first action-bar button blindly"
        );
    }

    #[test]
    fn portal_script_retries_and_reports_each_submit_attempt() {
        let script = portal_script(LoginRegion::TW);
        // Bounded retries with a different candidate each time…
        assert!(script.contains("tries >= 3"));
        // …and every attempt names the button it pressed, so a stuck
        // flow is diagnosable from the log alone.
        assert!(script.contains("-click"));
        assert!(script.contains("label: textOf(btn)"));
        assert!(script.contains("-no-button"));
    }

    #[test]
    fn portal_script_selects_the_account_through_its_label_wrapper() {
        // The page tracks selection via its own handler on the label
        // wrapper; setting `input.checked` alone leaves it unselected.
        let script = portal_script(LoginRegion::TW);
        assert!(script.contains("closest('label')"));
    }

    #[test]
    fn portal_script_covers_every_flow_step() {
        let script = portal_script(LoginRegion::TW);
        // Password field (not the URL) is what marks "needs login".
        assert!(script.contains("input[type=password]"));
        assert!(script.contains("need-login"));
        // GamaPass consent + account selection.
        assert!(script.contains("/GamaPassLogin/"));
        assert!(script.contains("input[type=radio][name=account]"));
        assert!(script.contains("account-choice"));
        // NGM install guide detection.
        assert!(script.contains("ngmBtnStart"));
        assert!(script.contains("ngm-missing"));
    }

    #[test]
    fn soft_deadline_precedes_hard_deadline_with_room_to_spare() {
        // The measured worst case was 37 s (74 ticks); the soft deadline
        // must sit well past it and the hard one well past that. Read
        // through `black_box` so the compile-time values don't fold the
        // assertion into a constant.
        let soft = std::hint::black_box(SOFT_DEADLINE_TICKS);
        let hard = std::hint::black_box(HARD_DEADLINE_TICKS);
        assert!(soft > 74, "soft deadline must exceed the measured 37s");
        assert!(
            hard > soft * 2,
            "hard deadline must leave a late-launch margin"
        );
    }
}
