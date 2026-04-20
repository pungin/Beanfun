//! QR-code login **init** step — fetches the QR PNG, the mobile-app
//! deeplink, and the antiforgery token the polling step needs.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Login.cs::GetQRCodeValue` (L409-453)
//! plus the inner `getQRCodeStrEncryptData` (L455-476) and the
//! `NormalizeBeanfunAppDeeplink` helper (L478-504). The WPF flow is:
//!
//! 1. `GET https://login.beanfun.com/Login/Index?pSKey={skey}` — scrape
//!    the `__RequestVerificationToken` hidden input out of the HTML
//!    (regex `__RequestVerificationToken[^>]+value="([^"]+)"`,
//!    L416-418).
//! 2. `GET https://login.beanfun.com/Login/InitLogin?pSKey={skey}` with
//!    `Accept: application/json, text/plain, */*`,
//!    `X-Requested-With: XMLHttpRequest`, and
//!    `Origin: https://login.beanfun.com` headers (L455-466). Parse the
//!    JSON body and require `Result == 0`; the payload's
//!    `ResultData.QRImage` is a base64-encoded PNG and
//!    `ResultData.DeepLink` is the mobile-app deeplink string.
//! 3. Run the deeplink through [`normalize_beanfun_app_deeplink`] —
//!    when the server returns a `play.games.gamania.com/.../deeplink/?url=…`
//!    wrapper, we unwrap the inner `url=` query parameter (L478-504).
//!
//! # Reused building blocks
//!
//! Step 1 is functionally identical to the TW Regular flow's first
//! call, so we delegate to [`super::get_login_index`] — same `GET
//! Login/Index?pSKey=...` request, same regex extraction, same typed
//! [`LoginError::MissingVerificationToken`] error mapping. Sharing
//! that path means a future tweak to the antiforgery extraction lands
//! in one place.
//!
//! # Region scope
//!
//! WPF UI explicitly disables the QR button when `App.LoginRegion ==
//! "HK"` (`MainWindow.xaml.cs::loginMethodInit` L1099-1114), and the
//! `BeanfunClient` QR code path hardcodes `https://login.beanfun.com`
//! regardless of region. We refuse the call early with
//! [`LoginError::QrUnsupportedRegion`] when the client targets HK so
//! callers get a typed error instead of a confusing transport / cookie
//! failure deeper in the flow.
//!
//! # Output shape
//!
//! [`QrLoginInit`] carries three fields that downstream consumers
//! (`qr_poll`, `qr_finalize`, the Tauri UI command) all need:
//!
//! - `bitmap_base64` — `data:image/png;base64,<…>` data URL the UI can
//!   embed straight into an `<img src=…>` tag. We preserve WPF's
//!   storage shape (`bitmapBase64 = "data:image/png;base64," + base64`,
//!   L449) verbatim so downstream code paths stay byte-compatible.
//! - `deeplink` — `Option<String>` carrying the post-normalisation
//!   deeplink. `None` when the server omitted `DeepLink` or sent it
//!   empty; `Some(...)` with the unwrapped url otherwise.
//! - `verification_token` — the `__RequestVerificationToken` value the
//!   polling step (`qr_poll::poll_qr_login_status`) sends as a header
//!   on every `CheckLoginStatus` POST.

use reqwest::header;
use serde::Deserialize;
use url::Url;

use super::{ensure_success, get_login_index};
use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion};

/// QR-code login bootstrap data — what `GetQRCodeValue` returns in WPF
/// (`QRCodeClass`, L401-407), reshaped to fit our typed-API style.
///
/// Cheap to clone (small `String`s only). The token + skey pair is
/// what the subsequent `qr_poll` and `qr_finalize` calls need to
/// drive the rest of the flow — bundling them here keeps the
/// downstream API surface to a single `&QrLoginInit` argument
/// instead of three loose `&str`s.
#[derive(Debug, Clone)]
pub struct QrLoginInit {
    /// Portal session key (`pSKey`). Forwarded by
    /// `qr_poll::poll_qr_login_status` and `qr_finalize` as the
    /// `Referer` URL's `pSKey=…` query parameter — WPF L538 / L618
    /// both rebuild the same `Login/Index?pSKey={skey}` referer
    /// from `qrcodeclass.skey`.
    pub skey: String,

    /// Full `data:image/png;base64,<base64>` data URL — UI can drop
    /// this straight into an `<img>` tag. WPF stores the same shape
    /// verbatim on `QRCodeClass.bitmapBase64` (L449).
    pub bitmap_base64: String,

    /// Beanfun mobile-app deeplink the user can scan / open. `None`
    /// when the server omitted `DeepLink` or sent it empty, matching
    /// WPF's "store whatever, even null" behaviour at L443-444.
    /// Already passed through [`normalize_beanfun_app_deeplink`] so
    /// `play.games.gamania.com/.../deeplink/?url=…` wrappers are
    /// unwrapped to the inner url.
    pub deeplink: Option<String>,

    /// `__RequestVerificationToken` value scraped from the `Login/Index`
    /// page. Forwarded by `qr_poll::poll_qr_login_status` as the
    /// `RequestVerificationToken` request header (WPF L621).
    pub verification_token: String,
}

/// Run the QR-code login bootstrap and return the
/// [`QrLoginInit`] payload the rest of the flow needs.
///
/// WPF reference: `BeanfunClient.Login.cs::GetQRCodeValue` (L409-453)
/// + `getQRCodeStrEncryptData` (L455-476).
///
/// Errors:
///
/// - [`LoginError::QrUnsupportedRegion`] when `client.config().region`
///   is `LoginRegion::HK` — see module docs.
/// - [`LoginError::MissingVerificationToken`] when step 1's HTML lacks
///   the antiforgery token (propagated from
///   [`super::get_login_index`]).
/// - [`LoginError::QrInitResultError`] when step 2's JSON body has
///   `Result != 0`, `Result` missing, `ResultData` missing, or
///   `ResultData.QRImage` empty / missing — matches WPF's
///   `errmsg = "LoginIntResultError"` (L469-473) and the three layered
///   null-checks at `GetQRCodeValue` L429-441.
/// - [`LoginError::Json`] when step 2's body is not parseable JSON.
///   WPF's `JObject.Parse` would throw `JsonReaderException` here and
///   crash the timer thread; surfacing a typed error is strictly
///   safer (same rationale as P3 chunk 3.3.4 — see
///   `login/registered_device.rs` module docs).
/// - [`LoginError::Http`] / [`LoginError::Unknown`] for transport-
///   level failures.
pub async fn init_qr_login(
    client: &BeanfunClient,
    session_key: &str,
) -> Result<QrLoginInit, LoginError> {
    if client.config().region != LoginRegion::TW {
        return Err(LoginError::QrUnsupportedRegion);
    }

    // Step 1 — same `GET Login/Index?pSKey=...` call the TW Regular
    // flow makes; reuse it so the antiforgery extraction lives in
    // one place.
    let index = get_login_index(client, session_key).await?;

    // Step 2 — JSON GET. WPF's `Origin` header is the bare scheme +
    // host of the login base; `Url::origin().ascii_serialization()`
    // yields exactly that ("https://login.beanfun.com" — no trailing
    // slash, no path, default port omitted) which keeps prod and
    // mock URLs both correct.
    let init_url = client.login_url_with_skey("Login/InitLogin", session_key)?;
    let origin = client
        .config()
        .endpoints
        .login_base
        .origin()
        .ascii_serialization();

    let resp = client
        .http()
        .get(init_url)
        .header(header::ACCEPT, "application/json, text/plain, */*")
        .header(header::REFERER, index.index_url.as_str())
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Origin", origin)
        .send()
        .await?;

    ensure_success(&resp, "Login/InitLogin")?;
    let body = client.bounded_text(resp).await?;
    let parsed: InitLoginResponse = serde_json::from_str(&body)?;

    // WPF L469: `Result == null || (int)Result != 0` → LoginIntResultError.
    let result_code = parsed.result.ok_or(LoginError::QrInitResultError)?;
    if result_code != 0 {
        return Err(LoginError::QrInitResultError);
    }

    // WPF L429-434: `ResultData == null` → LoginIntResultError.
    let result_data = parsed.result_data.ok_or(LoginError::QrInitResultError)?;

    // WPF L436-441: `string.IsNullOrEmpty(QRImage)` → LoginIntResultError.
    // We treat both "field absent" and "empty string" as the same failure
    // because WPF's `string.IsNullOrEmpty` collapses null and "" to one
    // branch.
    let raw_image = result_data
        .qr_image
        .filter(|s| !s.is_empty())
        .ok_or(LoginError::QrInitResultError)?;

    // WPF L449: `bitmapBase64 = "data:image/png;base64," + base64Image`.
    // We preserve that exact storage shape so downstream consumers
    // (UI, future serialisation) stay byte-compatible.
    let bitmap_base64 = format!("data:image/png;base64,{raw_image}");

    // WPF L443-444: `result["DeepLink"]?.Value<string>()` returns null
    // when missing; `NormalizeBeanfunAppDeeplink(null)` returns null.
    // We mirror that with `Option<String>`: `None` when raw was
    // null/empty, `Some(unwrapped)` otherwise.
    let deeplink = result_data
        .deep_link
        .as_deref()
        .map(normalize_beanfun_app_deeplink)
        .filter(|s| !s.is_empty());

    Ok(QrLoginInit {
        skey: session_key.to_owned(),
        bitmap_base64,
        deeplink,
        verification_token: index.verification_token,
    })
}

/// Unwrap a Beanfun mobile-app deeplink that the QR backend wraps in a
/// `play.games.gamania.com/.../deeplink/?url=…` redirect.
///
/// Pure helper, no I/O. WPF reference: `BeanfunClient.Login.cs::
/// NormalizeBeanfunAppDeeplink` (L478-504).
///
/// Returns the input verbatim when:
///
/// - Input is empty / whitespace-only.
/// - Input is not a parseable absolute URL.
/// - URL host is not `play.games.gamania.com` (case-insensitive,
///   matching WPF L490).
/// - URL path does not contain `deeplink` (case-insensitive, matching
///   WPF L495 `IndexOf("deeplink", OrdinalIgnoreCase)`).
/// - URL has matching host + path but no `?url=` query parameter, or
///   the `url=` value is empty.
///
/// Otherwise, returns the decoded `?url=` value.
pub fn normalize_beanfun_app_deeplink(raw: &str) -> String {
    if raw.trim().is_empty() {
        return raw.to_owned();
    }

    let Ok(uri) = Url::parse(raw.trim()) else {
        return raw.to_owned();
    };

    // WPF L487-491 — host check, case-insensitive.
    let host_matches = uri
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case("play.games.gamania.com"));
    if !host_matches {
        return raw.to_owned();
    }

    // WPF L495 — `uri.AbsolutePath.IndexOf("deeplink", OrdinalIgnoreCase)`.
    if !uri.path().to_ascii_lowercase().contains("deeplink") {
        return raw.to_owned();
    }

    // WPF L498-501 — `query["url"]`, where `NameValueCollection`'s
    // default comparer is case-insensitive. We approximate with
    // `eq_ignore_ascii_case` and "first match wins" — the real server
    // emits exactly one `url=` parameter, so the multi-value
    // join-with-comma corner case `NameValueCollection` would handle
    // never fires in practice.
    for (k, v) in uri.query_pairs() {
        if k.eq_ignore_ascii_case("url") && !v.is_empty() {
            return v.into_owned();
        }
    }

    raw.to_owned()
}

// -----------------------------------------------------------------------------
// JSON shape — private to this module
// -----------------------------------------------------------------------------

/// Outer envelope of the `Login/InitLogin` JSON response. WPF accesses
/// these fields via `JObject` indexer (`json["Result"]`, etc.) — we
/// model them with `Option` so missing fields surface cleanly as
/// [`LoginError::QrInitResultError`] in the layered checks above
/// rather than as a `serde_json::Error` mid-parse.
#[derive(Debug, Deserialize)]
struct InitLoginResponse {
    #[serde(rename = "Result")]
    result: Option<i64>,
    #[serde(rename = "ResultData")]
    result_data: Option<InitLoginResultData>,
}

/// Inner `ResultData` payload — contains the actual base64 PNG and the
/// (optional) deeplink string. WPF treats both fields as nullable
/// strings (L436-444) so we do too.
#[derive(Debug, Deserialize)]
struct InitLoginResultData {
    #[serde(rename = "QRImage")]
    qr_image: Option<String>,
    #[serde(rename = "DeepLink")]
    deep_link: Option<String>,
}

// -----------------------------------------------------------------------------
// Unit tests — pure helpers only. The full `init_qr_login` orchestration
// is covered end-to-end in `tests/qr_init.rs`.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // normalize_beanfun_app_deeplink
    // -------------------------------------------------------------------------

    #[test]
    fn normalize_passes_through_empty_input() {
        assert_eq!(normalize_beanfun_app_deeplink(""), "");
        assert_eq!(normalize_beanfun_app_deeplink("   "), "   ");
    }

    #[test]
    fn normalize_passes_through_non_url_input() {
        let raw = "not a url at all";
        assert_eq!(normalize_beanfun_app_deeplink(raw), raw);
    }

    #[test]
    fn normalize_passes_through_when_host_does_not_match() {
        // Different host → no unwrap, even if path contains "deeplink".
        let raw = "https://example.com/deeplink/?url=https://other.example.com/x";
        assert_eq!(normalize_beanfun_app_deeplink(raw), raw);
    }

    #[test]
    fn normalize_passes_through_when_path_lacks_deeplink_marker() {
        // Right host, wrong path → return verbatim.
        let raw = "https://play.games.gamania.com/redirect?url=https://target.example/";
        assert_eq!(normalize_beanfun_app_deeplink(raw), raw);
    }

    #[test]
    fn normalize_unwraps_inner_url_query_param() {
        let raw = "https://play.games.gamania.com/app/deeplink/?url=https://target.example/auth?token=abc";
        // `url::Url::query_pairs` already URL-decodes; the inner URL
        // arrives at the caller in its native form.
        assert_eq!(
            normalize_beanfun_app_deeplink(raw),
            "https://target.example/auth?token=abc"
        );
    }

    #[test]
    fn normalize_host_match_is_case_insensitive() {
        // WPF L487-491 uses `OrdinalIgnoreCase`. We mirror that.
        let raw = "https://Play.Games.Gamania.COM/app/DeepLink/?url=https://t.example/";
        assert_eq!(normalize_beanfun_app_deeplink(raw), "https://t.example/");
    }

    #[test]
    fn normalize_returns_raw_when_inner_url_param_empty() {
        // Matching host + path but `url=` is empty → fall back to raw.
        let raw = "https://play.games.gamania.com/deeplink/?url=";
        assert_eq!(normalize_beanfun_app_deeplink(raw), raw);
    }

    #[test]
    fn normalize_returns_raw_when_inner_url_param_missing() {
        // Matching host + path but no `url` parameter → raw.
        let raw = "https://play.games.gamania.com/deeplink/?other=value";
        assert_eq!(normalize_beanfun_app_deeplink(raw), raw);
    }

    // -------------------------------------------------------------------------
    // InitLoginResponse serde shape — locks the field-name spellings so a
    // future rename can't silently break wire-compat. The full happy /
    // error path coverage lives in `tests/qr_init.rs`.
    // -------------------------------------------------------------------------

    #[test]
    fn init_login_response_parses_full_shape() {
        let body = r#"{
            "Result": 0,
            "ResultData": {
                "QRImage": "iVBORw0KGgoAAAANSUhEUgAA",
                "DeepLink": "https://target.example/"
            }
        }"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(parsed.result, Some(0));
        let data = parsed.result_data.expect("ResultData present");
        assert_eq!(data.qr_image.as_deref(), Some("iVBORw0KGgoAAAANSUhEUgAA"));
        assert_eq!(data.deep_link.as_deref(), Some("https://target.example/"));
    }

    #[test]
    fn init_login_response_tolerates_missing_optional_fields() {
        // ResultData absent + DeepLink absent are both legal "Option"
        // shapes; the orchestrator's layered checks decide whether
        // they constitute a hard error.
        let body = r#"{"Result": -1}"#;
        let parsed: InitLoginResponse = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(parsed.result, Some(-1));
        assert!(parsed.result_data.is_none());
    }
}
