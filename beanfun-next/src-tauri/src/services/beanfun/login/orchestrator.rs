//! Top-level login dispatcher — routes a [`LoginMethod`] choice to
//! the appropriate single-shot login flow.
//!
//! # Why a typed dispatcher?
//!
//! The Tauri command layer (P6) needs ONE entry point that takes "the
//! user wants to log in via method X with these credentials" and
//! returns a [`Session`]. Doing that match in the command layer would
//! mean string-matching method names at the IPC boundary, with all
//! the runtime errors that implies. A typed enum here pushes that
//! decision into the type system at compile time, and keeps service-
//! level routing inside the service layer where it belongs.
//!
//! # What's in (and what's not)
//!
//! [`LoginMethod`] only enumerates **single-shot password flows** —
//! ones that take credentials, hit the network exactly once from the
//! caller's perspective, and return a fully-formed [`Session`]:
//!
//! - [`LoginMethod::TwRegular`] → [`super::login_tw_regular`]
//! - [`LoginMethod::HkRegular`] → [`super::login_hk_regular`]
//!
//! Multi-step / interactive flows are deliberately **not** in the
//! enum because they don't share the same input/output shape:
//!
//! - **HK TOTP** (`super::login_totp`) needs an interactive 6-digit
//!   code from the user mid-flow. The natural API is a two-call
//!   sequence: get the [`super::TotpChallenge`], then submit the
//!   code. Wedging that into a single-call enum would either force
//!   blocking on stdin (unacceptable in async) or force the caller
//!   to encode the code as part of the method, which collapses two
//!   conceptually-different states (challenge / answer) into one.
//!
//! - **QR Code** (`super::init_qr_login` →
//!   [`super::poll_qr_login_status`] → [`super::finalize_qr_login`])
//!   is a 3-step flow that the UI drives explicitly to render the
//!   QR image, poll status, and react to the
//!   [`super::QrPollOutcome`] state machine. Hiding any of those
//!   transitions inside a single dispatcher call would either
//!   delay or mis-render the QR UI.
//!
//! - **HK device-registered re-login**
//!   ([`super::login_registered_device`]) is a follow-up to
//!   `LoginRegion::HK + TOTP fail with DeviceRegistered`, not a
//!   user-selectable login method. It belongs to the TOTP/HK error
//!   recovery path, not to the top-level method picker.
//!
//! Both kinds of flows are still public exports from the parent
//! module — callers reach for them directly when needed.

use crate::services::beanfun::{BeanfunClient, Credentials, LoginError, Session};

use super::{login_hk_regular, login_tw_regular};

/// Choice of single-shot login method exposed to the Tauri command
/// layer. See module docs for what's in scope and what's not.
#[derive(Debug, Clone)]
pub enum LoginMethod<'a> {
    /// TW Regular login. The TW flow scrapes its own
    /// `service_code` / `service_region` from the SendLogin form's
    /// hidden inputs, so no service args are needed here.
    TwRegular,

    /// HK Regular login. HK has no equivalent server-driven
    /// scrape, so the caller must specify which game service to
    /// log into. Pass `LoginRegion::HK.default_service_code()` /
    /// `default_service_region()` for the WPF default
    /// (`new MapleStory` — `610074` / `T9`).
    HkRegular {
        service_code: &'a str,
        service_region: &'a str,
    },
}

/// Single-call top-level dispatcher: picks the right login flow
/// based on `method` and forwards `client` + `creds`.
///
/// The `client`'s [`super::super::LoginRegion`] should match the
/// chosen method's region — the underlying flow functions
/// debug-assert this. In release builds a region mismatch surfaces
/// as a normal flow-time error from whatever endpoint the wrong
/// region tries to hit.
pub async fn login_with(
    client: &BeanfunClient,
    method: LoginMethod<'_>,
    creds: &Credentials,
) -> Result<Session, LoginError> {
    match method {
        LoginMethod::TwRegular => login_tw_regular(client, creds).await,
        LoginMethod::HkRegular {
            service_code,
            service_region,
        } => login_hk_regular(client, creds, service_code, service_region).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LoginMethod` should be `Send` — orchestrator is invoked
    /// from the Tauri command layer (which runs on tokio worker
    /// threads). This test pins the trait bound so an accidental
    /// non-`Send` payload (e.g. an `Rc`) inside a future variant
    /// fails at compile time instead of crash time.
    #[test]
    fn login_method_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LoginMethod<'_>>();
    }
}
