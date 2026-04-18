//! Shared "login tail" — the final hop that **every** login flow
//! funnels through.
//!
//! # WPF reference
//!
//! Ports `BeanfunClient.Login.cs::LoginCompleted` (L838-882). WPF signs
//! off every successful HK Regular / TOTP / QR flow with the **exact
//! same five-field form** posted to
//! `…/beanfun_block/bflogin/return.aspx`. (TW Regular is the lone
//! exception — see "Why this is a 'shared tail'" below for why it
//! still has its own inline `return.aspx` step.) The five-field form
//! shape:
//!
//! | Field              | Value                                          |
//! |--------------------|------------------------------------------------|
//! | `SessionKey`       | `pSKey` from the portal entry page (WPF `this.SessionKey`) |
//! | `AuthKey`          | `akey` captured from the `ResponseUri` query   |
//! | `ServiceCode`      | **empty string**                               |
//! | `ServiceRegion`    | **empty string**                               |
//! | `ServiceAccountSN` | literal `"0"`                                  |
//!
//! The `ServiceCode` / `ServiceRegion` fields being blank at the wire
//! level is **intentional** — WPF hardcodes `""` even though it has the
//! real values (`"610074"` / `"T9"`) in scope. The real service metadata
//! travels back to us in the `Session` struct we build; the form values
//! are just echoes that the return endpoint ignores for account lookup.
//!
//! # Why this is a "shared tail"
//!
//! TW Regular has its own inline `return.aspx` step inside `tw_regular.rs`
//! because the form there is **scraped from `SendLogin`** — not the
//! fixed five-field shape. HK Regular / TOTP / QR all use the fixed
//! shape, so they call [`login_completed`] instead of rebuilding the
//! form by hand.
//!
//! # Wire shape — auto-redirect + cookie-jar read (WPF L863-868 parity)
//!
//! Unlike TW Regular's inline `return.aspx` (which uses
//! [`super::post_return_aspx`] on a no-redirect client and scrapes
//! `Set-Cookie` from the **immediate** 302), `LoginCompleted` talks
//! to a different server surface: WPF L863 posts the five-field form
//! via `UploadString` (auto-follow enabled because no
//! `redirect = false` override is in scope — see WPF L921 in
//! `SetBaseHeaders`), then L868 reads `bfWebToken` from the cookie
//! **jar** via `GetCookie("bfWebToken")` (L153-163), scoped to
//! `https://{portal_host}/` (L144-150). WPF-observed beanfun traffic
//! carries `bfWebToken` on one of the **later** redirect hops, not
//! on the first 302 — scraping `Set-Cookie` header-by-header from
//! the first hop is not sufficient.
//!
//! We mirror that exactly:
//!
//! 1. POST via the redirect-following [`BeanfunClient::http`] client.
//! 2. `ensure_success` on the final response (after the chain settles).
//! 3. Discard the body — WPF L874-879 further calls `GetAccounts` /
//!    `getRemainPoint`, but we defer those to higher-level callers
//!    (see "No `GetAccounts` / `getRemainPoint` tail" below).
//! 4. Read `bfWebToken` from the shared [`CookieStoreMutex`] the two
//!    reqwest clients share (see `BeanfunClient::cookie_store`),
//!    scoped to the portal domain.
//!
//! This differs from an earlier draft that piggy-backed on
//! [`super::post_return_aspx`]: that helper is TW Regular-shaped
//! (no-redirect + header scrape) and produced `MissingWebToken`
//! errors against live beanfun traffic because `bfWebToken` arrived
//! only on a later hop. See 2026-04-16 Todo.md hotfix entry.
//!
//! # Intentional divergences from WPF
//!
//! - **No `GetAccounts` / `getRemainPoint` tail.** WPF L874-879 calls
//!   both inside `LoginCompleted` and stores the results on the client.
//!   We keep `login_completed` narrowly scoped to "finalise auth →
//!   return `Session`"; account listing and balance queries live in
//!   P4's `services/beanfun/account.rs` and the orchestrator chains
//!   them when the caller actually wants them. This keeps SRP clean
//!   and avoids blocking the login path on downstream API calls that
//!   can fail independently.

use reqwest::header;

use crate::core::parser::HiddenInput;
use crate::services::beanfun::{BeanfunClient, LoginError, Session};

use super::{ensure_success, read_bfwebtoken_from_jar};

/// Run the shared login-tail: post the five-field form to `return.aspx`,
/// extract `bfWebToken` from the response, and wrap everything into a
/// [`Session`] tied to the client's configured region.
///
/// # Parameters
///
/// - `session_key` — the `pSKey` the orchestrator obtained from
///   `get_session_key`. Stored on the final `Session.skey`.
/// - `akey` — the `AuthKey` for whichever branch we came from. HK
///   Regular and TOTP scrape it from the redirect URL after their
///   login POST; the QR flow passes the literal sentinel `"OK"`
///   that `QRCodeLogin` returns on success (WPF L600 / L774-782).
/// - `account_id` — the user-facing login id, propagated onto
///   `Session.account_id` for UI purposes (not sent on the wire here).
/// - `service_code` / `service_region` — MapleStory service metadata.
///   Echoed onto `Session` but **not** on the wire; both request fields
///   are blank by design (see module docs).
///
/// # Error surface
///
/// - [`LoginError::MissingWebToken`] — the cookie jar contained no
///   `bfWebToken` entry after the redirect chain settled. Almost
///   always indicates an upstream issue (bad akey, expired session,
///   server-side policy change) rather than a local bug.
/// - [`LoginError::Unknown`] — `return.aspx` (or any redirect-chain
///   hop) returned a non-2xx final status. Mirrors WPF catching a
///   `WebException` at the outer try block (L604-607).
/// - [`LoginError::InvalidUrl`] / transport errors — propagated from
///   URL construction and the reqwest POST verbatim.
pub async fn login_completed(
    client: &BeanfunClient,
    session_key: &str,
    akey: &str,
    account_id: &str,
    service_code: &str,
    service_region: &str,
) -> Result<Session, LoginError> {
    debug_assert!(
        !session_key.is_empty(),
        "login_completed requires a non-empty session_key"
    );
    debug_assert!(
        !akey.is_empty(),
        "login_completed requires a non-empty akey"
    );

    let form = build_completed_form(session_key, akey);
    let url = client.portal_url("beanfun_block/bflogin/return.aspx")?;
    // WPF L921-922 `SetBaseHeaders(..., "https://login.beanfun.com/")`.
    // `Url::as_str()` preserves the trailing slash that url::Url
    // canonicalises for origin URLs, matching the WPF byte shape.
    let login_base = client.config().endpoints.login_base.as_str().to_owned();

    // WPF parity: use the redirect-following client (WPF's WebClient
    // auto-follows because `SetBaseHeaders` did not toggle
    // `AllowAutoRedirect = false` on L921-922). reqwest 0.12's
    // default `RedirectPolicy::default()` follows up to 10 hops —
    // more than enough for beanfun's observed 1-2 hop chain.
    let resp = client
        .http()
        .post(url)
        .header(header::REFERER, login_base)
        .form(&form)
        .send()
        .await?;

    ensure_success(&resp, "return.aspx (LoginCompleted tail)")?;
    // Drop explicitly to drain the body + return the connection to
    // the pool before we reach into the shared cookie jar. Not
    // strictly required for correctness (the Response's Drop impl
    // does the same), but makes the "jar is ready to read" intent
    // explicit.
    drop(resp);

    let web_token = read_bfwebtoken_from_jar(client).ok_or(LoginError::MissingWebToken)?;

    // Operator observability: single success line at the shared login
    // tail — covers HK Regular, HK TOTP, and QR Code flows in one log
    // site (they all funnel through `login_completed` by design).
    // `account_id` is non-secret; skey / web_token / akey are session
    // bearers and deliberately not logged — `Session::Debug` already
    // redacts them for safe capture elsewhere.
    //
    // Empty-account_id rendering: QR login has no user-typed id and
    // intentionally passes `""` here (see `qr_finalize.rs` "Session
    // .account_id" module docs — actual account resolves on the
    // subsequent `GetAccounts` call). Rendering that as a bare
    // `account_id=` in the log trips postmortem readers into hunting
    // a bug that isn't there, so substitute a sentinel that makes the
    // deferred-resolution intent visible. HK Regular / TOTP callers
    // *must* pass a non-empty id (they have the user's textbox value
    // in hand); if one somehow arrives here empty the same sentinel
    // will surface the gap instead of silently swallowing it — an
    // empty-value log line that would otherwise read identically.
    let account_id_display = if account_id.is_empty() {
        "<deferred>"
    } else {
        account_id
    };
    tracing::info!(
        step = "LoginCompleted",
        region = ?client.config().region,
        account_id = account_id_display,
        "login flow completed successfully"
    );

    Ok(Session::new(
        client.config().region,
        session_key,
        &web_token,
        account_id,
        service_code,
        service_region,
    ))
}

/// Assemble the five-field form that WPF L853-858 posts to `return.aspx`.
///
/// Extracted as a pure helper so we can unit-test the exact wire shape
/// without spinning up a mock HTTP server: the integration test
/// (`tests/login_completed.rs`) covers the "helper + reqwest
/// serialisation + server-side parse" path end-to-end, this test just
/// nails down the field order and values.
///
/// Field order matches WPF's `NameValueCollection.Add` call order
/// (L853-858). `reqwest::RequestBuilder::form(&Vec<…>)` preserves the
/// slice order when serialising to `application/x-www-form-urlencoded`,
/// so order-sensitive servers (we haven't seen one, but cheap insurance)
/// see exactly the WPF byte sequence.
pub(crate) fn build_completed_form(session_key: &str, akey: &str) -> Vec<HiddenInput> {
    vec![
        ("SessionKey".to_owned(), session_key.to_owned()),
        ("AuthKey".to_owned(), akey.to_owned()),
        ("ServiceCode".to_owned(), String::new()),
        ("ServiceRegion".to_owned(), String::new()),
        ("ServiceAccountSN".to_owned(), "0".to_owned()),
    ]
}

// -----------------------------------------------------------------------------
// Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_has_exact_wpf_field_order() {
        let form = build_completed_form("SKEY_X", "AKEY_Y");
        let names: Vec<&str> = form.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "SessionKey",
                "AuthKey",
                "ServiceCode",
                "ServiceRegion",
                "ServiceAccountSN",
            ],
            "field order must match WPF L853-858 exactly"
        );
    }

    #[test]
    fn form_values_include_session_key_and_akey_verbatim() {
        let form = build_completed_form("SKEY_X", "AKEY_Y");
        assert_eq!(form[0].1, "SKEY_X");
        assert_eq!(form[1].1, "AKEY_Y");
    }

    #[test]
    fn service_code_and_region_are_empty_strings_on_the_wire() {
        let form = build_completed_form("SKEY_X", "AKEY_Y");
        // WPF L856-857 — always empty, regardless of the real service
        // codes the caller plans to store on `Session`.
        assert_eq!(form[2].1, "");
        assert_eq!(form[3].1, "");
    }

    #[test]
    fn service_account_sn_is_literal_zero() {
        // WPF L858 — string "0", not int. Kept as string because the
        // wire is `application/x-www-form-urlencoded` either way.
        let form = build_completed_form("_", "_");
        assert_eq!(form[4].1, "0");
    }

    #[test]
    fn form_len_is_exactly_five() {
        // Guard against accidentally adding a sixth field — the
        // return.aspx endpoint has been known to reject forms with
        // unexpected keys in the past.
        let form = build_completed_form("_", "_");
        assert_eq!(form.len(), 5);
    }

    #[test]
    fn empty_inputs_still_produce_well_formed_form() {
        // Structural test only — `login_completed` itself debug-asserts
        // non-empty, but `build_completed_form` stays total so callers
        // building test fixtures can exercise empty-value edge cases.
        let form = build_completed_form("", "");
        assert_eq!(form.len(), 5);
        assert_eq!(form[0].1, "");
        assert_eq!(form[1].1, "");
    }
}
