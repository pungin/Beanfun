//! QR-code login **finalize** step — runs the four HTTP calls that
//! turn an "approved" QR scan into a [`Session`].
//!
//! Run *after* [`super::poll_qr_login_status`] has returned
//! [`super::QrPollOutcome::Approved`].
//!
//! # WPF reference
//!
//! Two WPF methods compose the QR finalize sequence:
//!
//! - `BeanfunClient.Login.cs::QRCodeLogin` (L530-607) — steps 1-3
//!   (handshake → SendLogin → first `POST return.aspx`).
//! - `BeanfunClient.Login.cs::LoginCompleted` (L838-882) — step 4
//!   (second `POST return.aspx` with the `AuthKey="OK"` sentinel,
//!   re-reading `bfWebToken` from the cookie jar afterwards).
//!
//! Both methods run unconditionally for QR: `QRCodeLogin` returns the
//! string `"OK"` on success (L600), the enclosing `Login(...)` then
//! calls `LoginCompleted("OK", ...)` (L774-782), and
//! `LoginCompleted`'s `akey == null` early-exit (L844) does not fire
//! because `"OK"` is non-null. Splitting the WPF flow into per-step
//! functions lets each step be wiremock-tested in isolation and lets
//! us reuse [`super::send_login()`] / [`super::post_return_aspx()`] /
//! [`super::login_completed()`] verbatim from the HK Regular and TOTP
//! flows.
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
//! ## Step 3 — `POST return.aspx` with the SendLogin form (no-redirect)
//!
//! WPF L588-600. `redirect = false` → no-redirect client; `Referer:
//! https://login.beanfun.com/`; payload is the `<input>`s scraped
//! from step 2's HTML form.
//!
//! WPF scrapes `Set-Cookie` here for `bfWebToken` (L592-598), but
//! that captured value is **transient**: `LoginCompleted` (step 4
//! below) re-reads `bfWebToken` from the cookie jar after its own
//! POST, which means whatever value the jar holds *after step 4*
//! is the canonical one used by every subsequent API call (WPF
//! L868). We mirror that lifetime: we call [`super::post_return_aspx()`]
//! to perform the request (so the cookie jar is primed and any HTTP
//! / transport failure still surfaces as a typed error), then
//! **deliberately discard** the returned token — the canonical
//! webtoken comes from step 4.
//!
//! ### Leniency on missing `bfWebToken` (WPF parity)
//!
//! WPF L591-598 wraps the cookie read inside
//! `if (!string.IsNullOrEmpty(setCookieHeader))`, silently falling
//! through to `return "OK"` (L600) when the server omits the cookie.
//! The canonical lookup happens in step 4 anyway (L868), so step 3's
//! job is only to advance server state — not to produce a token.
//! We mirror that by swallowing [`LoginError::MissingWebToken`] at
//! this callsite specifically; every other error variant
//! (transport, HTTP non-2xx/3xx, invalid URL) still propagates
//! unchanged. Live testing on 2026-04-16 showed beanfun's TW server
//! does omit `Set-Cookie: bfWebToken` on this hop for some session
//! states, and a strict reading surfaced as `auth.missing_web_token`
//! to the user — aligning with WPF eliminates the regression.
//!
//! ## Step 4 — shared `LoginCompleted` tail (`AuthKey="OK"`)
//!
//! WPF L838-882. The same five-field `return.aspx` POST that HK
//! Regular and TOTP funnel through, with `AuthKey="OK"` and a blank
//! `account_id`. We delegate to [`super::login_completed()`] verbatim
//! — see `login/completed.rs` module docs for the wire shape and the
//! intentional divergences from WPF (skipping the auto-redirect chase
//! at L865, deferring `GetAccounts`/`getRemainPoint` to higher-level
//! callers).
//!
//! ### Why we run step 4 even though step 3 already returned a token
//!
//! An earlier draft of this module skipped step 4 on the assumption
//! that "the second POST is redundant — bfWebToken was already
//! captured in step 3". A line-by-line re-read of `LoginCompleted`
//! turned that into an unverified assumption: WPF deliberately does
//! the second POST + reads the cookie jar afterwards (L853-868),
//! which means the WPF developers expected the second POST to
//! either rotate the token or carry session-rotation state we don't
//! observe. Strictly aligning with WPF's wire shape eliminates the
//! risk of stale-token surprises in the absence of a real-server
//! test bed. See the chunk 3.4 review notes in `Todo.md`.
//!
//! ### Documented divergence: `Accept: */*` on steps 3 & 4
//!
//! WPF's `SetBaseHeaders(true, null, "https://login.beanfun.com/")`
//! sends **no** `Accept` header on the wire (L911-925) for both
//! `return.aspx` POSTs. reqwest 0.12 (via hyper) auto-injects
//! `Accept: */*` on every request and exposes no public API to
//! suppress it short of swapping HTTP clients. The shared
//! [`super::post_return_aspx()`] helper (used by both step 3 and step
//! 4 via [`super::login_completed()`]) does not set `Accept` itself,
//! so both POSTs end up with `Accept: */*` instead of "absent". The
//! two are semantically equivalent — RFC 9110 §12.5.1 specifies
//! `*/*` as the implicit default when `Accept` is omitted — and no
//! Beanfun endpoint observed in WPF's traffic switches on this
//! difference. The integration tests
//! `step3_return_aspx_posts_send_login_form_with_login_base_referer`
//! and `step4_login_completed_posts_five_field_form_with_authkey_ok`
//! lock the divergence so a real wire-shape regression elsewhere
//! still trips an assertion.
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
use super::{ensure_success, login_completed, post_return_aspx, send_login};
use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion, Session};

/// `akey` sentinel WPF passes to `LoginCompleted` for the QR flow.
/// `QRCodeLogin` returns the literal string `"OK"` on success
/// (`BeanfunClient.Login.cs::QRCodeLogin` L600), and `Login(...)` then
/// forwards that as the `akey` argument to `LoginCompleted` (L774-782).
/// Surfacing the value as a named constant keeps the WPF reference
/// trivially greppable from both this module and `login_completed`.
const QR_LOGIN_COMPLETED_AKEY: &str = "OK";

/// `Accept` header value WPF's `QRCodeLogin` sends on the SendLogin
/// GET (L545). Differs from the TW Regular value (L124) by adding
/// `image/avif,image/webp,image/apng`. Surfaced as a constant so the
/// test in `tests/qr_finalize.rs` can assert on the exact byte
/// string sent on the wire.
const QR_SEND_LOGIN_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";

/// Run the four-step QR finalize sequence and assemble a [`Session`].
///
/// `init` carries the `skey` (used to rebuild the `Login/Index`
/// `Referer` URL and as the `SessionKey` field in step 4's form) and
/// the `verification_token` (currently unused in finalize, but kept
/// on the bundle so callers don't have to thread the value separately
/// between [`super::poll_qr_login_status`] and this function).
///
/// See module docs for the per-step header / payload contracts and
/// the rationale for running step 4 even after step 3 already
/// captured a transient `bfWebToken`.
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

    // Step 3 — `POST return.aspx` with the SendLogin form (WPF L588-600).
    // We deliberately discard the captured `bfWebToken` here: this POST
    // exists to advance server-side session state and prime the cookie
    // jar, but the *canonical* token is the one captured in step 4
    // below. WPF reads `this.webtoken = this.GetCookie("bfWebToken")`
    // (L868) AFTER `LoginCompleted`'s second POST, which means the jar
    // value at that point — not the value scraped here — is what every
    // subsequent API call uses. See module-level "Step 4" docs for the
    // alignment rationale.
    //
    // WPF parity for missing cookie: L591-598 wraps the scrape in
    // `if (!string.IsNullOrEmpty(setCookieHeader))` and silently falls
    // through to `return "OK"` when the cookie is absent. Mirror that
    // by swallowing `MissingWebToken` specifically; every other error
    // (transport, HTTP 4xx/5xx, URL) still short-circuits. See
    // module-level "Leniency on missing bfWebToken" section.
    match post_return_aspx(client, &form).await {
        Ok(_) | Err(LoginError::MissingWebToken) => {}
        Err(other) => return Err(other),
    }

    // Step 4 — shared `LoginCompleted` tail (WPF L838-882). Mirrors
    // what HK Regular and TOTP also do; the QR-specific bits are the
    // hardcoded `"OK"` akey sentinel and the empty `account_id` (QR
    // has no user-typed account; populated by GetAccounts in P3.5).
    login_completed(
        client,
        &init.skey,
        QR_LOGIN_COMPLETED_AKEY,
        "",
        LoginRegion::TW.default_service_code(),
        LoginRegion::TW.default_service_region(),
    )
    .await
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
