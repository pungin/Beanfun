//! Orchestrator for the **TOTP continuation** flow — consume a
//! [`TotpChallenge`] produced by `login_hk_regular` (or a future TW
//! TOTP producer) and post the user's 6-digit OTP to complete login.
//!
//! # Sequence
//!
//! (Line numbers reference `Beanfun/Tools/BeanfunClient.Login.cs::TotpLogin`.)
//!
//! 1. Reuse the three `__VIEWSTATE*` fields cached on the challenge
//!    (WPF re-parses them from the stashed `totpResponse`; we skip
//!    that round because the HK producer already validated them).
//! 2. Build the OTP POST payload in WPF order                       (L342-356)
//!    — `__EVENTTARGET`, `__EVENTARGUMENT`, `__VIEWSTATE`,
//!    `__VIEWSTATEGENERATOR`, **[`__VIEWSTATEENCRYPTED` if HK]**,
//!    `__EVENTVALIDATION`, `otpCode1..6`, `totpLoginBtn="登入"`.
//! 3. `POST challenge.totp_url` via the redirect-following client
//!    (WPF L358 `UploadString(loginHost, payload)`).
//! 4. Branch on the response — WPF precedence (L359-388):
//!    - body contains `RELOAD_CAPTCHA_CODE` + `alert`
//!      → [`LoginError::AdvanceCheckRequired`]
//!    - final URL carries `akey=…`
//!      → call [`login_completed`] to obtain the `bfWebToken`
//!    - else → `classify_missing_akey_body` splits MsgBox /
//!      pollRequest / Unrecognized to `ServerMessage` /
//!      `DeviceRegistrationRequired` / `MissingAkey`
//!      ([`super::hk_error`] module — `pub(super)` so plain backticks
//!      avoid public-docs-link-to-private warnings).
//!
//! # Region-conditional `__VIEWSTATEENCRYPTED`
//!
//! WPF L347-348 keys this field on `App.LoginRegion == "HK"`. We
//! read the same single-source-of-truth off the client's
//! `ClientConfig::region` (populated once per login session) so the
//! [`TotpChallenge`] itself stays region-agnostic. A future TW TOTP
//! producer only needs to populate the challenge; the region already
//! lives on the client, matching WPF's `App.LoginRegion` pattern.
//!
//! # Defensive viewstate checks
//!
//! The challenge-producing side (`login_hk_regular`) guarantees all
//! three `__VIEWSTATE*` fields are `Some` before it builds a
//! [`TotpChallenge`]. We still check here — in the order WPF does
//! (`__VIEWSTATE` → `__EVENTVALIDATION` → `__VIEWSTATEGENERATOR`,
//! L319-340) — so a future producer that forgets to validate cannot
//! ship a broken challenge past this boundary. The runtime cost is
//! three branch predicted reads.
//!
//! # Why we *don't* re-parse the viewstate here
//!
//! WPF stashes raw HTML on `this.totpResponse` and re-scrapes inside
//! `TotpLogin`. That's pure redundancy — the HK flow already scraped
//! the same page. We carry the parsed values on [`TotpChallenge`]
//! instead and save ~20 KB of allocated HTML + three regex runs per
//! OTP submission.
//!
//! # `service_code` / `service_region` plumbing
//!
//! WPF `TotpLogin(…, string service_code = "610074", string service_region = "T9")`
//! (L303-311) accepts service metadata as parameters, and the sole
//! call site — `MainWindow.xaml.cs` L1542-1551 — passes
//! `this.service_code` / `this.service_region` read off the same
//! `MainWindow` field the preceding `HkRegularLogin` call also read.
//!
//! We **capture those values on the challenge at
//! `login_hk_regular` time** and forward them verbatim to
//! `login_completed` here, instead of re-accepting them on the
//! `login_totp` signature. Observable behaviour is identical
//! because:
//!
//! 1. WPF's OTP UI is modal; the `MainWindow.service_code` value
//!    cannot change between the two calls.
//! 2. Our UI layer would otherwise have to re-thread app-config
//!    state across the async OTP prompt, adding state the Rust
//!    caller does not need.
//!
//! The trade-off is a single additional pair of `String` fields on
//! `TotpChallenge`; see the `TotpChallenge` module docs for the
//! full rationale.

use reqwest::header;

use crate::core::parser::{extract_akey, HiddenInput};
use crate::services::beanfun::{
    login::{
        completed::login_completed,
        ensure_success,
        hk_error::{classify_missing_akey_body, is_advance_check},
        totp_challenge::TotpChallenge,
    },
    BeanfunClient, LoginError, LoginRegion, Session,
};

/// Run the TOTP continuation: post the six OTP digits against
/// `challenge.totp_url` and finalise the session when the server
/// accepts them.
///
/// # Parameters
///
/// - `client` — already-region-configured [`BeanfunClient`] (same
///   instance that produced `challenge`). We require the same client
///   because the challenge's cookie store carries the ASP.NET
///   session cookies that the server binds the OTP submission to.
/// - `challenge` — the continuation handed back through
///   [`LoginError::TotpRequired`].
/// - `otp1..otp6` — the six OTP digits, each as a `&str`. WPF's
///   `TotpLogin(string, string, string, string, string, string, …)`
///   takes them individually (L303-309); we match the shape 1:1
///   because it maps cleanly onto the on-wire `otpCode1..6` fields
///   without any slice-index ambiguity at call sites.
///
/// # Error surface
///
/// - [`LoginError::AdvanceCheckRequired`] — server flipped into the
///   captcha / advance-check flow (WPF L359-362).
/// - [`LoginError::ServerMessage`] — server rendered a MsgBox error
///   body (WPF L368-376 via `classify_missing_akey_body`).
/// - [`LoginError::DeviceRegistrationRequired`] — server rendered a
///   `pollRequest(…)` triplet (WPF L378-388); caller drives the
///   mobile-app auto-login polling loop via
///   [`login_registered_device`](super::registered_device::login_registered_device).
/// - [`LoginError::MissingAkey`] — final redirect URL had no
///   `akey=…` and the body held neither error pattern (WPF L368
///   default).
/// - [`LoginError::MissingViewStateGenerator`] /
///   [`LoginError::MissingEventValidation`] — defensive guards
///   against a malformed challenge (WPF's strict `if (!IsMatch)`
///   returns at L328 / L335).
/// - Any [`LoginError`] that [`login_completed`] can surface
///   bubbles up unchanged.
//
// `too_many_arguments` is allowed here on purpose: we mirror WPF's
// `TotpLogin(string otp1..6, ...)` 1:1 so the rename/reorder of any
// parameter is immediately visible against the legacy reference. A
// wrapped `[&str; 6]` parameter would technically fit clippy's
// threshold but would hide the positional mapping at call sites —
// `login_totp(client, ch, "1", "2", "3", "4", "5", "6")` is clearer
// than `login_totp(client, ch, ["1","2","3","4","5","6"])` when
// cross-referencing the WPF signature.
#[allow(clippy::too_many_arguments)]
pub async fn login_totp(
    client: &BeanfunClient,
    challenge: &TotpChallenge,
    otp1: &str,
    otp2: &str,
    otp3: &str,
    otp4: &str,
    otp5: &str,
    otp6: &str,
) -> Result<Session, LoginError> {
    // Defensive unwrap of the two Option-typed viewstate fields.
    // WPF checks in the order __VIEWSTATE → __EVENTVALIDATION →
    // __VIEWSTATEGENERATOR (L319-340); we match that so the first
    // variant surfaced is the one WPF would have surfaced too.
    //
    // `__VIEWSTATE` itself is a non-optional String on ViewStateForm,
    // so the earliest check — WPF's L319-325 — is already enforced
    // statically by the type.
    let event_validation = challenge
        .viewstate
        .event_validation
        .as_deref()
        .ok_or(LoginError::MissingEventValidation)?;
    let generator = challenge
        .viewstate
        .viewstate_generator
        .as_deref()
        .ok_or(LoginError::MissingViewStateGenerator)?;

    let region = client.config().region;
    let form = build_totp_form(
        &challenge.viewstate.viewstate,
        generator,
        event_validation,
        region,
        [otp1, otp2, otp3, otp4, otp5, otp6],
    );

    let (final_url, body) = post_totp(client, challenge.totp_url.clone(), &form).await?;

    if is_advance_check(&body) {
        return Err(LoginError::AdvanceCheckRequired { url: None });
    }

    match extract_akey(final_url.as_str()) {
        Ok(akey) => {
            // Service metadata rides on the challenge (captured at
            // `login_hk_regular` call time) rather than being passed
            // through `login_totp`. WPF re-reads `this.service_code`
            // at TotpLogin time from the same `MainWindow` instance
            // field it read at HkRegularLogin time; since the OTP UI
            // prompt blocks any mutation in between, capture-at-HK
            // is equivalent to capture-at-TOTP in observable
            // behaviour. See `TotpChallenge` module docs for the
            // full argument.
            login_completed(
                client,
                &challenge.session_key,
                &akey,
                &challenge.account_id,
                &challenge.service_code,
                &challenge.service_region,
            )
            .await
        }
        Err(_) => Err(classify_missing_akey_body(&body)),
    }
}

// -----------------------------------------------------------------------------
// Helpers — pure, covered by unit tests below
// -----------------------------------------------------------------------------

/// Construct the TOTP POST payload.
///
/// Field order matches WPF `TotpLogin` L343-356 exactly. The
/// `__VIEWSTATEENCRYPTED` slot is conditionally emitted only for
/// [`LoginRegion::HK`] — WPF L347-348 gates the field on
/// `App.LoginRegion == "HK"`, and we preserve that asymmetry because
/// a spurious empty field on the TW wire would be a silent divergence
/// the server could (in principle) sniff.
///
/// Two parameterisation choices worth noting:
///
/// - `otps: [&str; 6]` is chosen over a `&[&str]` slice so the type
///   system rejects anything other than exactly six codes at compile
///   time — on par with WPF's fixed-arity signature and without the
///   runtime length check a slice would demand.
/// - The returned `Vec<HiddenInput>` preserves insertion order when
///   reqwest serialises it to `application/x-www-form-urlencoded`, so
///   order-sensitive servers (we haven't seen one, but WPF matches
///   anyway) see a byte-compatible request body.
fn build_totp_form(
    viewstate: &str,
    viewstate_generator: &str,
    event_validation: &str,
    region: LoginRegion,
    otps: [&str; 6],
) -> Vec<HiddenInput> {
    // Capacity of 13 is exactly the HK size; TW drops one field so
    // the `Vec` may end up one short of capacity — negligible
    // over-allocation, keeps the branch simple.
    let mut form: Vec<HiddenInput> = Vec::with_capacity(13);
    form.push(("__EVENTTARGET".into(), String::new()));
    form.push(("__EVENTARGUMENT".into(), String::new()));
    form.push(("__VIEWSTATE".into(), viewstate.to_owned()));
    form.push((
        "__VIEWSTATEGENERATOR".into(),
        viewstate_generator.to_owned(),
    ));
    // WPF L347-348 — HK-only. TW TOTP (legacy non-AJAX path) omits
    // this field entirely, and ASP.NET treats its presence vs
    // absence as distinct.
    if matches!(region, LoginRegion::HK) {
        form.push(("__VIEWSTATEENCRYPTED".into(), String::new()));
    }
    form.push(("__EVENTVALIDATION".into(), event_validation.to_owned()));
    form.push(("otpCode1".into(), otps[0].to_owned()));
    form.push(("otpCode2".into(), otps[1].to_owned()));
    form.push(("otpCode3".into(), otps[2].to_owned()));
    form.push(("otpCode4".into(), otps[3].to_owned()));
    form.push(("otpCode5".into(), otps[4].to_owned()));
    form.push(("otpCode6".into(), otps[5].to_owned()));
    // `登入` — literal Traditional Chinese "Login" button label.
    // ASP.NET sometimes validates the submit button value server
    // side, so we match WPF L356 byte-for-byte.
    form.push(("totpLoginBtn".into(), "登入".into()));
    form
}

// -----------------------------------------------------------------------------
// HTTP helpers
// -----------------------------------------------------------------------------

/// POST the OTP form and return `(final_url, body)` — same shape as
/// `hk_regular::post_credentials`.
///
/// Uses the redirect-following client because WPF's default
/// `WebClient.UploadString` follows redirects; the `akey=…` value
/// (on success) ends up on the final URL, not the 302's `Location`.
///
/// Deliberate header divergence from WPF: `TotpLogin` never calls
/// `SetBaseHeaders`, so its POST ships without an explicit `Referer`.
/// We add one matching the TOTP URL for the same reasons we do on
/// HK Regular (`hk_regular::post_credentials` docs): browser-aligned,
/// same-origin, and can only be a superset of WPF's accepted shape.
async fn post_totp(
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
    ensure_success(&resp, "TOTP POST")?;

    let final_url = resp.url().clone();
    let body = client.bounded_text(resp).await?;
    Ok((final_url, body))
}

// -----------------------------------------------------------------------------
// Unit tests — pure helpers only. End-to-end coverage lives in
// `tests/totp_login.rs` where we can drive the flow through wiremock.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // build_totp_form — HK vs TW wire shape
    // -------------------------------------------------------------------------

    #[test]
    fn hk_form_has_thirteen_fields_in_wpf_order() {
        let form = build_totp_form(
            "VS",
            "GEN",
            "EV",
            LoginRegion::HK,
            ["1", "2", "3", "4", "5", "6"],
        );
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
                "otpCode1",
                "otpCode2",
                "otpCode3",
                "otpCode4",
                "otpCode5",
                "otpCode6",
                "totpLoginBtn",
            ],
            "HK TOTP field order must match WPF L343-356 (with L347-348 \
             __VIEWSTATEENCRYPTED for HK)"
        );
    }

    #[test]
    fn tw_form_drops_viewstateencrypted() {
        let form = build_totp_form(
            "VS",
            "GEN",
            "EV",
            LoginRegion::TW,
            ["1", "2", "3", "4", "5", "6"],
        );
        let keys: Vec<&str> = form.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            !keys.contains(&"__VIEWSTATEENCRYPTED"),
            "TW TOTP must NOT emit __VIEWSTATEENCRYPTED (WPF L347-348 \
             gates on App.LoginRegion == \"HK\"): {keys:?}"
        );
        assert_eq!(
            keys.len(),
            12,
            "TW TOTP expects 12 fields (HK's 13 minus __VIEWSTATEENCRYPTED)"
        );
    }

    #[test]
    fn form_fills_required_values() {
        let form = build_totp_form(
            "VS_VAL",
            "GEN_VAL",
            "EV_VAL",
            LoginRegion::HK,
            ["111111", "222222", "333333", "444444", "555555", "666666"],
        );
        let by_key: std::collections::HashMap<_, _> =
            form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(by_key.get("__VIEWSTATE"), Some(&"VS_VAL"));
        assert_eq!(by_key.get("__VIEWSTATEGENERATOR"), Some(&"GEN_VAL"));
        assert_eq!(by_key.get("__EVENTVALIDATION"), Some(&"EV_VAL"));
        assert_eq!(by_key.get("otpCode1"), Some(&"111111"));
        assert_eq!(by_key.get("otpCode6"), Some(&"666666"));
        assert_eq!(by_key.get("totpLoginBtn"), Some(&"登入"));
        // Hard-coded empties from WPF:
        assert_eq!(by_key.get("__EVENTTARGET"), Some(&""));
        assert_eq!(by_key.get("__EVENTARGUMENT"), Some(&""));
        assert_eq!(by_key.get("__VIEWSTATEENCRYPTED"), Some(&""));
    }

    #[test]
    fn otp_positional_mapping_is_preserved() {
        // Regression guard: the array index → field name mapping
        // must stay [0]→otpCode1 .. [5]→otpCode6. Swap any two and
        // the server will reject with "wrong OTP" which is
        // notoriously hard to diagnose at the integration layer.
        let form = build_totp_form(
            "VS",
            "GEN",
            "EV",
            LoginRegion::HK,
            ["A", "B", "C", "D", "E", "F"],
        );
        let otp_fields: Vec<(&str, &str)> = form
            .iter()
            .filter(|(k, _)| k.starts_with("otpCode"))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            otp_fields,
            vec![
                ("otpCode1", "A"),
                ("otpCode2", "B"),
                ("otpCode3", "C"),
                ("otpCode4", "D"),
                ("otpCode5", "E"),
                ("otpCode6", "F"),
            ],
            "otpCode1..6 must map to otps[0..6] in order"
        );
    }
}
