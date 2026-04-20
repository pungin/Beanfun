//! HTML-body branch classifiers for the HK-shaped login responses.
//!
//! Serves **both** [`login_hk_regular`](super::hk_regular::login_hk_regular)
//! and [`login_totp`](super::totp::login_totp) — WPF's `HkRegularLogin`
//! and `TotpLogin` classify failure bodies with the same two regexes
//! and the same `RELOAD_CAPTCHA_CODE + alert` advance-check marker,
//! so the rules live in one shared module to keep DRY.
//!
//! The module hosts three layers:
//! 1. Pure HTML parsers — [`extract_hk_error_signal`] and the
//!    `is_advance_check` probe (no [`LoginError`] dependency).
//! 2. The [`HkErrorSignal`] enum — a typed view of "what did the
//!    server's error body actually say".
//! 3. One thin classifier — `classify_missing_akey_body` — that
//!    maps [`HkErrorSignal`] onto a [`LoginError`] variant the
//!    orchestrators can bubble up with `?`. `is_advance_check` and
//!    `classify_missing_akey_body` are `pub(super)`; the plain
//!    backticks above avoid public-docs-link-to-private warnings.
//!
//! # WPF reference
//!
//! `BeanfunClient.Login.cs::HkRegularLogin` L247-285 and the
//! identical block in `TotpLogin` L359-388. When the response URL
//! does not carry `akey=…`, WPF scans the body for two script
//! patterns:
//!
//! 1. `<script type="text/javascript">$(function(){MsgBox.Show('…');});</script>`
//!    — a pop-up error message. The inner text (group 1) becomes
//!    `this.errmsg` verbatim.
//! 2. `pollRequest("…","(\w+)","…");` — a triplet of (url, token,
//!    param) the page uses to register a mobile-app auth flow. WPF
//!    builds a display string of `group1 + '","' + group3` for
//!    `errmsg` and **stashes `group2` on `LoginToken`** for the timer
//!    thread to poll against `CheckIsRegisteDevice`.
//!
//! Both patterns are Beanfun-specific (the "MsgBox.Show" helper and
//! the `pollRequest` JS shim are bespoke to the Beanfun login pages),
//! so this parser lives under `services/beanfun/login` rather than
//! the generic `core/parser` tree.
//!
//! # Why we keep `token` in the return type
//!
//! Chunk 3.3.4 (`CheckIsRegisteDevice`) wires `token` through as the
//! `LoginToken` sent with the `LT=` form field on every
//! `bfAPPAutoLogin.ashx` poll. Keeping `token` on
//! [`HkErrorSignal::PollRequest`] means the classifier has one
//! canonical shape regardless of which flow consumed it.

use regex::Regex;
use std::sync::OnceLock;

use crate::services::beanfun::LoginError;

/// One of three possible outcomes when we scan an HK / TOTP response
/// body that failed to carry an `akey=…` redirect.
///
/// The [`HkErrorSignal::PollRequest::token`] field preserves
/// `this.LoginToken` (WPF L281) for the future
/// `CheckIsRegisteDevice` wiring. All string values are the raw
/// regex capture groups — no HTML-unescape is applied, matching WPF
/// which also feeds the raw text into `errmsg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HkErrorSignal {
    /// Matched `MsgBox.Show('…');`. The captured string is the
    /// human-readable error text meant for direct display.
    MsgBox(String),
    /// Matched `pollRequest("…","(\w+)","…");`. The shared
    /// `classify_missing_akey_body` wraps this into
    /// [`LoginError::DeviceRegistrationRequired`] so callers can:
    /// (a) feed `token` into
    /// [`login_registered_device`](super::registered_device::login_registered_device)
    /// as the `LoginToken`, (b) log `url` + `param` for diagnostics —
    /// WPF's display string concatenates them into `errmsg` via
    /// `url + '","' + param` (L277-280 / L383-385).
    PollRequest {
        /// First group — a URL the page intends to poll. In practice
        /// almost always an opaque ashx / handler endpoint.
        url: String,
        /// Second group — the `LoginToken` the page will forward on
        /// subsequent polls. `\w+` in the regex, so always a safe
        /// alphanumeric id.
        token: String,
        /// Third group — an opaque parameter carried alongside the
        /// token. WPF puts this into the visible error string.
        param: String,
    },
    /// Neither pattern matched. Caller should surface
    /// `LoginError::MissingAkey` as a last resort (WPF L264 sets
    /// `errmsg = "LoginNoAkey"` at the start of the same branch).
    Unrecognized,
}

/// Classify an HK login response body that failed to redirect to an
/// `akey=…` URL.
///
/// Tries the `MsgBox` regex first (WPF L266-272), falling back to the
/// `pollRequest` regex (L274-282). Returns [`HkErrorSignal::Unrecognized`]
/// when neither matches — same precedence and same regexes as WPF.
pub fn extract_hk_error_signal(html: &str) -> HkErrorSignal {
    if let Some(msg) = capture_msgbox(html) {
        return HkErrorSignal::MsgBox(msg);
    }
    if let Some((url, token, param)) = capture_poll_request(html) {
        return HkErrorSignal::PollRequest { url, token, param };
    }
    HkErrorSignal::Unrecognized
}

/// WPF `HkRegularLogin` L247-251 / `TotpLogin` L359-363 — the
/// advance-check signal is `RELOAD_CAPTCHA_CODE` appearing together
/// with an `alert` call. Either alone is not enough (the login page's
/// help link also mentions the captcha reload), so we require both
/// substrings.
///
/// Pure predicate — no [`LoginError`] dependency — so callers can use
/// it inside their own match arms freely.
pub(super) fn is_advance_check(body: &str) -> bool {
    body.contains("RELOAD_CAPTCHA_CODE") && body.contains("alert")
}

/// WPF `HkRegularLogin` L264-284 / `TotpLogin` L368-388 — classify
/// the error body when the final URL carries no `akey`. Delegates to
/// the pure [`extract_hk_error_signal`] parser; this wrapper is the
/// translation layer from the regex outcome to the typed
/// [`LoginError`] variant the orchestrators surface.
///
/// Shared by `login_hk_regular` and `login_totp` because WPF emits
/// the same failure-body shape in both flows (`TotpLogin` literally
/// pastes the `HkRegularLogin` classification block). Keeping them
/// on one function means any future tweak takes one edit instead of
/// two.
///
/// # `pollRequest` continuation contract
///
/// The `pollRequest` branch surfaces
/// [`LoginError::DeviceRegistrationRequired`] rather than a flat
/// `ServerMessage`, preserving all three regex capture groups:
///
/// - `login_token` (WPF L281 / L385 → `this.LoginToken`) — the
///   identifier the caller sends as the `LT=` form field to
///   `bfAPPAutoLogin.ashx` when driving `login_registered_device`.
/// - `poll_url` (group 1) and `param` (group 3) — WPF formats them
///   into a display-only `errmsg` via `url + '","' + param`
///   (L277-280 / L383-385). We preserve them as separate strings so
///   the caller can choose whether to show the same WPF-style
///   concat string or present them independently.
pub(super) fn classify_missing_akey_body(body: &str) -> LoginError {
    match extract_hk_error_signal(body) {
        HkErrorSignal::MsgBox(msg) => LoginError::ServerMessage(msg),
        HkErrorSignal::PollRequest { url, token, param } => {
            LoginError::DeviceRegistrationRequired {
                login_token: token,
                poll_url: url,
                param,
            }
        }
        // WPF L264 / L368 pre-sets `errmsg = "LoginNoAkey"` before
        // the script-scan; if neither regex matches, that default
        // wins.
        HkErrorSignal::Unrecognized => LoginError::MissingAkey,
    }
}

// -----------------------------------------------------------------------------
// Regex helpers
// -----------------------------------------------------------------------------

/// Match the `MsgBox.Show('…')` pop-up script. Mirrors WPF's regex
/// exactly — `\$\(function\(\){…}\);` wrapper and all.
fn capture_msgbox(html: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // The inner `(.*)` is **greedy**, matching WPF. That's OK in
        // practice because the script tag sits on its own line and
        // the real server never nests a second MsgBox on the same
        // line. Non-greedy would be safer but would diverge from WPF
        // without an observable benefit; we stay with the WPF shape.
        Regex::new(
            r#"<script type="text/javascript">\$\(function\(\)\{MsgBox\.Show\('(.*)'\);\}\);</script>"#,
        )
        .expect("MsgBox regex must compile")
    });
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

/// Match the `pollRequest("url","token","param");` call with three
/// explicit capture groups, matching WPF's three-group regex (L274).
///
/// Regex details:
/// - Group 1 `[^"]*` — allows empty strings (WPF L274 `([^"]*)`).
/// - Group 2 `\w+` — non-empty alphanumeric + underscore, matching
///   the ASP.NET token alphabet.
/// - Group 3 `[^"]+` — non-empty; empty would be meaningless.
fn capture_poll_request(html: &str) -> Option<(String, String, String)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"pollRequest\("([^"]*)","(\w+)","([^"]+)"\);"#)
            .expect("pollRequest regex must compile")
    });
    let caps = re.captures(html)?;
    Some((
        caps.get(1)?.as_str().to_owned(),
        caps.get(2)?.as_str().to_owned(),
        caps.get(3)?.as_str().to_owned(),
    ))
}

// -----------------------------------------------------------------------------
// Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // MsgBox cases
    // -------------------------------------------------------------------------

    #[test]
    fn msgbox_plain_ascii_message() {
        let html = r#"<script type="text/javascript">$(function(){MsgBox.Show('Invalid credentials');});</script>"#;
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::MsgBox("Invalid credentials".into())
        );
    }

    #[test]
    fn msgbox_traditional_chinese_message() {
        // The real server returns Traditional Chinese error messages;
        // the regex operates on the UTF-8 byte stream so Chinese
        // characters pass through unchanged.
        let html = r#"<script type="text/javascript">$(function(){MsgBox.Show('帳號或密碼錯誤');});</script>"#;
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::MsgBox("帳號或密碼錯誤".into())
        );
    }

    #[test]
    fn msgbox_empty_message_still_matches() {
        // `(.*)` with nothing inside is a valid zero-length capture.
        // We preserve WPF's greedy behaviour rather than require a
        // non-empty body.
        let html = r#"<script type="text/javascript">$(function(){MsgBox.Show('');});</script>"#;
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::MsgBox(String::new())
        );
    }

    #[test]
    fn msgbox_wins_over_poll_request_when_both_present() {
        // WPF checks MsgBox first (L266) and only falls through to
        // pollRequest if MsgBox misses (L272 `else`). We mirror that.
        let html = concat!(
            r#"<script type="text/javascript">$(function(){MsgBox.Show('first');});</script>"#,
            r#"pollRequest("url","TOKEN","param");"#,
        );
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::MsgBox("first".into())
        );
    }

    // -------------------------------------------------------------------------
    // pollRequest cases
    // -------------------------------------------------------------------------

    #[test]
    fn poll_request_captures_three_groups() {
        let html = r#"<div>pollRequest("/foo/bar.ashx","TOKEN_123","extra_param");</div>"#;
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::PollRequest {
                url: "/foo/bar.ashx".into(),
                token: "TOKEN_123".into(),
                param: "extra_param".into(),
            }
        );
    }

    #[test]
    fn poll_request_allows_empty_first_group() {
        // WPF `[^"]*` accepts empty URL strings. We lock that in as
        // a regression guard.
        let html = r#"pollRequest("","TOKEN","param");"#;
        assert_eq!(
            extract_hk_error_signal(html),
            HkErrorSignal::PollRequest {
                url: String::new(),
                token: "TOKEN".into(),
                param: "param".into(),
            }
        );
    }

    #[test]
    fn poll_request_rejects_empty_token_group() {
        // Group 2 is `\w+` (one-or-more), so an empty token fails.
        // WPF uses the exact same quantifier — no match → fall
        // through to Unrecognized.
        let html = r#"pollRequest("/url","","param");"#;
        assert_eq!(extract_hk_error_signal(html), HkErrorSignal::Unrecognized);
    }

    #[test]
    fn poll_request_rejects_empty_param_group() {
        // Group 3 is `[^"]+` — empty param fails to match, same as
        // WPF.
        let html = r#"pollRequest("/url","TOKEN","");"#;
        assert_eq!(extract_hk_error_signal(html), HkErrorSignal::Unrecognized);
    }

    // -------------------------------------------------------------------------
    // Unrecognized cases
    // -------------------------------------------------------------------------

    #[test]
    fn unrecognized_when_no_script_pattern_matches() {
        let html = "<html><body>completely unrelated error page</body></html>";
        assert_eq!(extract_hk_error_signal(html), HkErrorSignal::Unrecognized);
    }

    #[test]
    fn unrecognized_for_partial_msgbox_match() {
        // A MsgBox.Show call OUTSIDE the specific script wrapper that
        // WPF regex expects does not match — preserving WPF's exact
        // shape requirement (L266).
        let html = r#"<div>MsgBox.Show('inline call');</div>"#;
        assert_eq!(extract_hk_error_signal(html), HkErrorSignal::Unrecognized);
    }

    // -------------------------------------------------------------------------
    // is_advance_check
    // -------------------------------------------------------------------------

    #[test]
    fn advance_check_requires_both_tokens() {
        // Both markers present → match
        assert!(is_advance_check(
            "<script>if(window.RELOAD_CAPTCHA_CODE){alert('x');}</script>"
        ));
        // Only one of the two → no match (WPF L247 requires both).
        // The negative strings are deliberately free of the other
        // substring so the assertion actually proves the AND.
        assert!(!is_advance_check(
            "RELOAD_CAPTCHA_CODE marker but no popup trigger"
        ));
        assert!(!is_advance_check("alert('popup, no reload marker')"));
    }

    // -------------------------------------------------------------------------
    // classify_missing_akey_body
    // -------------------------------------------------------------------------

    #[test]
    fn classify_msgbox_becomes_server_message() {
        let body = r#"<script type="text/javascript">$(function(){MsgBox.Show('帳號或密碼錯誤');});</script>"#;
        match classify_missing_akey_body(body) {
            LoginError::ServerMessage(msg) => assert_eq!(msg, "帳號或密碼錯誤"),
            other => panic!("expected ServerMessage, got {other:?}"),
        }
    }

    #[test]
    fn classify_poll_request_surfaces_device_registration_required() {
        // Chunk 3.3.4 refactor: pollRequest now routes to
        // `DeviceRegistrationRequired` with all three regex groups
        // preserved, superseding the earlier `ServerMessage(concat)`
        // shape. Callers that want WPF's display string can still
        // format it as `format!("{poll_url}\",\"{param}")`.
        let body = r#"pollRequest("/poll/url","TOK","extra_param");"#;
        match classify_missing_akey_body(body) {
            LoginError::DeviceRegistrationRequired {
                login_token,
                poll_url,
                param,
            } => {
                assert_eq!(login_token, "TOK");
                assert_eq!(poll_url, "/poll/url");
                assert_eq!(param, "extra_param");
            }
            other => panic!("expected DeviceRegistrationRequired, got {other:?}"),
        }
    }

    #[test]
    fn classify_unrecognized_surfaces_missing_akey() {
        let body = "<html>nothing relevant here</html>";
        match classify_missing_akey_body(body) {
            LoginError::MissingAkey => {}
            other => panic!("expected MissingAkey, got {other:?}"),
        }
    }
}
