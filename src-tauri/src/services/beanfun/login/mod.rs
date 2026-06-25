//! Login flows for the Beanfun service.
//!
//! Every submodule here is an `async` function (or small family of
//! functions) that drives one discrete HTTP round-trip of the WPF login
//! sequence. The flows are composed by higher-level orchestrators (added
//! in later chunks) that call each step in order, handling branching
//! (TOTP required / advance-check required / QR-code polling) as typed
//! [`super::LoginError`] variants.
//!
//! # Why split per step?
//!
//! The WPF source inlines the entire flow inside one giant `try` block per
//! method (`TwRegularLogin`, `HkRegularLogin`, `QRCodeLogin`). That makes
//! unit-testing any individual step difficult without mocking the whole
//! sequence. Splitting each HTTP call into its own function lets us:
//!
//! - Test each step against a tiny wiremock expectation.
//! - Reuse shared steps (e.g. `get_session_key`) across all three login
//!   methods without duplicating code.
//! - Surface per-step errors via the typed [`super::LoginError`] enum.

pub mod account_login;
pub mod check_account_type;
pub mod completed;
pub mod gamepass;
pub mod hk_error;
pub mod hk_regular;
pub mod index;
pub mod init_login;
pub mod logout;
pub mod orchestrator;
pub mod qr_finalize;
pub mod qr_init;
pub mod qr_poll;
pub mod registered_device;
pub mod return_aspx;
pub mod send_login;
pub mod session_key;
pub mod totp;
pub mod totp_challenge;
pub mod tw_regular;

pub use account_login::account_login;
pub use check_account_type::check_account_type;
pub use completed::login_completed;
pub use gamepass::{
    inject_webview_cookies, seed_webview_cookies_from_client, try_complete_gamepass_login,
};
pub use hk_error::{extract_hk_error_signal, HkErrorSignal};
pub use hk_regular::login_hk_regular;
pub use index::{get_login_index, LoginIndex};
pub use init_login::check_recaptcha_required;
pub use logout::logout;
pub use orchestrator::{login_with, LoginMethod};
pub use qr_finalize::finalize_qr_login;
pub use qr_init::{init_qr_login, normalize_beanfun_app_deeplink, QrLoginInit};
pub use qr_poll::{poll_qr_login_status, QrPollOutcome};
pub use registered_device::login_registered_device;
pub use return_aspx::post_return_aspx;
pub use send_login::send_login;
pub use session_key::get_session_key;
pub use totp::login_totp;
pub use totp_challenge::TotpChallenge;
pub use tw_regular::login_tw_regular;

use std::fmt;

use serde::de::{self, Deserializer, Visitor};

use crate::services::beanfun::{BeanfunClient, LoginError};

// -----------------------------------------------------------------------------
// Shared request helpers
// -----------------------------------------------------------------------------

/// Fail with [`LoginError::Unknown`] when `resp` is not a 2xx. Keeps
/// every login step's "non-success shortcut" to one line so the error
/// text stays consistent across the flow.
///
/// Does **not** handle 3xx as success — the one step that needs that
/// (`return.aspx`) inspects the status itself.
pub(crate) fn ensure_success(resp: &reqwest::Response, step: &str) -> Result<(), LoginError> {
    if !resp.status().is_success() {
        return Err(LoginError::Unknown(format!(
            "{step} returned HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Apply the exact header set that WPF's `SetJsonHeaders` installs before
/// any JSON-bodied login call (CheckAccountType, AccountLogin).
///
/// Factored out here because both call sites send the **same** four
/// headers; keeping a single helper means a future protocol tweak only
/// needs to be applied in one place.
///
/// Note: `Content-Type: application/json; charset=utf-8` is **not** set
/// explicitly — reqwest's `.json(&body)` adds it automatically, which
/// byte-matches what WPF's `Headers[HttpRequestHeader.ContentType]`
/// assignment emits.
pub(crate) fn apply_json_headers(
    rb: reqwest::RequestBuilder,
    verification_token: &str,
    referer: &str,
) -> reqwest::RequestBuilder {
    rb.header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
        .header(reqwest::header::REFERER, referer)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("RequestVerificationToken", verification_token)
}

/// Read `bfWebToken` from `client`'s shared cookie jar, scoped to the
/// portal host (TW: `tw.beanfun.com`; HK: `bfweb.hk.beanfun.com`).
///
/// # Shared ownership
///
/// Two callers need exactly this lookup shape — HK Regular / TOTP / QR
/// via [`completed::login_completed`], and GamePass via
/// [`gamepass::try_complete_gamepass_login`]. Both follow the WPF
/// `BeanfunClient.cs::GetCookie("bfWebToken")` (L153-163) pattern of
/// querying `CookieContainer.GetCookies(new Uri("https://{portal_host}/"))`
/// (L144-150) — i.e. the cookie set whose `Domain` attribute
/// domain-matches the portal host, per RFC 6265 §5.1.3.
///
/// Hoisting the helper here keeps the "portal-origin scoping + mutex
/// lock + case-insensitive name match" policy in one place. A future
/// flow that also needs `bfWebToken` lookup (e.g. multi-session
/// diagnostics) picks it up for free; a future policy change
/// (different cookie name / broader scope) lands in exactly one
/// location.
///
/// # Why scope matters
///
/// During the login redirect chain, beanfun sets `bfWebToken` on one
/// of the later hops — often with `Domain=.beanfun.com` or a
/// `portal_host`-specific Domain. An earlier hop may emit an
/// unrelated cookie (e.g. an auth session-id on `login.beanfun.com`)
/// that would NOT be visible to the portal host. `CookieStore::matches`
/// performs the same RFC 6265 visibility check WPF's `GetCookies(Uri)`
/// does, so feeding it the portal URL gives us exactly the cookie set
/// WPF would have seen.
///
/// Returns `None` when no `bfWebToken` cookie is visible from the
/// portal origin. The caller decides how to surface the miss — HK
/// Regular / QR / TOTP raise [`LoginError::MissingWebToken`] (WPF
/// L868-872 `if (this.webtoken == "") { errmsg = "LoginNoWebtoken"; }`);
/// GamePass treats it as "navigation isn't finished yet" and waits
/// for the next page load (WPF `GamePassBrowser.TryCompleteLogin`
/// L143-144 early-returns silently).
pub(super) fn read_bfwebtoken_from_jar(client: &BeanfunClient) -> Option<String> {
    let portal_base = &client.config().endpoints.portal_base;
    let store = client.cookie_store();
    let guard = store
        .lock()
        .expect("cookie store mutex must not be poisoned");
    guard
        .matches(portal_base)
        .into_iter()
        .find(|c| c.name().eq_ignore_ascii_case("bfWebToken"))
        .map(|c| c.value().to_owned())
}

// -----------------------------------------------------------------------------
// Shared response-parsing helpers
// -----------------------------------------------------------------------------

/// Upper bound on the body snippet we include in the `tracing::warn!`
/// emitted by [`parse_step_json`]. Picked at 500 **chars** (not bytes)
/// so multi-byte CJK responses don't get truncated mid-codepoint.
const BODY_LOG_PREVIEW_CHARS: usize = 500;

/// Parse a JSON body returned by a login step, emitting a
/// `tracing::warn!` on failure that includes a bounded body preview.
///
/// Centralises the "try to parse, log body on error" pattern used by
/// every JSON-bodied login step so operators have enough context to
/// diagnose a server response-shape regression without having to
/// redeploy a debug build. The full body is **not** logged — we cap
/// at `BODY_LOG_PREVIEW_CHARS` chars so a mis-routed page-sized
/// response (e.g. HTML error page) doesn't blow up the log volume.
///
/// The input is always already bounded by [`crate::services::beanfun::
/// BeanfunClient::bounded_text`], so the preview cap is defence in
/// depth against a future caller that forgets that guard — not a
/// substitute for it.
pub(crate) fn parse_step_json<T>(text: &str, step: &str) -> Result<T, LoginError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(text).map_err(|e| {
        tracing::warn!(
            step,
            error = %e,
            body_preview = %truncate_chars(text, BODY_LOG_PREVIEW_CHARS),
            "login step JSON parse failed"
        );
        LoginError::from(e)
    })
}

/// Return a borrowed prefix of `s` up to `max_chars` Unicode scalar
/// values. Cheap O(max_chars) scan; caller is expected to treat the
/// return as "at most N characters" — the rest of the string is
/// silently dropped.
fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

/// Serde `deserialize_with` helper mimicking Newtonsoft
/// `JToken.ToString()` semantics.
///
/// # Why this exists (P3 parity fix)
///
/// WPF reads the login-response fields `ResultCode` / `Result` /
/// `ResultMessage` / `ResultData.Captcha` via `JToken.ToString()`,
/// which silently coerces **any** JSON scalar (string / integer /
/// float / boolean / null) into its string form:
///
/// ```csharp
/// // BeanfunClient.Login.cs L97-99
/// string resultCode = loginJson["ResultCode"]?.ToString();
/// string result     = loginJson["Result"]?.ToString();
/// string resultMsg  = loginJson["ResultMessage"]?.ToString() ?? "";
/// ```
///
/// Beanfun's production server genuinely returns these fields as
/// integers in some branches (e.g. `ResultCode: 1` instead of
/// `ResultCode: "1"`). A strict `#[serde(rename = "ResultCode")]
/// result_code: Option<String>` on the Rust side therefore blows up
/// with `invalid type: integer 1, expected a string` — a regression
/// against WPF that only surfaces against the live server.
///
/// Applying `#[serde(deserialize_with = "deserialize_jtoken_to_string")]`
/// to the same set of fields restores the WPF lenient behaviour for
/// those specific call sites without weakening strict typing
/// elsewhere (other Beanfun endpoints use `(int)` / `(string)` casts
/// in WPF, which mirror strict serde behaviour and therefore do not
/// need this helper).
///
/// # Semantics
///
/// | JSON token    | Returned `Option<String>` |
/// |---------------|---------------------------|
/// | `"foo"`       | `Some("foo")`             |
/// | `1` (int)     | `Some("1")`               |
/// | `-1`          | `Some("-1")`              |
/// | `1.5` (float) | `Some("1.5")`             |
/// | `true`        | `Some("True")`  (matches .NET `bool.ToString()`) |
/// | `false`       | `Some("False")` (matches .NET `bool.ToString()`) |
/// | `null`        | `None`                    |
/// | missing key   | `None` (requires `#[serde(default)]` on field) |
///
/// Objects and arrays are rejected with a standard serde type-mismatch
/// error — Newtonsoft's `JToken.ToString()` would return a JSON
/// substring for those, but no observed Beanfun field returns a
/// nested value where a scalar is expected, so accepting them here
/// would only mask a future response-shape regression.
///
/// # Placement (SRP)
///
/// The helper lives here rather than in `services/beanfun/mod.rs`
/// because, at the time of writing, only the login flow exercises
/// the WPF `.ToString()` pattern. Hoisting to a broader module on
/// first speculative need would violate YAGNI; the second call site
/// outside `login/` is the promotion trigger.
pub(super) fn deserialize_jtoken_to_string<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct JTokenVisitor;

    impl<'de> Visitor<'de> for JTokenVisitor {
        type Value = Option<String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a JSON scalar (string, integer, float, boolean, or null)")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_owned()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // .NET `bool.ToString()` returns "True" / "False" with the
            // first letter capitalised — match it verbatim so any
            // downstream code that compares against those exact
            // strings continues to work.
            Ok(Some(if v {
                "True".to_owned()
            } else {
                "False".to_owned()
            }))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            d.deserialize_any(JTokenVisitor)
        }
    }

    de.deserialize_any(JTokenVisitor)
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(default, deserialize_with = "deserialize_jtoken_to_string")]
        v: Option<String>,
    }

    fn parse(json: &str) -> Option<String> {
        serde_json::from_str::<Wrapper>(json).expect("valid JSON").v
    }

    #[test]
    fn string_value_passes_through_unchanged() {
        assert_eq!(parse(r#"{"v":"hello"}"#), Some("hello".to_owned()));
    }

    #[test]
    fn positive_integer_coerces_to_string() {
        // The specific WPF-parity case: Beanfun server returns
        // `ResultCode: 1` and WPF's `.ToString()` yields "1".
        assert_eq!(parse(r#"{"v":1}"#), Some("1".to_owned()));
    }

    #[test]
    fn negative_integer_coerces_to_string() {
        assert_eq!(parse(r#"{"v":-1}"#), Some("-1".to_owned()));
    }

    #[test]
    fn float_coerces_to_string() {
        // No observed Beanfun field uses floats, but the coercion
        // should still succeed rather than reject the payload — this
        // pins the contract against a future server change.
        assert_eq!(parse(r#"{"v":1.5}"#), Some("1.5".to_owned()));
    }

    #[test]
    fn bool_true_matches_dot_net_capitalisation() {
        // .NET `true.ToString()` → "True" (capital T).
        assert_eq!(parse(r#"{"v":true}"#), Some("True".to_owned()));
    }

    #[test]
    fn bool_false_matches_dot_net_capitalisation() {
        assert_eq!(parse(r#"{"v":false}"#), Some("False".to_owned()));
    }

    #[test]
    fn null_becomes_none() {
        assert_eq!(parse(r#"{"v":null}"#), None);
    }

    #[test]
    fn missing_key_becomes_none() {
        assert_eq!(parse(r#"{}"#), None);
    }

    #[test]
    fn object_value_is_rejected() {
        assert!(serde_json::from_str::<Wrapper>(r#"{"v":{"a":1}}"#).is_err());
    }

    #[test]
    fn array_value_is_rejected() {
        assert!(serde_json::from_str::<Wrapper>(r#"{"v":[1,2]}"#).is_err());
    }

    #[test]
    fn truncate_chars_returns_full_str_when_shorter_than_max() {
        assert_eq!(truncate_chars("hi", 10), "hi");
    }

    #[test]
    fn truncate_chars_caps_multibyte_input_without_splitting_codepoint() {
        // "中文字符" = 4 chars, each 3 bytes → 12 bytes total.
        // `truncate_chars(_, 2)` must return exactly "中文" (6 bytes,
        // 2 chars) — not 2 bytes, which would land mid-codepoint.
        let out = truncate_chars("中文字符", 2);
        assert_eq!(out, "中文");
    }

    #[test]
    fn parse_step_json_surfaces_login_error_on_malformed_body() {
        // Smoke test that the wrapper returns `LoginError::Json` (via
        // the `#[from] serde_json::Error` conversion) rather than the
        // raw serde error. The `tracing::warn!` side-effect is not
        // asserted — `tracing-test` isn't pulled in as a dev-dep for
        // this crate; the structured fields are covered by manual
        // inspection plus the `truncate_chars` tests above.
        #[derive(Debug, serde::Deserialize)]
        struct T {
            #[allow(dead_code)]
            x: String,
        }
        let err = parse_step_json::<T>("not json", "TestStep").unwrap_err();
        assert!(matches!(err, LoginError::Json(_)));
    }
}
