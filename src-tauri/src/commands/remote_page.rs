//! Undoing what Tauri's plugins do to beanfun's own pages.
//!
//! Tauri plugins inject their init scripts into **every** webview, and
//! several of them replace standard web APIs with IPC calls. Our
//! capability grants those commands to the `main` window only — on
//! purpose, since every other window renders beanfun's pages and a
//! remote page must not be able to reach our commands. The result is
//! that the replaced API is dead in exactly the windows where the page
//! is not ours to fix: the control is cancelled, the IPC is denied, and
//! nothing happens.
//!
//! Three shims live here, covering `target="_blank"` links, `alert()`,
//! and popups that never come back. All are pure page-side JavaScript
//! with no IPC and no capability, so they close the gap without opening
//! one. A test asserts each is actually injected into each remote
//! window — a shim that ships wired to nothing fixes nothing, and that
//! has already happened once here.
//!
//! # The dead-button bug
//!
//! `tauri-plugin-opener` injects this into **every** webview:
//!
//! ```js
//! window.addEventListener("click", (e) => {
//!   /* … */ if (anchor.target === "_blank" || e.ctrlKey || e.shiftKey) {
//!     e.preventDefault();
//!     invoke("plugin:opener|open_url", { url });
//!   }
//! })
//! ```
//!
//! Our capability grants `opener:default` to the `main` window only —
//! deliberately, because the other windows render **beanfun's pages**,
//! and a remote page must not be able to reach our commands. So in those
//! windows the plugin cancels the navigation and then its `open_url`
//! call is denied:
//!
//! ```text
//! opener.open_url not allowed on window "web-browser-…"
//!   allowed on: [windows: "main", URL: local]
//! ```
//!
//! The click is consumed and nothing happens. Every `target="_blank"`
//! control in the in-app browser (the account-management pages are full
//! of them) is simply dead.
//!
//! Widening the capability would fix the symptom by handing remote pages
//! an IPC surface — the wrong trade, and it would still send the user
//! out to their system browser for a link that belongs inside the
//! window they are already in.
//!
//! # The fix
//!
//! A **capture-phase** listener on `window`. Capture always runs before
//! the plugin's bubble-phase listener regardless of registration order,
//! and the plugin's first guard is `if (e.defaultPrevented) return` — so
//! claiming the click with `preventDefault()` is enough to take it, with
//! no need to stop propagation and break the page's own handlers.
//!
//! Then we navigate in the same window, which is what
//! `NewWindowRequested` already does for script-driven `window.open`
//! (see [`super::cookie_native::register_new_window_handler`]) and what
//! WPF's `CoreWebView2_NewWindowRequested` did before it.

/// Injected into every window that renders a remote page.
///
/// Deliberately mirrors the plugin's own matching rules — left button,
/// no meta/alt, nearest `<a>` on the composed path, `_blank` or a
/// ctrl/shift click, http(s) only — so the set of clicks we claim is
/// exactly the set it would have swallowed. Anything else is left
/// completely alone.
pub const KEEP_LINKS_IN_WINDOW: &str = r#"
(function () {
  window.addEventListener(
    'click',
    function (event) {
      if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.altKey) return;
      var anchor = event.composedPath().find(function (el) {
        return el && el.nodeName && String(el.nodeName).toUpperCase() === 'A';
      });
      if (!anchor || !anchor.href) return;
      if (anchor.target !== '_blank' && !event.ctrlKey && !event.shiftKey) return;
      var url;
      try {
        url = new URL(anchor.href, window.location.href);
      } catch (_) {
        return;
      }
      if (url.protocol !== 'http:' && url.protocol !== 'https:') return;
      // Claiming the click: the opener plugin's bubble-phase listener
      // starts with `if (e.defaultPrevented) return`, so this is all it
      // takes. Propagation is left intact so the page's own handlers
      // still run.
      event.preventDefault();
      window.location.assign(url.href);
    },
    true,
  );
})();
"#;

/// Restores a visible `alert()` in windows that render beanfun's pages.
///
/// # Why it is broken there
///
/// `tauri-plugin-dialog` replaces `window.alert` with
/// `invoke("plugin:dialog|message")`. In these windows that command is
/// denied, so the call resolves to a rejected promise nobody awaits and
/// **the message is simply never shown** — the page carries on and the
/// user sees a button that did nothing:
///
/// ```text
/// dialog.message not allowed on window "web-browser-…"
///   allowed on: [windows: "main", URL: local]
/// ```
///
/// # Why the native one cannot be put back
///
/// Both escape routes were measured and neither works:
///
/// - **Capturing the native first** — plugin init scripts run *before*
///   the window builder's, so by the time this runs `window.alert` is
///   already the plugin's.
/// - **Borrowing a child frame's copy, or `delete`ing the override** —
///   the injection reaches child frames too, and `alert` is an own
///   property of the global rather than something on `Window.prototype`,
///   so deleting it leaves `undefined`.
///
/// So the message is rendered in the page instead. It does not block the
/// way a native dialog would, but the plugin's replacement did not block
/// either, so no behaviour regresses — it just becomes visible again.
///
/// `window.confirm` is replaced by the same plugin with an **async**
/// function, which means `if (confirm(…))` sees a promise and is always
/// true. That is a pre-existing bug in every window including `main`,
/// and it cannot be fixed from JavaScript: a synchronous, blocking
/// `confirm` is not expressible. It is deliberately left alone here
/// rather than papered over with something that returns a guess.
pub const RESTORE_ALERT: &str = r#"
(function () {
  var OVERLAY_STYLE =
    'position:fixed;inset:0;z-index:2147483647;background:rgba(0,0,0,.45);' +
    'display:flex;align-items:center;justify-content:center;';
  var PANEL_STYLE =
    'max-width:min(30rem,80vw);max-height:70vh;overflow:auto;box-sizing:border-box;' +
    'background:#fff;color:#111;border-radius:.5rem;padding:1.25rem;' +
    'box-shadow:0 .5rem 2rem rgba(0,0,0,.35);' +
    "font:14px/1.6 system-ui,-apple-system,'Segoe UI',sans-serif;";
  var TEXT_STYLE = 'margin:0 0 1rem;white-space:pre-wrap;word-break:break-word;';
  var BUTTON_STYLE =
    'display:block;margin-left:auto;min-width:5rem;padding:.4rem 1rem;cursor:pointer;' +
    'border:0;border-radius:.25rem;background:#1565c0;color:#fff;font:inherit;';

  function show(text) {
    var root = document.body || document.documentElement;
    if (!root) return;

    var overlay = document.createElement('div');
    overlay.setAttribute('role', 'alertdialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.style.cssText = OVERLAY_STYLE;

    var panel = document.createElement('div');
    panel.style.cssText = PANEL_STYLE;

    var body = document.createElement('p');
    body.style.cssText = TEXT_STYLE;
    // Assigned as text, never parsed as markup: the string comes from
    // the page and must not be able to inject through this shim.
    body.textContent = text;

    var ok = document.createElement('button');
    ok.type = 'button';
    ok.style.cssText = BUTTON_STYLE;
    ok.textContent = 'OK';

    function close() {
      document.removeEventListener('keydown', onKey, true);
      if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
    }
    function onKey(event) {
      if (event.key === 'Escape' || event.key === 'Enter') {
        event.preventDefault();
        close();
      }
    }

    ok.addEventListener('click', close);
    document.addEventListener('keydown', onKey, true);

    panel.appendChild(body);
    panel.appendChild(ok);
    overlay.appendChild(panel);
    root.appendChild(overlay);
    try {
      ok.focus();
    } catch (_) {}
  }

  window.alert = function (message) {
    var text = message === undefined ? '' : String(message);
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function () {
        show(text);
      });
      return;
    }
    show(text);
  };
})();
"#;

/// Stops pages falling back to a blocked `<iframe>` when a popup does
/// not come back.
///
/// # The symptom
///
/// ```text
/// Framing 'https://accounts.gamania.com/' violates the following
/// Content Security Policy directive: "frame-ancestors 'none'"
/// ```
///
/// Gamania's account host refuses framing outright — measured on `/`,
/// `/login` and `/signin?client_id=…`, all three answer
/// `frame-ancestors 'none'` plus `X-Frame-Options: SAMEORIGIN`, and the
/// headers are byte-identical under a WebView2 and a plain Chrome
/// user-agent. Nothing on our side can permit that framing, and the same
/// page works in Chrome — so the page is not *supposed* to be framing
/// it. It is falling back to an iframe because its popup never arrived.
///
/// # Why the popup never arrives
///
/// `window.open` returns `null` in a wry-hosted WebView2. Measured in
/// the main window, which registers **no** `NewWindowRequested` handler
/// at all: the return value is still `null`. So this is not our
/// popup-to-same-window policy — WebView2 simply does not create a
/// popup, or hand back a `WindowProxy`, unless the host supplies a
/// webview for it through `put_NewWindow`.
///
/// The page then reads that `null` as "popup blocked" and takes its
/// fallback path, which is the framing attempt above.
///
/// # What this does
///
/// Calls through to the real `window.open` — so
/// [`super::cookie_native::register_new_window_handler`] still routes
/// the navigation — and then, only when that returns nothing, hands back
/// a small stand-in carrying the members popup code actually touches
/// (`closed`, `close`, `focus`, `blur`, `postMessage`, `opener`,
/// `location.href`). The page stops believing it was blocked, so it
/// never reaches for the iframe.
///
/// This does **not** manufacture a real popup, and there is no opener
/// relationship: a flow that needs the popup to `postMessage` back to
/// its parent still cannot complete. Giving pages a genuine second
/// window means implementing `put_NewWindow` with a deferral and a
/// second webview built from inside the event handler — a much larger
/// and deadlock-prone change (see the wry#583 note on
/// `commands::auth::open_gamepass_window`), and not one to make
/// speculatively.
pub const KEEP_POPUPS_ALIVE: &str = r#"
(function () {
  var nativeOpen = window.open;
  window.open = function (url, target, features) {
    var opened = null;
    try {
      opened = nativeOpen.call(window, url, target, features);
    } catch (_) {}
    if (opened) return opened;
    var href = url === undefined || url === null ? '' : String(url);
    return {
      closed: false,
      opener: window,
      name: target === undefined ? '' : String(target),
      location: { href: href, assign: function () {}, replace: function () {} },
      close: function () {
        this.closed = true;
      },
      focus: function () {},
      blur: function () {},
      postMessage: function () {},
    };
  };
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listener_is_registered_in_the_capture_phase() {
        // Bubble phase would run *after* the opener plugin's listener,
        // by which point the click is already consumed.
        // Compare whitespace-free so reformatting the script cannot
        // silently turn this assertion into a no-op.
        let dense: String = KEEP_LINKS_IN_WINDOW
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            dense.contains("true,)") || dense.contains("true)"),
            "the third addEventListener argument must be `true`"
        );
    }

    #[test]
    fn it_claims_the_click_without_stopping_propagation() {
        // preventDefault is what makes the plugin bail; stopping
        // propagation would also silence the page's own handlers.
        assert!(KEEP_LINKS_IN_WINDOW.contains("event.preventDefault()"));
        assert!(!KEEP_LINKS_IN_WINDOW.contains("stopPropagation"));
        assert!(!KEEP_LINKS_IN_WINDOW.contains("stopImmediatePropagation"));
    }

    #[test]
    fn it_matches_the_same_clicks_the_plugin_would_have_taken() {
        for guard in [
            "event.button !== 0",
            "event.metaKey",
            "event.altKey",
            "anchor.target !== '_blank'",
            "event.ctrlKey",
            "event.shiftKey",
            "'http:'",
            "'https:'",
        ] {
            assert!(
                KEEP_LINKS_IN_WINDOW.contains(guard),
                "missing guard: {guard}"
            );
        }
    }

    #[test]
    fn it_navigates_in_the_same_window() {
        assert!(KEEP_LINKS_IN_WINDOW.contains("window.location.assign"));
        // A new native window would just reintroduce the popup we are
        // deliberately collapsing into this one.
        assert!(!KEEP_LINKS_IN_WINDOW.contains("window.open"));
    }

    #[test]
    fn the_alert_shim_needs_no_ipc_and_no_capability() {
        // The whole point: it must work in a window that is denied every
        // command, so it cannot reach for one.
        for forbidden in ["invoke", "__TAURI", "plugin:", "ipc.localhost"] {
            assert!(
                !RESTORE_ALERT.contains(forbidden),
                "the shim must not depend on {forbidden}"
            );
        }
    }

    #[test]
    fn the_alert_shim_renders_the_message_as_text() {
        // The string comes from the page; rendering it as markup would
        // turn a compatibility shim into an injection point.
        assert!(RESTORE_ALERT.contains("body.textContent = text"));
        assert!(!RESTORE_ALERT.contains("innerHTML"));
    }

    #[test]
    fn the_alert_shim_replaces_only_alert() {
        assert!(RESTORE_ALERT.contains("window.alert = function"));
        // `confirm` cannot be shimmed correctly — a synchronous blocking
        // dialog is not expressible in JS — so it is left untouched
        // rather than replaced by something that guesses an answer.
        assert!(!RESTORE_ALERT.contains("window.confirm ="));
        assert!(!RESTORE_ALERT.contains("window.prompt ="));
    }

    #[test]
    fn the_popup_shim_still_calls_through_to_the_real_open() {
        // The stand-in must not replace the navigation, only the return
        // value — otherwise the link goes nowhere at all.
        assert!(KEEP_POPUPS_ALIVE.contains("nativeOpen.call(window, url, target, features)"));
        assert!(KEEP_POPUPS_ALIVE.contains("if (opened) return opened;"));
    }

    #[test]
    fn the_popup_stand_in_carries_what_popup_code_touches() {
        for member in [
            "closed:",
            "opener:",
            "close:",
            "focus:",
            "blur:",
            "postMessage:",
            "location:",
        ] {
            assert!(
                KEEP_POPUPS_ALIVE.contains(member),
                "the stand-in is missing {member}"
            );
        }
    }

    /// Every shim here must actually be injected into every window that
    /// renders a remote page.
    ///
    /// This exists because [`RESTORE_ALERT`] once shipped with its
    /// content fully tested and **wired into nothing** — the edit that
    /// was supposed to add the `initialization_script` calls silently
    /// never ran. Tests that only assert on the constant cannot see
    /// that: the script was perfect and dead. This one reads the call
    /// sites instead.
    #[test]
    fn every_shim_is_injected_into_every_remote_window() {
        use std::path::Path;

        const SHIMS: &[&str] = &["KEEP_LINKS_IN_WINDOW", "RESTORE_ALERT", "KEEP_POPUPS_ALIVE"];
        // The windows that render pages we do not control.
        const WINDOWS: &[&str] = &[
            "web_browser.rs", // in-app browser
            "auth.rs",        // GamePass sign-in
            "classic.rs",     // MapleStory Classic portal
        ];

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
        let mut missing = Vec::new();
        for window in WINDOWS {
            let source = std::fs::read_to_string(dir.join(window)).expect("read call site");
            for shim in SHIMS {
                let injected =
                    format!(".initialization_script(crate::commands::remote_page::{shim}")
                        .replace(' ', "");
                let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();
                if !dense.contains(&injected) {
                    missing.push(format!("{window} never injects {shim}"));
                }
            }
        }

        assert!(
            missing.is_empty(),
            "a shim that is not injected fixes nothing:\n  {}",
            missing.join("\n  ")
        );
    }

    #[test]
    fn the_popup_shim_needs_no_ipc_and_no_capability() {
        for forbidden in ["invoke", "__TAURI", "plugin:", "ipc.localhost"] {
            assert!(
                !KEEP_POPUPS_ALIVE.contains(forbidden),
                "the shim must not depend on {forbidden}"
            );
        }
    }

    #[test]
    fn the_alert_shim_survives_being_called_before_the_body_exists() {
        // Init scripts run at document-start, so `document.body` can be
        // null when a page alerts from an inline script in <head>.
        assert!(RESTORE_ALERT.contains("document.readyState === 'loading'"));
        assert!(RESTORE_ALERT.contains("DOMContentLoaded"));
    }
}
