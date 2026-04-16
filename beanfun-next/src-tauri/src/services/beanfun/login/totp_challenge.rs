//! [`TotpChallenge`] — opaque handle carrying the server-side state
//! needed to complete a TOTP login, handed from `login_hk_regular` to
//! `login_totp`.
//!
//! # Why a dedicated type
//!
//! WPF stashes the entire raw TOTP HTML on the `BeanfunClient`
//! instance (`this.totpResponse` + `this.totpUrl`, L255-256) and
//! re-extracts the three `__VIEWSTATE*` fields inside `TotpLogin`
//! (L319-340). That works because WPF has a single mutable client
//! owning the whole login lifecycle.
//!
//! For our async port we surface the continuation state as a typed
//! value instead of mutable client state, for three reasons:
//!
//! 1. **No hidden `BeanfunClient` invariants.** The challenge is a
//!    plain value — you cannot call `login_totp` without having a
//!    `LoginError::TotpRequired` to pattern-match on, so there's no
//!    "did you remember to call HK first?" footgun.
//! 2. **Smaller footprint.** We cache only the three already-parsed
//!    viewstate fields (~a few KB at most) rather than the raw
//!    ~dozens-of-KB HTML body. WPF's POST payload only uses those
//!    three fields anyway, so the wire behaviour is identical.
//! 3. **Debug safety.** Raw HTML can contain user-identifying data
//!    (account id in hidden fields, CSP nonces, etc.). Storing only
//!    the parsed triad + a redacted Debug impl keeps tracing output
//!    from leaking session secrets.
//!
//! # What lives inside
//!
//! - `totp_url` — the exact URL the TOTP POST must target. For HK
//!   Regular this is the same URL as the credential POST, scraped
//!   from the live request so test / alternate hosts Just Work.
//! - `viewstate` — the three `__VIEWSTATE*` fields extracted from
//!   the HK response, carried forward so `login_totp` can build the
//!   OTP POST payload without a second HTTP round-trip.
//! - `session_key` — the `pSKey` the HK flow obtained from
//!   `get_session_key`. Needed downstream by `login_completed` to
//!   populate the final `Session.skey`.
//! - `account_id` — the login id the user supplied; propagated onto
//!   `Session.account_id` for UI purposes.
//! - `service_code` / `service_region` — the MapleStory (or other
//!   Beanfun-hosted game) service metadata captured at
//!   `login_hk_regular` call time. WPF reads these off the mutable
//!   `MainWindow.service_code` instance field at *both*
//!   `HkRegularLogin` and `TotpLogin` sites (see
//!   `Beanfun/MainWindow.xaml.cs` L72-73, L357-358, L1542-1551).
//!   In practice the UI blocks on the OTP prompt, so the values
//!   cannot change between the two calls — we capture once here and
//!   forward them unchanged to `login_completed`. The behaviour is
//!   WPF-equivalent; the shape diverges by storing the values on the
//!   challenge instead of re-accepting them on `login_totp`, which
//!   keeps the TOTP caller from having to re-thread app-config state
//!   through an async UI await.
//!
//! Only the URL and account id are visible in Debug output; the
//! session key and viewstate contents are redacted. Service metadata
//! is not secret — the game catalog is public — but we still omit it
//! from Debug to keep the output focused on what a developer needs
//! to diagnose auth issues.
//!
//! Cross-module links are intentionally rendered as plain backticks
//! rather than rustdoc intra-doc links. Most referents (`login_totp`,
//! `login_completed`, `login_hk_regular`) are reachable now, but
//! keeping the prose plain avoids module-path churn when we rearrange
//! the login tree in later chunks (e.g. 3.3.4 may split `hk_regular`
//! or introduce a TW TOTP producer).

use std::fmt;

use url::Url;

use crate::core::parser::ViewStateForm;

/// Opaque continuation handle for a TOTP challenge.
///
/// Constructed internally by `login_hk_regular` when the HK response
/// body contains a `totpLoginBtn` form; consumed by `login_totp`.
/// All fields are crate-private so evolving the struct (e.g. adding a
/// service-code field later) is a non-breaking change.
///
/// `#[allow(dead_code)]` covers the fields that are only consumed by
/// the TOTP orchestrator (chunk 3.3.3) — they're written here but not
/// read anywhere yet.
#[derive(Clone)]
#[allow(dead_code)]
pub struct TotpChallenge {
    /// URL the TOTP code POST must target. For the HK Regular flow
    /// this is the same `id-pass_form_newBF.aspx?otp1=…` URL that was
    /// just used for credentials (WPF L256 `this.totpUrl = url`).
    pub(crate) totp_url: Url,
    /// Pre-parsed `__VIEWSTATE*` fields from the HK response body.
    /// All three must be present; `login_hk_regular` validates that
    /// before constructing the challenge, so TOTP can rely on
    /// [`ViewStateForm::viewstate_generator`] / `event_validation`
    /// being `Some`.
    pub(crate) viewstate: ViewStateForm,
    /// Session key minted by the portal entry page. Forwarded to
    /// `login_completed` so the final `Session.skey` matches what the
    /// HK flow started with.
    pub(crate) session_key: String,
    /// Login id the user authenticated as. Stored so the downstream
    /// `Session.account_id` field stays populated without forcing the
    /// caller to re-supply it.
    pub(crate) account_id: String,
    /// Beanfun service code — the MapleStory "game slot" id the
    /// Session will be bound to. Captured at `login_hk_regular`
    /// call time and forwarded verbatim to `login_completed` after
    /// the OTP exchange; mirrors WPF's `this.service_code` which is
    /// also read at TOTP time from the same persistent field.
    pub(crate) service_code: String,
    /// Beanfun service region — companion to `service_code`. Same
    /// capture-and-forward contract as above.
    pub(crate) service_region: String,
}

impl TotpChallenge {
    /// The URL the TOTP POST will target. Exposed read-only so UIs
    /// can show "about to authenticate against foo.bar" diagnostics
    /// without round-tripping through `login_totp`.
    pub fn totp_url(&self) -> &Url {
        &self.totp_url
    }

    /// The account id the challenge is bound to — safe to display in
    /// the OTP prompt UI.
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
}

impl fmt::Debug for TotpChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Redact the two fields that can leak secrets:
        // - `session_key` is the `pSKey` (session bearer equivalent
        //   for the short login window).
        // - `viewstate` contents include Base64-encoded ASP.NET
        //   server-side state that can hold user-identifying data.
        // Service metadata is public (anyone reading a game URL can
        // see it) but we keep Debug narrow to the three fields a
        // developer usually wants at a glance.
        f.debug_struct("TotpChallenge")
            .field("totp_url", &self.totp_url.as_str())
            .field("viewstate", &"***")
            .field("session_key", &"***")
            .field("account_id", &self.account_id)
            .field("service_code", &self.service_code)
            .field("service_region", &self.service_region)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_challenge() -> TotpChallenge {
        TotpChallenge {
            totp_url: Url::parse(
                "https://login.hk.beanfun.com/login/id-pass_form_newBF.aspx?otp1=SKEY",
            )
            .unwrap(),
            viewstate: ViewStateForm {
                viewstate: "VS_SECRET_BASE64".into(),
                viewstate_generator: Some("GEN_SECRET".into()),
                event_validation: Some("EV_SECRET".into()),
            },
            session_key: "SKEY_PLAINTEXT".into(),
            account_id: "alice".into(),
            service_code: "610074".into(),
            service_region: "T9".into(),
        }
    }

    #[test]
    fn debug_redacts_session_key_and_viewstate() {
        let rendered = format!("{:?}", sample_challenge());
        // Non-secret fields must stay visible — the whole point of
        // Debug is to help diagnose "which account / which URL" bugs.
        assert!(
            rendered.contains("alice"),
            "account_id should be visible in Debug: {rendered}"
        );
        assert!(
            rendered.contains("id-pass_form_newBF.aspx"),
            "totp_url should be visible in Debug: {rendered}"
        );
        // Secret fields must be redacted.
        assert!(
            !rendered.contains("SKEY_PLAINTEXT"),
            "session_key must be redacted: {rendered}"
        );
        assert!(
            !rendered.contains("VS_SECRET_BASE64"),
            "viewstate contents must be redacted: {rendered}"
        );
        assert!(
            !rendered.contains("GEN_SECRET"),
            "viewstate generator must be redacted: {rendered}"
        );
        assert!(
            !rendered.contains("EV_SECRET"),
            "event validation must be redacted: {rendered}"
        );
        assert!(
            rendered.contains("***"),
            "Debug must mark secrets as redacted: {rendered}"
        );
    }

    #[test]
    fn accessors_expose_non_secret_fields() {
        let c = sample_challenge();
        assert_eq!(c.account_id(), "alice");
        assert!(c.totp_url().as_str().contains("id-pass_form_newBF"));
    }

    #[test]
    fn clone_produces_independent_copy() {
        // TotpChallenge is Clone so callers can inspect it multiple
        // times without consuming the boxed variant inside
        // LoginError. The clone must preserve every field.
        let original = sample_challenge();
        let cloned = original.clone();
        assert_eq!(original.account_id, cloned.account_id);
        assert_eq!(original.session_key, cloned.session_key);
        assert_eq!(original.totp_url, cloned.totp_url);
        assert_eq!(original.viewstate, cloned.viewstate);
        assert_eq!(original.service_code, cloned.service_code);
        assert_eq!(original.service_region, cloned.service_region);
    }

    #[test]
    fn debug_surfaces_service_metadata_non_secret() {
        // Service metadata is public (game catalog), so Debug shows
        // it verbatim — handy when triaging "wrong game slot" bugs.
        let rendered = format!("{:?}", sample_challenge());
        assert!(
            rendered.contains("610074"),
            "service_code must be visible in Debug: {rendered}"
        );
        assert!(
            rendered.contains("T9"),
            "service_region must be visible in Debug: {rendered}"
        );
    }
}
