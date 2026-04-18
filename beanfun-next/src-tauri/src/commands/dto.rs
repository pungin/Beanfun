//! IPC data-transfer objects (DTOs) owned by the command layer.
//!
//! # Q4 hybrid strategy (P10.2 pre-flight)
//!
//! Todo.md L897 locks in the "hybrid" approach to domain → IPC
//! marshalling:
//!
//! - **Data-only domain types** — [`LoginRegion`],
//!   `services::beanfun::account::ServiceAccount`, QR / verify /
//!   TOTP payloads etc. — derive [`specta::Type`] **directly on
//!   the domain struct** (analogous to how they already derive
//!   [`serde::Serialize`]). These are cross-layer contract traits,
//!   not business logic, so their presence on the domain type is
//!   not a layer violation. No shadow DTO needed.
//! - **Secret-or-resource-bearing domain types** —
//!   [`Session`] (holds `skey` / `web_token`),
//!   `services::beanfun::session::Credentials` (holds plaintext
//!   password under `Zeroize` policy) — **never** cross the IPC
//!   boundary. This module defines a command-layer **shadow DTO**
//!   that strips the sensitive fields, plus an explicit
//!   `From<&Domain>` impl so the conversion is the single documented
//!   path.
//!
//! This module therefore contains only the shadow DTOs
//! ([`SessionInfo`]) and shared IPC helpers ([`encode_png_base64`]).
//! Everything else — `ServiceAccount`, `QrLoginInit`, `VerifyOutcome`,
//! etc. — derives `specta::Type` in its own `services::beanfun::*`
//! module.
//!
//! # Binary payloads over JSON
//!
//! IPC payloads serialize as JSON, which is not friendly to raw
//! `Vec<u8>` (would become a `number[]`, blowing up size ~4×). The
//! command layer always encodes binary blobs as **base64 strings** so
//! the frontend can drop them into `<img src="data:image/png;base64,...">`
//! directly. The one-line helper [`encode_png_base64`] guarantees the
//! same engine (`base64::engine::general_purpose::STANDARD`) is used
//! everywhere, keeping the contract uniform across `login_qr_start`
//! (QR image), `get_verify_captcha` (verify captcha image), and any
//! future binary surface.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use specta::Type;

use crate::services::beanfun::{client::LoginRegion, login::TotpChallenge, session::Session};

/// Public-safe snapshot of an authenticated [`Session`], suitable for
/// exposure over IPC.
///
/// # What's inside
///
/// - `region` — which Beanfun region the session authenticates against.
/// - `account_id` — the user-facing login id (same thing that appears
///   on the invoice / support ticket).
/// - `service_code` / `service_region` — the MapleStory service this
///   session defaults to launching (`"610074"` / `"T9"` for TW & HK;
///   WPF parity).
///
/// # What's **NOT** inside
///
/// - `skey` — one-time session key. Held only in the backend.
/// - `web_token` (`bfWebToken` cookie value) — leaking this is
///   equivalent to leaking the session. Held only in the backend (in
///   the cookie jar owned by
///   [`BeanfunClient`][crate::services::beanfun::client::BeanfunClient]).
///
/// The frontend never needs these two values because every Beanfun
/// call happens through the backend command layer, which already
/// carries the session via [`AppState`][crate::commands::state::AppState].
/// Not exposing them is a defence-in-depth measure: even if a future
/// renderer-side XSS leaked `localStorage` or a Tauri IPC response
/// log, the session secrets would remain inside the main process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SessionInfo {
    /// Beanfun region (`TW` / `HK`) — see [`LoginRegion`].
    pub region: LoginRegion,
    /// Login account id. Non-secret.
    pub account_id: String,
    /// MapleStory service code (`"610074"` for both TW and HK in the
    /// WPF reference).
    pub service_code: String,
    /// MapleStory service region (`"T9"` for both TW and HK in the
    /// WPF reference).
    pub service_region: String,
}

impl From<&Session> for SessionInfo {
    fn from(session: &Session) -> Self {
        Self {
            region: session.region,
            account_id: session.account_id.clone(),
            service_code: session.service_code.clone(),
            service_region: session.service_region.clone(),
        }
    }
}

impl From<Session> for SessionInfo {
    fn from(session: Session) -> Self {
        SessionInfo::from(&session)
    }
}

/// Public-safe snapshot of a pending TOTP challenge, carried inside
/// the `CommandError { code: "auth.totp_required", details }`
/// surface so the frontend can render "enter 6-digit OTP for
/// `{account_id}`" without ever seeing the underlying
/// [`TotpChallenge`]'s server-side state.
///
/// # What's inside
///
/// - `totp_url` — the URL the TOTP POST will target, exposed purely
///   for diagnostics (a UI might show it in an advanced panel).
///   Not usable on its own — the frontend cannot replay the POST
///   because it lacks the viewstate bundle.
/// - `account_id` — the login id bound to this challenge. Shown in
///   the OTP prompt so the user knows which account they're
///   completing.
///
/// # What's **NOT** inside
///
/// - `session_key` (`pSKey`) — session bearer equivalent for the
///   login window; kept in [`PendingTotp`][crate::commands::state::PendingTotp].
/// - `viewstate` — ASP.NET Base64 server-side state; also kept on
///   the backend slot.
///
/// The split mirrors the
/// [`Session` → `SessionInfo`][SessionInfo] pattern: secrets stay
/// behind the IPC boundary, only the fields a UI can legitimately
/// display cross to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct TotpChallengeInfo {
    /// The URL the TOTP POST will target. Diagnostic-only — the
    /// frontend does not re-POST.
    pub totp_url: String,
    /// The login account id the challenge is bound to. Safe to
    /// render in the OTP prompt.
    pub account_id: String,
}

impl From<&TotpChallenge> for TotpChallengeInfo {
    fn from(c: &TotpChallenge) -> Self {
        Self {
            totp_url: c.totp_url().to_string(),
            account_id: c.account_id().to_string(),
        }
    }
}

/// Encode `bytes` as a standard-alphabet base64 string, suitable for
/// embedding in a `data:image/png;base64,…` URI on the frontend.
///
/// Uses [`base64::engine::general_purpose::STANDARD`] (the same
/// engine WPF `System.Convert.ToBase64String` produces), so captcha /
/// QR image strings round-trip byte-for-byte against the reference
/// implementation.
///
/// # When to use this
///
/// Every command that hands raw bytes to the frontend. As of P10.2
/// that's:
///
/// - `login_qr_start` — QR PNG image.
/// - `get_verify_captcha` — verify captcha JPEG/PNG image.
///
/// Future binary surfaces (avatars, export blobs) should reuse this
/// helper so only one base64 engine choice lives in the codebase.
pub fn encode_png_base64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::ViewStateForm;
    use url::Url;

    fn sample_session() -> Session {
        Session::new(
            LoginRegion::TW,
            "SKEY_SECRET_VALUE",
            "BFWT_SECRET_VALUE",
            "alice",
            "610074",
            "T9",
        )
    }

    #[test]
    fn session_info_from_session_copies_public_fields() {
        let session = sample_session();
        let info = SessionInfo::from(&session);

        assert_eq!(info.region, LoginRegion::TW);
        assert_eq!(info.account_id, "alice");
        assert_eq!(info.service_code, "610074");
        assert_eq!(info.service_region, "T9");
    }

    #[test]
    fn session_info_by_value_and_by_ref_produce_equal_results() {
        let session = sample_session();
        let by_ref = SessionInfo::from(&session);
        let by_value = SessionInfo::from(sample_session());
        assert_eq!(by_ref, by_value);
    }

    /// Serialize a [`SessionInfo`] built from a [`Session`] whose
    /// `skey` / `web_token` carry easy-to-recognise sentinel values,
    /// then assert the JSON text contains neither sentinel anywhere.
    ///
    /// This is the acid test for the "no session secrets cross IPC"
    /// policy: a future refactor that accidentally added `skey:
    /// session.skey.clone()` to [`SessionInfo`] would break this
    /// immediately.
    #[test]
    fn session_info_json_never_contains_secret_fields() {
        let session = sample_session();
        let info = SessionInfo::from(&session);
        let json = serde_json::to_string(&info).expect("serializes");

        assert!(
            !json.contains("SKEY_SECRET_VALUE"),
            "skey must not leak into IPC JSON: {json}"
        );
        assert!(
            !json.contains("BFWT_SECRET_VALUE"),
            "web_token must not leak into IPC JSON: {json}"
        );

        // Positive assertions to lock the public shape.
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = value.as_object().expect("object-shaped");
        assert_eq!(obj.len(), 4, "exactly 4 public fields expected: {json}");
        assert!(obj.contains_key("region"));
        assert!(obj.contains_key("account_id"));
        assert!(obj.contains_key("service_code"));
        assert!(obj.contains_key("service_region"));
    }

    #[test]
    fn encode_png_base64_round_trips_with_standard_engine() {
        // Synthetic 8-byte PNG-like header pattern; the helper is
        // format-agnostic so any byte sequence round-trips.
        let bytes: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let encoded = encode_png_base64(&bytes);

        assert!(
            encoded.is_ascii(),
            "base64 output must be ASCII: {encoded}"
        );
        let decoded = STANDARD
            .decode(&encoded)
            .expect("standard-alphabet encoding decodes back");
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn encode_png_base64_empty_bytes_returns_empty_string() {
        assert_eq!(encode_png_base64(&[]), "");
    }

    fn sample_totp_challenge() -> TotpChallenge {
        TotpChallenge {
            totp_url: Url::parse(
                "https://login.hk.beanfun.com/login/id-pass_form_newBF.aspx?otp1=SK",
            )
            .expect("static URL"),
            viewstate: ViewStateForm {
                viewstate: "VS_SECRET_PAYLOAD".into(),
                viewstate_generator: Some("GEN_SECRET".into()),
                event_validation: Some("EV_SECRET".into()),
            },
            session_key: "SKEY_SECRET_VALUE".into(),
            account_id: "alice".into(),
            service_code: "610074".into(),
            service_region: "T9".into(),
        }
    }

    /// Same acid test as `session_info_json_never_contains_secret_fields`:
    /// a TotpChallenge with sentinel secrets must not leak any of them
    /// through the IPC DTO.
    #[test]
    fn totp_challenge_info_json_never_contains_secret_fields() {
        let info = TotpChallengeInfo::from(&sample_totp_challenge());
        let json = serde_json::to_string(&info).expect("serializes");

        assert!(
            !json.contains("SKEY_SECRET_VALUE"),
            "session_key must not leak: {json}"
        );
        assert!(
            !json.contains("VS_SECRET_PAYLOAD"),
            "viewstate must not leak: {json}"
        );
        assert!(
            !json.contains("GEN_SECRET"),
            "viewstate_generator must not leak: {json}"
        );
        assert!(
            !json.contains("EV_SECRET"),
            "event_validation must not leak: {json}"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = value.as_object().expect("object-shaped");
        assert_eq!(obj.len(), 2, "exactly 2 public fields expected: {json}");
        assert!(obj.contains_key("totp_url"));
        assert!(obj.contains_key("account_id"));
    }
}
