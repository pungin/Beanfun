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
//! Transport-wise we reuse [`post_return_aspx`] verbatim: same host
//! (`portal_base/beanfun_block/bflogin/return.aspx`), same no-redirect
//! quirk, same `Set-Cookie` scrape for `bfWebToken`.
//!
//! # Intentional divergences from WPF
//!
//! - **Skip the redundant `Location` GET (WPF L865).** WPF calls
//!   `this.DownloadString($"https://{host}/{ResponseHeaders["Location"]}")`
//!   after the `UploadString`. Because WPF's `WebClient` auto-follows
//!   the 302 by default, `ResponseHeaders` already reflects the final
//!   200 response at that point — where `Location` is usually absent,
//!   making the second GET either a no-op or a request to the bare host.
//!   Our no-redirect client captures `bfWebToken` directly from the
//!   302's `Set-Cookie` header (same strategy used by TW's
//!   `post_return_aspx`), so the extra GET adds nothing we need.
//! - **No `GetAccounts` / `getRemainPoint` tail.** WPF L874-879 calls
//!   both inside `LoginCompleted` and stores the results on the client.
//!   We keep `login_completed` narrowly scoped to "finalise auth →
//!   return `Session`"; account listing and balance queries live in
//!   P4's `services/beanfun/account.rs` and the orchestrator chains
//!   them when the caller actually wants them. This keeps SRP clean
//!   and avoids blocking the login path on downstream API calls that
//!   can fail independently.

use crate::core::parser::HiddenInput;
use crate::services::beanfun::{BeanfunClient, LoginError, Session};

use super::post_return_aspx;

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
/// - [`LoginError::MissingWebToken`] — `return.aspx` returned no
///   `bfWebToken` cookie. Almost always indicates an upstream issue that
///   slipped through (bad akey, expired session) rather than a local
///   bug.
/// - Any [`LoginError`] that `post_return_aspx` can surface (HTTP,
///   invalid URL, etc.) bubbles up unchanged.
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
    let web_token = post_return_aspx(client, &form).await?;

    // Operator observability: single success line at the shared login
    // tail — covers HK Regular, HK TOTP, and QR Code flows in one log
    // site (they all funnel through `login_completed` by design).
    // `account_id` is non-secret; skey / web_token / akey are session
    // bearers and deliberately not logged — `Session::Debug` already
    // redacts them for safe capture elsewhere.
    tracing::info!(
        step = "LoginCompleted",
        region = ?client.config().region,
        account_id = %account_id,
        "login flow completed successfully"
    );

    Ok(Session::new(
        client.config().region,
        session_key,
        web_token,
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
