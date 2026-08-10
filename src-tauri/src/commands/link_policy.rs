//! Keeping `target="_blank"` links alive in our remote-content windows.
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
}
