//! Orchestrator for the **HK Regular** login flow — `account + password`
//! against the Hong Kong portal.
//!
//! # Sequence
//!
//! (Line numbers reference `Beanfun/Tools/BeanfunClient.Login.cs::HkRegularLogin`.)
//!
//! 1. `GET /` → session key via `get_session_key`                 (L207 `otp1={skey}`)
//! 2. `GET login/id-pass_form_newBF.aspx?otp1={skey}`             (L208)
//!    → scrape the three `__VIEWSTATE*` fields (L210-232).
//! 3. `POST` the same URL with 9 form fields                      (L234-245)
//! 4. Branch on the response — in **WPF precedence order**
//!    (L247-285):
//!    - body contains `RELOAD_CAPTCHA_CODE` + `alert`
//!      → [`LoginError::AdvanceCheckRequired`] (url `None`)
//!    - body contains `totpLoginBtn`
//!      → [`LoginError::TotpRequired`] carrying a [`TotpChallenge`]
//!    - final URL's query carries `akey=…`
//!      → call [`login_completed`] to obtain the `bfWebToken`
//!    - else → classify via `classify_missing_akey_body`:
//!      - `MsgBox` → [`LoginError::ServerMessage`] with the inner text
//!      - `PollRequest` → [`LoginError::DeviceRegistrationRequired`]
//!        preserving `login_token`, `poll_url`, and `param`; the
//!        caller is expected to drive the mobile-app auto-login poll
//!        via [`login_registered_device`](super::registered_device::login_registered_device)
//!      - `Unrecognized` → [`LoginError::MissingAkey`] (= WPF's
//!        `errmsg = "LoginNoAkey"` default on L264)
//!
//! # Intentional divergences from WPF
//!
//! - **Loose `__VIEWSTATE*` regex.** WPF uses three strict patterns
//!   (`id="__X" value="(.*)" />`). Our [`extract_viewstate`] uses the
//!   looser `id="__X"[^>]+value="([^"]+)"` shape, which is a strict
//!   *superset* of WPF's — every HTML WPF accepts, we accept, plus
//!   future ASP.NET renderings that reorder attributes. The HK flow
//!   still requires **all three** fields to be `Some`; we return the
//!   typed `MissingViewStateGenerator` / `MissingEventValidation`
//!   errors instead of the WPF string constants, preserving the same
//!   user-visible outcome.
//! - **Body-size cap (16 MiB).** WPF's `WebClient.DownloadString` is
//!   unbounded; a hostile server could OOM the client. Our
//!   [`BeanfunClient::bounded_text`] caps reads and surfaces
//!   [`LoginError::BodyTooLarge`]. In practice the HK page is ~20 KB.
//!
//! # Why we capture `resp.url()` *before* draining the body
//!
//! `bounded_text` consumes the response by value (it must, to stream
//! chunks without OOM). The final URL after redirects is only
//! available while the `Response` still exists, so we clone it first.

use reqwest::header;

use crate::core::parser::{extract_akey, extract_viewstate, HiddenInput, ParserError};
use crate::services::beanfun::{
    login::{
        completed::login_completed,
        ensure_success,
        hk_error::{classify_missing_akey_body, is_advance_check},
        session_key::get_session_key,
        totp_challenge::TotpChallenge,
    },
    BeanfunClient, Credentials, LoginError, LoginRegion, Session,
};

/// Run the full HK Regular login flow.
///
/// On success returns a [`Session`] ready for downstream service
/// calls. The `Err` channel surfaces **continuations** as well as
/// actual failures — notably [`LoginError::TotpRequired`] for 2FA
/// accounts; the caller is expected to match on that and dispatch
/// into `login_totp`.
///
/// # Service metadata parameters
///
/// `service_code` / `service_region` mirror the same-named parameters
/// on WPF's `HkRegularLogin` (L191-195, defaults `"610074"` / `"T9"`
/// = new MapleStory). The caller is expected to pipe through
/// whatever value `MainWindow.service_code` / `MainWindow.service_region`
/// would hold — i.e. the user's last-played game, loaded from config
/// at startup (see `Beanfun/MainWindow.xaml.cs` L72-73, L357-358).
///
/// The values are **not** sent on the wire (WPF `LoginCompleted`
/// L853-856 hardcodes blank strings for both fields). They populate
/// the returned `Session.service_code` / `Session.service_region`
/// so downstream P4 account lookups can target the right game slot.
///
/// On the TOTP branch the values are captured into the
/// [`TotpChallenge`] so `login_totp` can forward them when the OTP
/// round-trip succeeds — matching WPF's behaviour of reading
/// `this.service_code` at the TotpLogin site too, without forcing
/// the UI layer to re-thread config state across an async wait.
///
/// Preconditions:
/// - `client.config().region` must be [`LoginRegion::HK`]. Other
///   regions are a programming error and `debug_assert`-ed; release
///   builds would still work but hit the wrong endpoints.
pub async fn login_hk_regular(
    client: &BeanfunClient,
    creds: &Credentials,
    service_code: &str,
    service_region: &str,
) -> Result<Session, LoginError> {
    debug_assert_eq!(
        client.config().region,
        LoginRegion::HK,
        "login_hk_regular requires an HK-configured BeanfunClient"
    );

    // Step 1 — portal session key (WPF L207 `{skey}` via `GetSessionkey`).
    let skey = get_session_key(client).await?;

    // Step 2 — GET the HK login page and scrape the viewstate triad.
    let login_url = build_hk_login_url(client, &skey)?;
    let html = fetch_login_page(client, login_url.clone()).await?;

    // WPF treats all three fields as required (L210-232, three
    // separate `if (!regex.IsMatch(response))` guards). Our
    // `extract_viewstate` leaves generator/validation Option-typed
    // because TW callers don't need them; we enforce HK's stricter
    // contract here.
    //
    // The three checks must keep WPF's ordering
    // (`__VIEWSTATE` → `__EVENTVALIDATION` → `__VIEWSTATEGENERATOR`)
    // so that when more than one field is simultaneously absent we
    // surface the exact same variant WPF would have returned first.
    //
    // `MissingViewState` is flattened onto a dedicated `LoginError`
    // variant rather than left wrapped in `LoginError::Parser(...)`,
    // so callers pattern-match on a single shape regardless of which
    // flow produced the error — this mirrors the other two required
    // fields below, which are flattened by construction.
    let viewstate = extract_viewstate(&html).map_err(|e| match e {
        ParserError::MissingViewState => LoginError::MissingViewState,
        other => LoginError::Parser(other),
    })?;
    let event_validation = viewstate
        .event_validation
        .clone()
        .ok_or(LoginError::MissingEventValidation)?;
    let generator = viewstate
        .viewstate_generator
        .clone()
        .ok_or(LoginError::MissingViewStateGenerator)?;

    // Step 3 — POST credentials against the same URL (WPF L234-245).
    let form = build_credentials_form(&viewstate.viewstate, &generator, &event_validation, creds);
    let (final_url, body) = post_credentials(client, login_url.clone(), &form).await?;

    // Step 4 — branch on the response (WPF L247-285).
    // WPF order must be preserved: RELOAD first (cheapest check that
    // wins over the TOTP form, since a replayed advance-check page
    // can technically contain both markers), then TOTP, then akey,
    // then the error-body fallback.
    if is_advance_check(&body) {
        return Err(LoginError::AdvanceCheckRequired { url: None });
    }
    if is_totp_required(&body) {
        // Consume `viewstate` into the challenge (we no longer need
        // the local copies of `generator` / `event_validation` after
        // this point, and the challenge type owns all three fields
        // pre-parsed so TOTP can reuse them without another scrape).
        //
        // `service_code` / `service_region` are captured here too so
        // `login_totp` can forward them to `login_completed` without
        // the UI layer having to re-thread app-config state through
        // the OTP prompt. See `TotpChallenge` module docs for the
        // WPF-equivalence argument.
        let challenge = TotpChallenge {
            totp_url: login_url,
            viewstate,
            session_key: skey,
            account_id: creds.account.clone(),
            service_code: service_code.to_owned(),
            service_region: service_region.to_owned(),
        };
        return Err(LoginError::TotpRequired(Box::new(challenge)));
    }

    // Success path — the server-side redirect chain left us on a URL
    // whose query carries `akey=…`. WPF L286 returns that akey
    // verbatim (greedy up to end-of-line) and the upper layer passes
    // it into `LoginCompleted`. We do the same.
    match extract_akey(final_url.as_str()) {
        Ok(akey) => {
            login_completed(
                client,
                &skey,
                &akey,
                &creds.account,
                service_code,
                service_region,
            )
            .await
        }
        Err(_) => Err(classify_missing_akey_body(&body)),
    }
}

// -----------------------------------------------------------------------------
// Helpers — pure, covered by unit tests below
// -----------------------------------------------------------------------------

/// Build `https://{login_host}/login/id-pass_form_newBF.aspx?otp1={skey}`
/// on top of `client.config().endpoints.login_base`.
///
/// We append `otp1` via `query_pairs_mut().append_pair` so the URL
/// crate handles percent-encoding for us. The `pSKey` helper on
/// `BeanfunClient` uses a different parameter name (`pSKey`), so we
/// build HK's URL inline here instead of adding a one-off helper.
fn build_hk_login_url(client: &BeanfunClient, skey: &str) -> Result<url::Url, LoginError> {
    let mut url = client.login_url("login/id-pass_form_newBF.aspx")?;
    url.query_pairs_mut().append_pair("otp1", skey);
    Ok(url)
}

/// Construct the 9-field credentials POST payload.
///
/// Order matches WPF L235-243 exactly — not because ASP.NET cares
/// about the field order, but because matching the reference makes
/// wire-level diffs between Rust and legacy WPF clients easier to
/// read. The `Arc<str>`-cloning inside is negligible; form bodies
/// are tiny.
fn build_credentials_form(
    viewstate: &str,
    viewstate_generator: &str,
    event_validation: &str,
    creds: &Credentials,
) -> Vec<HiddenInput> {
    vec![
        ("__EVENTTARGET".into(), String::new()),
        ("__EVENTARGUMENT".into(), String::new()),
        ("__VIEWSTATE".into(), viewstate.to_owned()),
        (
            "__VIEWSTATEGENERATOR".into(),
            viewstate_generator.to_owned(),
        ),
        // WPF L239 hard-codes an empty value here. Some ASP.NET
        // deployments gate login on the *presence* of this field
        // (even when empty), so we always send it.
        ("__VIEWSTATEENCRYPTED".into(), String::new()),
        ("__EVENTVALIDATION".into(), event_validation.to_owned()),
        ("t_AccountID".into(), creds.account.clone()),
        ("t_Password".into(), creds.password.clone()),
        // `登入` — literal Traditional Chinese "Login" button label.
        // ASP.NET sometimes validates the submit button value server
        // side, so we match WPF's literal (L243) byte-for-byte.
        ("btn_login".into(), "登入".into()),
    ]
}

/// WPF L253-256 — a `totpLoginBtn` element means the account has
/// TOTP enabled. The page embeds the same viewstate triad we already
/// scraped, so we can serve TOTP off-hand without re-fetching.
///
/// Stays local to this module: `is_totp_required` is only relevant
/// on the HK Regular response (the TOTP POST itself cannot "need
/// TOTP" again), so it does not belong in `hk_error.rs`.
fn is_totp_required(body: &str) -> bool {
    body.contains("totpLoginBtn")
}

// -----------------------------------------------------------------------------
// HTTP helpers — small wrappers around reqwest, kept private to this module
// -----------------------------------------------------------------------------

/// GET the HK login page and return the response body. Uses the
/// redirect-following client so any interstitial 30x hops are
/// resolved silently.
async fn fetch_login_page(client: &BeanfunClient, url: url::Url) -> Result<String, LoginError> {
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "HK login page GET")?;
    client.bounded_text(resp).await
}

/// POST credentials and return `(final_url, body)`.
///
/// Uses the redirect-following client because WPF's default
/// `WebClient.UploadValues` follows redirects; the `akey=…` value
/// ends up on the *final* URL, not the 302's `Location` header. We
/// grab `resp.url().clone()` before draining the body because
/// [`BeanfunClient::bounded_text`] consumes the response by value.
///
/// Deliberate header divergence from WPF: WPF's `HkRegularLogin`
/// never calls `SetBaseHeaders`, so its POST ships without an
/// explicit `Referer`. We add one matching the login page URL —
/// i.e. the same URL the browser was on when it "submitted" the
/// form. This aligns with real-browser behaviour and with the
/// Referer discipline WPF already applies on the TW flow
/// (`BeanfunClient.Login.cs` L43/L57/L110/L153), and because the
/// value we send is same-origin with the target endpoint it cannot
/// be rejected by a server that also accepts WPF's no-Referer POST.
async fn post_credentials(
    client: &BeanfunClient,
    url: url::Url,
    form: &[HiddenInput],
) -> Result<(url::Url, String), LoginError> {
    let referer = url.as_str().to_owned();

    let resp = client
        .http()
        .post(url)
        .header(header::REFERER, referer)
        .form(form)
        .send()
        .await?;
    ensure_success(&resp, "HK credentials POST")?;

    let final_url = resp.url().clone();
    let body = client.bounded_text(resp).await?;
    Ok((final_url, body))
}

// -----------------------------------------------------------------------------
// Unit tests — pure helpers only. End-to-end coverage lives in
// `tests/hk_login.rs` where we can drive the flow through wiremock.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::beanfun::{ClientConfig, Endpoints};

    fn hk_client() -> BeanfunClient {
        let cfg = ClientConfig {
            region: LoginRegion::HK,
            endpoints: Endpoints::hk(),
            ..ClientConfig::default()
        };
        BeanfunClient::new(cfg).expect("HK client must build")
    }

    // -------------------------------------------------------------------------
    // build_hk_login_url
    // -------------------------------------------------------------------------

    #[test]
    fn build_hk_login_url_appends_otp1_parameter() {
        let url = build_hk_login_url(&hk_client(), "SKEY_VAL").unwrap();
        assert_eq!(
            url.as_str(),
            "https://login.hk.beanfun.com/login/id-pass_form_newBF.aspx?otp1=SKEY_VAL"
        );
    }

    #[test]
    fn build_hk_login_url_percent_encodes_skey() {
        // `pSKey`-style random session keys never contain reserved
        // chars in practice, but the URL crate must still encode any
        // that slip through (e.g. `/` `=` `&`). Lock that in as a
        // regression guard.
        let url = build_hk_login_url(&hk_client(), "A B/C=D&E").unwrap();
        let q = url.query().expect("query must be present");
        assert!(q.starts_with("otp1="), "query should start with otp1=: {q}");
        assert!(!q.contains(' '), "space must be encoded: {q}");
        assert!(!q.contains('&'), "trailing & must be encoded: {q}");
    }

    // -------------------------------------------------------------------------
    // build_credentials_form
    // -------------------------------------------------------------------------

    #[test]
    fn credentials_form_has_nine_fields_in_wpf_order() {
        let creds = Credentials::new("alice", "hunter2");
        let form = build_credentials_form("VS", "GEN", "EV", &creds);
        let keys: Vec<&str> = form.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "__EVENTTARGET",
                "__EVENTARGUMENT",
                "__VIEWSTATE",
                "__VIEWSTATEGENERATOR",
                "__VIEWSTATEENCRYPTED",
                "__EVENTVALIDATION",
                "t_AccountID",
                "t_Password",
                "btn_login",
            ],
            "field order must match WPF L235-243 for wire-compatibility"
        );
    }

    #[test]
    fn credentials_form_fills_required_values() {
        let creds = Credentials::new("alice", "hunter2");
        let form = build_credentials_form("VS", "GEN", "EV", &creds);
        let by_key: std::collections::HashMap<_, _> =
            form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(by_key.get("__VIEWSTATE"), Some(&"VS"));
        assert_eq!(by_key.get("__VIEWSTATEGENERATOR"), Some(&"GEN"));
        assert_eq!(by_key.get("__EVENTVALIDATION"), Some(&"EV"));
        assert_eq!(by_key.get("t_AccountID"), Some(&"alice"));
        assert_eq!(by_key.get("t_Password"), Some(&"hunter2"));
        assert_eq!(by_key.get("btn_login"), Some(&"登入"));
        // WPF always hard-codes empty strings for these three:
        assert_eq!(by_key.get("__EVENTTARGET"), Some(&""));
        assert_eq!(by_key.get("__EVENTARGUMENT"), Some(&""));
        assert_eq!(by_key.get("__VIEWSTATEENCRYPTED"), Some(&""));
    }

    // -------------------------------------------------------------------------
    // is_totp_required (local predicate — HK Regular only)
    // -------------------------------------------------------------------------

    #[test]
    fn totp_required_detected_by_button_marker() {
        assert!(is_totp_required(
            r#"<input type="submit" id="totpLoginBtn" value="登入">"#
        ));
        assert!(!is_totp_required("<form><input type=\"submit\"/></form>"));
    }
}
