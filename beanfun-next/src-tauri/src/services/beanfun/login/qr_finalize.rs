//! QR-code login **finalize** step — runs the three HTTP calls that
//! turn an "approved" QR scan into a [`Session`].
//!
//! Run *after* [`super::poll_qr_login_status`] has returned
//! [`super::QrPollOutcome::Approved`].
//!
//! # WPF reference
//!
//! `BeanfunClient.Login.cs::QRCodeLogin` (L530-607). The original
//! method is a single 80-line `try` block; we split it into three
//! discrete round-trips below so each step can be wiremock-tested in
//! isolation, and so we can reuse [`super::send_login()`] /
//! [`super::post_return_aspx()`] verbatim from the TW Regular flow
//! (same endpoints, identical response shapes — only the `Accept`
//! string for SendLogin differs and is parameterised in
//! `send_login`).
//!
//! ## Step 1 — `GET QRLogin/QRLogin` (handshake, body discarded)
//!
//! WPF L535-541. Pure session-state nudge: the response body is only
//! `Debug.WriteLine`'d, never parsed. The point of the call is the
//! cookies + server-side session state it primes for steps 2 and 3.
//!
//! Headers (mirroring `SetBaseHeaders(true, "application/json,
//! text/plain, */*", "https://login.beanfun.com/Login/Index?pSKey=…")`):
//!
//! - `User-Agent` — set globally on the reqwest client.
//! - `Accept: application/json, text/plain, */*`
//! - `Referer: {login_base}Login/Index?pSKey={skey}`
//!
//! No `Origin`, no `X-Requested-With`, no
//! `RequestVerificationToken` — `SetBaseHeaders` clears every
//! header first (L917) and the QR step adds none of the above back.
//!
//! ## Step 2 — `GET Login/SendLogin`
//!
//! WPF L543-580. Same endpoint the TW Regular flow hits, **with a
//! different `Accept`** that adds `image/avif,image/webp,image/apng`
//! to the list (L545 vs L124). We delegate to
//! [`super::send_login()`] and pass the QR-specific Accept string at
//! the callsite — see `login/send_login.rs` module docs for the rationale.
//!
//! Returns [`LoginError::SendLoginNoFormData`] when the page comes
//! back empty (WPF L582-586 `errmsg = "SendLoginNoFormData"`).
//!
//! ## Step 3 — `POST return.aspx` (no-redirect)
//!
//! WPF L588-598. `redirect = false` → no-redirect client; `Referer:
//! https://login.beanfun.com/`; raw `Set-Cookie` header scrape for
//! `bfWebToken` (the cookie jar would also carry it but WPF reads
//! the raw header so we do too — see
//! `login/return_aspx.rs` for the rationale).
//!
//! Returns [`LoginError::MissingWebToken`] when the response carries
//! no `bfWebToken` cookie. Both the no-cookie and unparseable-cookie
//! cases are handled by the shared [`super::post_return_aspx()`] helper
//! we reuse here.
//!
//! ### Documented divergence: `Accept: */*` on step 3
//!
//! WPF's `SetBaseHeaders(true, null, "https://login.beanfun.com/")`
//! sends **no** `Accept` header on the wire (L911-925). reqwest 0.12
//! (via hyper) auto-injects `Accept: */*` on every request and
//! exposes no public API to suppress it short of swapping HTTP
//! clients. The shared [`super::post_return_aspx()`] helper does not
//! set `Accept` itself, so step 3 ends up with `Accept: */*` instead
//! of "absent". The two are semantically equivalent — RFC 9110
//! §12.5.1 specifies `*/*` as the implicit default when `Accept` is
//! omitted — and no Beanfun endpoint observed in WPF's traffic
//! switches on this difference. The integration test
//! `step3_return_aspx_sends_login_base_referer_and_form_body`
//! locks the divergence so a real wire-shape regression elsewhere
//! still trips an assertion.
//!
//! ## Skipped — second `LoginCompleted` POST
//!
//! WPF's enclosing `Login(...)` (L746-801) calls `LoginCompleted`
//! after `QRCodeLogin` returns "OK", which fires a *second*
//! `POST return.aspx` with `AuthKey="OK"` + a hand-rolled payload
//! (L838-882). That call's only useful side effect — capturing
//! `bfWebToken` — is already done in step 3 above (WPF L592-598
//! captures the cookie raw inside `QRCodeLogin` itself). Per the
//! P3.4 design decision, we **skip** that redundant round-trip; the
//! `Session` we return already carries the `bfWebToken`. A future
//! `GetAccounts` step (P3.5) will populate the user's actual account
//! list, mirroring `LoginCompleted`'s only other responsibility.
//!
//! # Region scope
//!
//! Same as [`super::init_qr_login`] / [`super::poll_qr_login_status`]:
//! [`LoginError::QrUnsupportedRegion`] when the client targets HK,
//! short-circuiting before any HTTP traffic. WPF's UI hides the QR
//! button entirely for HK (`MainWindow.loginMethodInit` L1099-1114),
//! and `BeanfunClient` hardcodes the TW endpoints.
//!
//! # `Session.account_id`
//!
//! Set to the empty string here. Unlike the TW/HK Regular flows
//! (where `creds.account` is what the user typed and is the canonical
//! account id for the session), QR login has no user-typed account
//! — the actual account is whatever the mobile-app scan resolved to,
//! and we only learn it on the subsequent `GetAccounts` call (P3.5).
//! WPF "kind of" gets the same outcome by passing the textbox content
//! through (often empty for QR mode), then `LoginCompleted` calls
//! `GetAccounts` which overwrites; surfacing an explicit empty string
//! is the honest representation of "not yet known".

use reqwest::header;

use super::qr_init::QrLoginInit;
use super::{ensure_success, post_return_aspx, send_login};
use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion, Session};

/// `Accept` header value WPF's `QRCodeLogin` sends on the SendLogin
/// GET (L545). Differs from the TW Regular value (L124) by adding
/// `image/avif,image/webp,image/apng`. Surfaced as a constant so the
/// test in `tests/qr_finalize.rs` can assert on the exact byte
/// string sent on the wire.
const QR_SEND_LOGIN_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";

/// Run the three-step QR finalize sequence and assemble a [`Session`].
///
/// `init` carries the `skey` (used to rebuild the `Login/Index`
/// `Referer` URL) and the `verification_token` (currently unused in
/// finalize, but kept on the bundle so callers don't have to thread
/// the value separately between [`super::poll_qr_login_status`] and
/// this function).
///
/// See module docs for the per-step header / payload contracts.
pub async fn finalize_qr_login(
    client: &BeanfunClient,
    init: &QrLoginInit,
) -> Result<Session, LoginError> {
    if client.config().region != LoginRegion::TW {
        return Err(LoginError::QrUnsupportedRegion);
    }

    // Index-page URL is reused as Referer for both step 1 and step 2;
    // build once and pass `&str` to keep the call sites cheap.
    let index_url = client
        .login_url_with_skey("Login/Index", &init.skey)?
        .to_string();

    qrlogin_handshake(client, &index_url).await?;

    let form = send_login(client, &index_url, QR_SEND_LOGIN_ACCEPT).await?;
    let web_token = post_return_aspx(client, &form).await?;

    Ok(Session::new(
        LoginRegion::TW,
        &init.skey,
        web_token,
        // QR has no user-typed account id — populated by GetAccounts
        // in P3.5. See module docs.
        "",
        LoginRegion::TW.default_service_code(),
        LoginRegion::TW.default_service_region(),
    ))
}

/// Step 1 — `GET QRLogin/QRLogin`. Body intentionally discarded; the
/// point of the call is the session-state side effect (cookies + the
/// server-side handshake that step 2's SendLogin depends on).
///
/// Private helper kept inside `qr_finalize` because it has exactly
/// one caller. Splitting it out makes [`finalize_qr_login`] read like
/// the WPF method (handshake → SendLogin → return.aspx) and keeps
/// the per-step header set narrowly scoped.
async fn qrlogin_handshake(client: &BeanfunClient, index_url: &str) -> Result<(), LoginError> {
    let url = client.login_url("QRLogin/QRLogin")?;

    let resp = client
        .http()
        .get(url)
        .header(header::ACCEPT, "application/json, text/plain, */*")
        .header(header::REFERER, index_url)
        .send()
        .await?;

    ensure_success(&resp, "QRLogin/QRLogin")?;
    // WPF L541 only does `Debug.WriteLine(response)` — no parsing,
    // no field extraction, the body is discarded. We drop it on the
    // floor too (drained implicitly when `resp` goes out of scope).
    drop(resp);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lock the exact byte string we send on the wire for the
    /// SendLogin Accept header. WPF L545 — adding/removing tokens
    /// here would silently diverge from the reference implementation.
    #[test]
    fn qr_send_login_accept_matches_wpf_byte_for_byte() {
        assert_eq!(
            QR_SEND_LOGIN_ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,\
             image/avif,image/webp,image/apng,*/*;q=0.8"
        );
    }

    /// Lock that the QR Accept string is a strict superset of the TW
    /// Regular one — the difference is exactly the three image MIME
    /// types added in the middle. If a future WPF tweak narrows the
    /// QR Accept this assertion will fail loudly.
    #[test]
    fn qr_send_login_accept_extends_tw_regular_with_image_mime_types() {
        let tw_accept = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
        for token in tw_accept.split(',') {
            assert!(
                QR_SEND_LOGIN_ACCEPT.contains(token),
                "QR Accept missing TW Regular token `{token}`"
            );
        }
        for image_token in ["image/avif", "image/webp", "image/apng"] {
            assert!(
                QR_SEND_LOGIN_ACCEPT.contains(image_token),
                "QR Accept missing image token `{image_token}`"
            );
        }
    }
}
