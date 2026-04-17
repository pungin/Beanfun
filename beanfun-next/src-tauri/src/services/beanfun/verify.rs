//! Advance-check verify flow — port of `BeanfunClient.Verify.cs` +
//! `MainWindow.xaml.cs::reLoadVerifyPage` / `verifyWorker_DoWork`.
//!
//! Triggered when [`super::LoginError::AdvanceCheckRequired`] surfaces
//! mid-login: the server demands the user solve a captcha + re-enter a
//! second authentication factor (email / SMS code) on
//! `https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx`
//! before account_login can succeed.
//!
//! ```text
//!   1. GET  AdvanceCheck.aspx                 -> HTML form (viewstate, captcha id, auth-type label)
//!   2. GET  BotDetectCaptcha.ashx?...         -> raw image bytes for the user
//!   3. POST AdvanceCheck.aspx                 -> classified outcome (success / server msg / wrong captcha / wrong auth info)
//! ```
//!
//! # Region asymmetry: TW only (deliberate)
//!
//! All three endpoints — `AdvanceCheck.aspx` GET, `BotDetectCaptcha.ashx`
//! GET, `AdvanceCheck.aspx` POST — are hardcoded to
//! `https://tw.newlogin.beanfun.com/...` in WPF
//! (`BeanfunClient.Verify.cs` L23-25 / L43-45 / L90-92, plus
//! `MainWindow.xaml.cs::reLoadVerifyPage` L797-803 which strips and
//! re-prepends the TW host onto the form action). Furthermore
//! `BeanfunClient.advanceCheckUrl` (L186 in `BeanfunClient.Login.cs`)
//! is only ever set on the **TW** account_login branch
//! (`resultCode == "2"`), never on HK Regular (L249) or HK TOTP
//! (L361).
//!
//! HK regular / TOTP flows still produce `LoginAdvanceCheck` errmsg
//! strings on captcha-required responses, but invoking verify on an HK
//! session is a **silent dead path** in WPF (the GET would hit a TW
//! host that has no idea about this HK session). To avoid replicating
//! that broken-by-design behaviour, every public function in this
//! module rejects non-TW clients with
//! [`LoginError::VerifyUnsupportedRegion`] up front. UI is expected
//! to surface "please re-login" instead of opening the verify
//! dialog when the underlying session is HK.
//!
//! # State model
//!
//! Same shape as the rest of P3/P4: every call takes
//! `&BeanfunClient` plus pure inputs. The optional "where to GET the
//! verify page from" is threaded **through**
//! [`LoginError::AdvanceCheckRequired`]'s `url: Option<String>` field
//! (set by [`super::login::account_login()`] on TW resultCode 2,
//! L186), so this module holds **no** mutable state. WPF stores
//! `advanceCheckUrl`, `verifyFormAction`, `verifyViewStateGenerator`,
//! `samplecaptcha`, `viewstate`, `eventvalidation` on the
//! `BeanfunClient` / `MainWindow` instance; we instead bundle them
//! into a [`VerifyPageInfo`] value the caller passes back into
//! [`submit_verify`].
//!
//! # Outcome classification
//!
//! `verifyWorker_DoWork` (`MainWindow.xaml.cs` L2616-2679) interprets
//! the POST response with three checks:
//!
//! | Response shape                                     | WPF action                              | [`VerifyOutcome`] variant |
//! |----------------------------------------------------|-----------------------------------------|---------------------------|
//! | Contains `alert('資料已驗證成功')`                 | `e.Result = true` → `do_Login`          | [`VerifyOutcome::Success`] |
//! | Contains `alert('其他訊息')`                       | `MessageBox.Show(msg)`                  | [`VerifyOutcome::ServerMessage`] |
//! | No `alert`, contains `圖形驗證碼輸入錯誤`          | `MessageBox.Show(WrongCaptcha)`         | [`VerifyOutcome::WrongCaptcha`] |
//! | No `alert`, no `圖形驗證碼輸入錯誤`                | `MessageBox.Show(WrongAuthInfo)`        | [`VerifyOutcome::WrongAuthInfo`] |
//!
//! All four outcomes are **HTTP 200 OK** business results, so
//! [`submit_verify`] returns them through `Ok(VerifyOutcome)`. Only
//! transport / parse failures take the `Err` channel.
//!
//! # WPF dev artifacts (NOT ported)
//!
//! - `Debug.WriteLine($"[Captcha] ...")` (L50 / L63 in Verify.cs):
//!   diagnostic logging, no behavioural effect.
//! - `BitmapImage` decoding (L54-59 in Verify.cs): WPF's UI layer
//!   converts the bytes to an in-memory image. Our Rust port returns
//!   raw `Vec<u8>` and lets the Tauri command layer base64-encode for
//!   the frontend `<img>`.

use std::sync::OnceLock;

use regex::Regex;
use reqwest::Response;

use crate::core::parser::{capture_first, extract_viewstate};
use crate::services::beanfun::client::{BeanfunClient, LoginRegion};
use crate::services::beanfun::error::LoginError;
use crate::services::beanfun::login::ensure_success;

// -----------------------------------------------------------------------------
// Public API — types
// -----------------------------------------------------------------------------

/// Parsed shape of an `AdvanceCheck.aspx` page.
///
/// Built by [`get_verify_page_info`] from the server's HTML; consumed
/// by [`submit_verify`]. `viewstate_generator` is `Option` because
/// WPF stores it only when present (`MainWindow.xaml.cs` L766-770);
/// every other field is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyPageInfo {
    /// `__VIEWSTATE` hidden field — required.
    pub viewstate: String,
    /// `__VIEWSTATEGENERATOR` hidden field — optional per WPF
    /// `MainWindow.xaml.cs` L766-770.
    pub viewstate_generator: Option<String>,
    /// `__EVENTVALIDATION` hidden field — required.
    pub event_validation: String,
    /// `LBD_VCID_*` captcha id — required. Becomes the `t=` query
    /// parameter on the captcha image URL **and** the
    /// `LBD_VCID_c_logincheck_advancecheck_samplecaptcha` form field
    /// on submit.
    pub samplecaptcha: String,
    /// `lblAuthType` label text — required. UI surfaces this so the
    /// user knows whether they're being asked for an email or SMS
    /// code.
    pub lbl_auth_type: String,
    /// Resolved absolute URL to POST the verify form back to.
    /// Either the TW-prepended `action="AdvanceCheck.aspx?..."` from
    /// the form, or the static fallback when no form action is found.
    pub form_action: String,
}

/// Classified outcome of a [`submit_verify`] call.
///
/// All four variants are valid HTTP-200 responses; this enum captures
/// the four ways `verifyWorker_DoWork` (`MainWindow.xaml.cs`
/// L2616-2679) reads the response body. See module docs for the
/// classification table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// `alert('資料已驗證成功');` — caller should resume the login flow.
    Success,
    /// `alert('其他訊息');` — server returned a non-success, non-captcha
    /// alert. WPF renders the message verbatim; we carry it for the UI
    /// to display / localise.
    ServerMessage(String),
    /// No `alert`, body contains `圖形驗證碼輸入錯誤` — captcha mistyped.
    WrongCaptcha,
    /// No `alert`, body lacks the captcha-error string — typically
    /// "wrong auth info" (email/SMS code). WPF shows a generic
    /// `WrongAuthInfo` resource string.
    WrongAuthInfo,
}

// -----------------------------------------------------------------------------
// Public API — functions
// -----------------------------------------------------------------------------

/// Step 1 — fetch the AdvanceCheck.aspx HTML and parse it into a
/// [`VerifyPageInfo`].
///
/// `advance_check_url` is whatever
/// [`LoginError::AdvanceCheckRequired::url`] carried out of the
/// upstream login call; pass `None` to use the static TW fallback
/// (`https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx`).
/// Mirrors `BeanfunClient.Verify.cs::getVerifyPageInfo` L23-26.
///
/// # Errors
///
/// - [`LoginError::VerifyUnsupportedRegion`] when `client.config().region`
///   is not [`LoginRegion::TW`] — see module docs for why HK is rejected
///   instead of replicating the WPF dead path.
/// - [`LoginError::ServerMessage`] when the page contains an
///   `alert('...')` script (per `reLoadVerifyPage` L805-810 which
///   surfaces the alert text as the errmsg).
/// - [`LoginError::VerifyMissingViewState`] /
///   [`LoginError::VerifyMissingEventValidation`] /
///   [`LoginError::VerifyMissingSampleCaptcha`] /
///   [`LoginError::VerifyMissingLblAuthType`] for missing required fields.
pub async fn get_verify_page_info(
    client: &BeanfunClient,
    advance_check_url: Option<&str>,
) -> Result<VerifyPageInfo, LoginError> {
    ensure_tw(client)?;
    let url = match advance_check_url {
        Some(u) if !u.is_empty() => u.to_owned(),
        _ => build_default_advance_check_url(client)?,
    };
    let resp = client.http().get(&url).send().await?;
    ensure_success(&resp, "AdvanceCheck.aspx (GET)")?;
    let body = client.bounded_text(resp).await?;
    parse_verify_page(client, &body)
}

/// Step 2 — fetch the captcha image bytes for `samplecaptcha`.
///
/// `samplecaptcha` is the value of [`VerifyPageInfo::samplecaptcha`].
/// The returned bytes are typically a PNG; the Tauri command layer is
/// expected to base64-encode them for an `<img src="data:...">`.
///
/// Mirrors `BeanfunClient.Verify.cs::getVerifyCaptcha` L35-67 with the
/// same `< 500 bytes` rejection threshold (L48).
///
/// # Errors
///
/// - [`LoginError::VerifyUnsupportedRegion`] for non-TW clients.
/// - [`LoginError::VerifyCaptchaImageTooSmall`] when the response body
///   is < 500 bytes — matches WPF's `buffer.Length < 500` check that
///   returns `null` (treated as "captcha load failed").
pub async fn get_verify_captcha(
    client: &BeanfunClient,
    samplecaptcha: &str,
) -> Result<Vec<u8>, LoginError> {
    ensure_tw(client)?;
    let url = build_captcha_url(client, samplecaptcha)?;
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "BotDetectCaptcha.ashx")?;
    let bytes = bounded_bytes(client, resp).await?;
    if bytes.len() < CAPTCHA_MIN_SIZE {
        return Err(LoginError::VerifyCaptchaImageTooSmall {
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}

/// Step 3 — submit the verify form with `verify_code` (auth code) and
/// `captcha_code` (typed-out captcha) and classify the response.
///
/// Mirrors `BeanfunClient.Verify.cs::verify` L69-100 (POST with the
/// 8-field form) plus `MainWindow.xaml.cs::verifyWorker_DoWork`
/// L2616-2679 (response classification).
///
/// # Errors
///
/// - [`LoginError::VerifyUnsupportedRegion`] for non-TW clients.
/// - Transport / parse failures bubble through the usual variants.
pub async fn submit_verify(
    client: &BeanfunClient,
    page_info: &VerifyPageInfo,
    verify_code: &str,
    captcha_code: &str,
) -> Result<VerifyOutcome, LoginError> {
    ensure_tw(client)?;
    let form = build_verify_form(page_info, verify_code, captcha_code);
    let resp = client
        .http()
        .post(&page_info.form_action)
        .form(&form)
        .send()
        .await?;
    ensure_success(&resp, "AdvanceCheck.aspx (POST)")?;
    let body = client.bounded_text(resp).await?;
    Ok(classify_verify_response(&body))
}

// -----------------------------------------------------------------------------
// Private helpers — region guard
// -----------------------------------------------------------------------------

/// Reject non-TW clients early with [`LoginError::VerifyUnsupportedRegion`].
///
/// Centralised so all three public entry points share one
/// implementation; called as the very first statement of each.
fn ensure_tw(client: &BeanfunClient) -> Result<(), LoginError> {
    if client.config().region != LoginRegion::TW {
        return Err(LoginError::VerifyUnsupportedRegion);
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Private helpers — URL construction
// -----------------------------------------------------------------------------

/// Static fallback path used when the upstream login call did not
/// surface an `advanceCheckUrl`. Joined onto `newlogin_base` so
/// wiremock tests can route this onto the mock server transparently.
const ADVANCE_CHECK_PATH: &str = "LoginCheck/AdvanceCheck.aspx";

/// Captcha endpoint path — same `LoginCheck/` parent.
const BOT_DETECT_CAPTCHA_PATH: &str = "LoginCheck/BotDetectCaptcha.ashx";

/// Fixed `c=` query parameter on the captcha URL — the WPF source
/// hardcodes this exact key (L44 in Verify.cs).
const CAPTCHA_C_KEY: &str = "c_logincheck_advancecheck_samplecaptcha";

/// WPF `getVerifyCaptcha` rejects images < 500 bytes (L48). The
/// threshold is empirical (real PNG captchas are several KB) and
/// guards against the server returning an HTML error page in place
/// of an image without setting a proper non-2xx status.
const CAPTCHA_MIN_SIZE: usize = 500;

fn build_default_advance_check_url(client: &BeanfunClient) -> Result<String, LoginError> {
    Ok(client.newlogin_url(ADVANCE_CHECK_PATH)?.to_string())
}

fn build_captcha_url(client: &BeanfunClient, samplecaptcha: &str) -> Result<url::Url, LoginError> {
    let mut url = client.newlogin_url(BOT_DETECT_CAPTCHA_PATH)?;
    url.query_pairs_mut()
        .append_pair("get", "image")
        .append_pair("c", CAPTCHA_C_KEY)
        .append_pair("t", samplecaptcha);
    Ok(url)
}

// -----------------------------------------------------------------------------
// Private helpers — form construction
// -----------------------------------------------------------------------------

/// Build the 8 (or 7, when `__VIEWSTATEGENERATOR` is absent) form
/// fields the verify POST sends.
///
/// Field order matches `BeanfunClient.Verify.cs::verify` L79-88
/// exactly. `__VIEWSTATEGENERATOR` is conditional (per L81-82
/// `if (!string.IsNullOrEmpty(...))`); every other field is
/// unconditional.
fn build_verify_form<'a>(
    page_info: &'a VerifyPageInfo,
    verify_code: &'a str,
    captcha_code: &'a str,
) -> Vec<(&'static str, &'a str)> {
    let mut form: Vec<(&'static str, &'a str)> =
        Vec::with_capacity(if page_info.viewstate_generator.is_some() {
            8
        } else {
            7
        });

    form.push(("__VIEWSTATE", page_info.viewstate.as_str()));
    if let Some(gen) = page_info.viewstate_generator.as_deref() {
        form.push(("__VIEWSTATEGENERATOR", gen));
    }
    form.push(("__EVENTVALIDATION", page_info.event_validation.as_str()));
    form.push(("txtVerify", verify_code));
    form.push(("CodeTextBox", captcha_code));
    form.push(("imgbtnSubmit.x", "19"));
    form.push(("imgbtnSubmit.y", "23"));
    form.push((
        "LBD_VCID_c_logincheck_advancecheck_samplecaptcha",
        page_info.samplecaptcha.as_str(),
    ));
    form
}

// -----------------------------------------------------------------------------
// Private helpers — bounded byte read (sibling of BeanfunClient::bounded_text)
// -----------------------------------------------------------------------------

/// Stream `resp` into a `Vec<u8>`, capped at
/// [`super::ClientConfig::max_body_size`].
///
/// Mirrors [`BeanfunClient::bounded_text`] but skips the UTF-8
/// validation pass — captcha responses are PNG bytes, not text.
/// Lives here rather than on [`BeanfunClient`] because it is the
/// only byte-returning call across the entire service surface;
/// promoting it to a public client method would invite misuse.
async fn bounded_bytes(client: &BeanfunClient, resp: Response) -> Result<Vec<u8>, LoginError> {
    let cap = client.config().max_body_size;

    if let Some(reported) = resp.content_length() {
        let reported = reported as usize;
        if reported > cap {
            return Err(LoginError::BodyTooLarge {
                limit: cap,
                actual: reported,
            });
        }
    }

    let mut resp = resp;
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len().saturating_add(chunk.len()) > cap {
            return Err(LoginError::BodyTooLarge {
                limit: cap,
                actual: buf.len() + chunk.len(),
            });
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

// -----------------------------------------------------------------------------
// Private helpers — HTML parsing
// -----------------------------------------------------------------------------

/// Memoised regex for the inline `<script>alert('...')</script>` shape
/// `MainWindow.xaml.cs::reLoadVerifyPage` L806 looks for. Same
/// pattern is reused by `verifyWorker_DoWork` L2634 — capturing the
/// quoted message body in group 1.
fn alert_message_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"alert\('(.*)'\);"#).expect("alert regex must compile"))
}

/// Captcha id (`LBD_VCID_*`) hidden field. WPF L781:
/// `id="LBD_VCID_[^"]+"[^>]+value="([^"]+)"`.
fn samplecaptcha_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"id="LBD_VCID_[^"]+"[^>]+value="([^"]+)""#)
            .expect("samplecaptcha regex must compile")
    })
}

/// `lblAuthType` label content. WPF L789:
/// `id="lblAuthType">([^<]+)<`.
fn lbl_auth_type_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"id="lblAuthType">([^<]+)<"#).expect("lblAuthType regex must compile")
    })
}

/// Form action URL fragment. WPF L797:
/// `action="(AdvanceCheck\.aspx[^"]+)"`. Note WPF L800 then does
/// `.Replace("&amp;", "&")` and prepends the TW host explicitly, so
/// we mirror both transformations here.
fn form_action_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"action="(AdvanceCheck\.aspx[^"]+)""#).expect("form action regex must compile")
    })
}

/// Parse `html` into a [`VerifyPageInfo`].
///
/// Pure function — no I/O. Extracted so unit tests can cover every
/// missing-field branch without spinning up wiremock.
///
/// # 1:1 alignment notes
///
/// WPF `reLoadVerifyPage` (`MainWindow.xaml.cs` L753-814) checks
/// fields in this order: `__VIEWSTATE` → `__VIEWSTATEGENERATOR`
/// (optional) → `__EVENTVALIDATION` → captcha id → auth-type
/// label → form action (optional) → alert. We preserve the same
/// order so error reporting matches WPF exactly when multiple
/// fields are missing.
///
/// The alert check is **last** intentionally: WPF still loads the
/// other fields onto `bfClient` / `verifyPage` before returning
/// the alert message, but for our purposes the alert short-circuits
/// the rest of the flow (caller will not POST the form), so an
/// alert with otherwise-malformed HTML still surfaces as
/// [`LoginError::ServerMessage`] via this branch.
fn parse_verify_page(client: &BeanfunClient, html: &str) -> Result<VerifyPageInfo, LoginError> {
    let viewstate_form = extract_viewstate(html).map_err(|_| LoginError::VerifyMissingViewState)?;
    let event_validation = viewstate_form
        .event_validation
        .ok_or(LoginError::VerifyMissingEventValidation)?;
    let samplecaptcha =
        capture_first(samplecaptcha_regex(), html).ok_or(LoginError::VerifyMissingSampleCaptcha)?;
    let lbl_auth_type =
        capture_first(lbl_auth_type_regex(), html).ok_or(LoginError::VerifyMissingLblAuthType)?;

    if let Some(msg) = capture_first(alert_message_regex(), html) {
        return Err(LoginError::ServerMessage(msg));
    }

    let form_action = match capture_first(form_action_regex(), html) {
        Some(action) => {
            let decoded = action.replace("&amp;", "&");
            client
                .newlogin_url(&format!("LoginCheck/{decoded}"))?
                .to_string()
        }
        None => build_default_advance_check_url(client)?,
    };

    Ok(VerifyPageInfo {
        viewstate: viewstate_form.viewstate,
        viewstate_generator: viewstate_form.viewstate_generator,
        event_validation,
        samplecaptcha,
        lbl_auth_type,
        form_action,
    })
}

// -----------------------------------------------------------------------------
// Private helpers — response classification
// -----------------------------------------------------------------------------

/// Hard-coded server-string sentinels. These are Chinese strings the
/// server returns verbatim, not localisable resources — WPF compares
/// against them with `Contains` at `MainWindow.xaml.cs` L2642 and
/// L2653.
const ALERT_SUCCESS_KEYWORD: &str = "資料已驗證成功";
const WRONG_CAPTCHA_KEYWORD: &str = "圖形驗證碼輸入錯誤";

/// Classify a verify POST response body into one of the four
/// [`VerifyOutcome`] variants.
///
/// Pure function — no I/O. Mirrors `verifyWorker_DoWork`
/// `MainWindow.xaml.cs` L2634-2661 step by step:
///
/// 1. If body matches `alert\\('(.*)'\\);` → look at the captured msg:
///    - msg contains `資料已驗證成功` → [`VerifyOutcome::Success`]
///    - else → [`VerifyOutcome::ServerMessage`] carrying the captured `msg`
/// 2. Else (no alert):
///    - body contains `圖形驗證碼輸入錯誤` → [`VerifyOutcome::WrongCaptcha`]
///    - else → [`VerifyOutcome::WrongAuthInfo`]
fn classify_verify_response(body: &str) -> VerifyOutcome {
    if let Some(msg) = capture_first(alert_message_regex(), body) {
        if msg.contains(ALERT_SUCCESS_KEYWORD) {
            VerifyOutcome::Success
        } else {
            VerifyOutcome::ServerMessage(msg)
        }
    } else if body.contains(WRONG_CAPTCHA_KEYWORD) {
        VerifyOutcome::WrongCaptcha
    } else {
        VerifyOutcome::WrongAuthInfo
    }
}

// -----------------------------------------------------------------------------
// Tests (pure helpers; integration tests live in tests/verify.rs)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::beanfun::client::ClientConfig;

    fn tw_client() -> BeanfunClient {
        BeanfunClient::new(ClientConfig::for_region(LoginRegion::TW)).unwrap()
    }

    fn hk_client() -> BeanfunClient {
        BeanfunClient::new(ClientConfig::for_region(LoginRegion::HK)).unwrap()
    }

    // -------------------------------------------------------------------------
    // ensure_tw
    // -------------------------------------------------------------------------

    #[test]
    fn ensure_tw_accepts_tw_client() {
        assert!(ensure_tw(&tw_client()).is_ok());
    }

    #[test]
    fn ensure_tw_rejects_hk_client_with_typed_error() {
        assert!(matches!(
            ensure_tw(&hk_client()).unwrap_err(),
            LoginError::VerifyUnsupportedRegion
        ));
    }

    // -------------------------------------------------------------------------
    // build_captcha_url
    // -------------------------------------------------------------------------

    #[test]
    fn captcha_url_includes_get_image_c_and_t_params() {
        let url = build_captcha_url(&tw_client(), "VCID_test_value").unwrap();
        let s = url.as_str();
        assert!(
            s.starts_with("https://tw.newlogin.beanfun.com/LoginCheck/BotDetectCaptcha.ashx?"),
            "got: {s}"
        );
        assert!(s.contains("get=image"), "got: {s}");
        assert!(
            s.contains("c=c_logincheck_advancecheck_samplecaptcha"),
            "got: {s}"
        );
        assert!(s.contains("t=VCID_test_value"), "got: {s}");
    }

    // -------------------------------------------------------------------------
    // build_default_advance_check_url
    // -------------------------------------------------------------------------

    #[test]
    fn default_advance_check_url_targets_tw_newlogin_host() {
        let url = build_default_advance_check_url(&tw_client()).unwrap();
        assert_eq!(
            url,
            "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx"
        );
    }

    // -------------------------------------------------------------------------
    // build_verify_form
    // -------------------------------------------------------------------------

    fn page_info_with_generator() -> VerifyPageInfo {
        VerifyPageInfo {
            viewstate: "VS_TOK".into(),
            viewstate_generator: Some("GEN_TOK".into()),
            event_validation: "EV_TOK".into(),
            samplecaptcha: "VCID_TOK".into(),
            lbl_auth_type: "Email".into(),
            form_action: "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx".into(),
        }
    }

    fn page_info_without_generator() -> VerifyPageInfo {
        VerifyPageInfo {
            viewstate_generator: None,
            ..page_info_with_generator()
        }
    }

    #[test]
    fn verify_form_has_eight_fields_when_generator_present() {
        let info = page_info_with_generator();
        let form = build_verify_form(&info, "VCODE", "CCODE");
        assert_eq!(form.len(), 8);
        let names: Vec<&str> = form.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            names,
            vec![
                "__VIEWSTATE",
                "__VIEWSTATEGENERATOR",
                "__EVENTVALIDATION",
                "txtVerify",
                "CodeTextBox",
                "imgbtnSubmit.x",
                "imgbtnSubmit.y",
                "LBD_VCID_c_logincheck_advancecheck_samplecaptcha",
            ]
        );
    }

    #[test]
    fn verify_form_drops_generator_when_absent() {
        let info = page_info_without_generator();
        let form = build_verify_form(&info, "VCODE", "CCODE");
        assert_eq!(form.len(), 7);
        let names: Vec<&str> = form.iter().map(|(k, _)| *k).collect();
        assert!(!names.contains(&"__VIEWSTATEGENERATOR"));
        assert_eq!(names[0], "__VIEWSTATE");
        assert_eq!(names[1], "__EVENTVALIDATION");
    }

    #[test]
    fn verify_form_uses_literal_19_and_23_for_imgbtn_coords() {
        let info = page_info_with_generator();
        let form = build_verify_form(&info, "v", "c");
        let pairs: std::collections::HashMap<&str, &str> = form.iter().copied().collect();
        assert_eq!(pairs["imgbtnSubmit.x"], "19");
        assert_eq!(pairs["imgbtnSubmit.y"], "23");
    }

    // -------------------------------------------------------------------------
    // parse_verify_page
    // -------------------------------------------------------------------------

    fn full_page() -> String {
        r#"
<html><body>
<form method="post" action="AdvanceCheck.aspx?ReturnUrl=foo&amp;sid=BAR" id="form1">
<input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="VS_FULL" />
<input type="hidden" name="__VIEWSTATEGENERATOR" id="__VIEWSTATEGENERATOR" value="GEN_FULL" />
<input type="hidden" name="__EVENTVALIDATION" id="__EVENTVALIDATION" value="EV_FULL" />
<input type="hidden" name="LBD_VCID_c_logincheck_advancecheck_samplecaptcha" id="LBD_VCID_c_logincheck_advancecheck_samplecaptcha" value="VCID_FULL" />
<span id="lblAuthType">Email</span>
</form>
</body></html>
"#
        .to_string()
    }

    #[test]
    fn parse_verify_page_happy_extracts_every_field() {
        let info = parse_verify_page(&tw_client(), &full_page()).unwrap();
        assert_eq!(info.viewstate, "VS_FULL");
        assert_eq!(info.viewstate_generator.as_deref(), Some("GEN_FULL"));
        assert_eq!(info.event_validation, "EV_FULL");
        assert_eq!(info.samplecaptcha, "VCID_FULL");
        assert_eq!(info.lbl_auth_type, "Email");
        // Form action must have `&amp;` decoded back to `&` and be
        // prepended with the TW newlogin host (mirrors WPF
        // L800-802).
        assert_eq!(
            info.form_action,
            "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?ReturnUrl=foo&sid=BAR"
        );
    }

    #[test]
    fn parse_verify_page_missing_viewstate_is_typed_error() {
        let html = full_page().replace("VS_FULL", "");
        // After the replace, `__VIEWSTATE`'s value="" — extract_viewstate
        // matches `[^"]+` which requires ≥ 1 char, so it returns
        // ParserError::MissingViewState which we map to
        // VerifyMissingViewState.
        assert!(matches!(
            parse_verify_page(&tw_client(), &html).unwrap_err(),
            LoginError::VerifyMissingViewState
        ));
    }

    #[test]
    fn parse_verify_page_missing_event_validation_is_typed_error() {
        let html = full_page().replace(r#"value="EV_FULL""#, r#"value="""#);
        assert!(matches!(
            parse_verify_page(&tw_client(), &html).unwrap_err(),
            LoginError::VerifyMissingEventValidation
        ));
    }

    #[test]
    fn parse_verify_page_missing_samplecaptcha_is_typed_error() {
        let html = full_page().replace(r#"value="VCID_FULL""#, r#"value="""#);
        assert!(matches!(
            parse_verify_page(&tw_client(), &html).unwrap_err(),
            LoginError::VerifyMissingSampleCaptcha
        ));
    }

    #[test]
    fn parse_verify_page_missing_lbl_auth_type_is_typed_error() {
        let html = full_page().replace(
            r#"<span id="lblAuthType">Email</span>"#,
            r#"<span id="something_else">Email</span>"#,
        );
        assert!(matches!(
            parse_verify_page(&tw_client(), &html).unwrap_err(),
            LoginError::VerifyMissingLblAuthType
        ));
    }

    #[test]
    fn parse_verify_page_alert_short_circuits_with_server_message() {
        let html = full_page().replace("</form>", "</form><script>alert('帳號已被鎖定');</script>");
        match parse_verify_page(&tw_client(), &html).unwrap_err() {
            LoginError::ServerMessage(msg) => assert_eq!(msg, "帳號已被鎖定"),
            other => panic!("expected ServerMessage, got {other:?}"),
        }
    }

    #[test]
    fn parse_verify_page_no_form_action_falls_back_to_default_url() {
        let html = full_page().replace(
            r#"action="AdvanceCheck.aspx?ReturnUrl=foo&amp;sid=BAR""#,
            r#"action="SomethingElse.aspx""#,
        );
        let info = parse_verify_page(&tw_client(), &html).unwrap();
        assert_eq!(
            info.form_action,
            "https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx"
        );
    }

    // -------------------------------------------------------------------------
    // classify_verify_response
    // -------------------------------------------------------------------------

    #[test]
    fn classify_alert_with_success_keyword_is_success() {
        let body = "<script>alert('資料已驗證成功');</script>";
        assert_eq!(classify_verify_response(body), VerifyOutcome::Success);
    }

    #[test]
    fn classify_alert_with_other_keyword_is_server_message() {
        let body = "<script>alert('帳號已被鎖定，請聯絡客服');</script>";
        assert_eq!(
            classify_verify_response(body),
            VerifyOutcome::ServerMessage("帳號已被鎖定，請聯絡客服".to_string())
        );
    }

    #[test]
    fn classify_no_alert_with_wrong_captcha_text_is_wrong_captcha() {
        let body = "<html>圖形驗證碼輸入錯誤，請重新輸入</html>";
        assert_eq!(classify_verify_response(body), VerifyOutcome::WrongCaptcha);
    }

    #[test]
    fn classify_no_alert_no_wrong_captcha_text_is_wrong_auth_info() {
        let body = "<html>some other content</html>";
        assert_eq!(classify_verify_response(body), VerifyOutcome::WrongAuthInfo);
    }
}
