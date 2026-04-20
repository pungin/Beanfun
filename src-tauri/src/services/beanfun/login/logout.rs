//! Logout flow — terminates the active Beanfun session by hitting the
//! WPF `BeanfunClient.Logout()` endpoints in order.
//!
//! # WPF reference
//!
//! Ports `BeanfunClient.Login.cs::Logout` (L884-909). Three sequential
//! HTTP calls:
//!
//! | Step | Method | URL                                                                       | Region  |
//! |------|--------|---------------------------------------------------------------------------|---------|
//! | 1    | GET    | `{portal_base}generic_handlers/remove_bflogin_session.ashx`               | both    |
//! | 2    | GET    | `{logout_host}logout.aspx?service=999999_T0`                              | both    |
//! | 3    | POST   | `{newlogin_base}generic_handlers/erase_token.ashx` (body `web_token=1`)   | TW only |
//!
//! `logout_host` is region-dependent. WPF overloads one local
//! variable name (`loginHost`, L887-897) for two
//! conceptually-different hosts:
//!
//! - **TW** routes step 2 through `tw.newlogin.beanfun.com` —
//!   our [`Endpoints::newlogin_base`](super::super::client::Endpoints::newlogin_base).
//! - **HK** routes step 2 through `login.hk.beanfun.com` —
//!   our [`Endpoints::login_base`](super::super::client::Endpoints::login_base).
//!
//! We faithfully port the same dispatch as a small `match` on the
//! region inside the private `logout_aspx` helper below.
//!
//! # Headers
//!
//! WPF's `Logout()` does **not** call `SetBaseHeaders`, so each
//! `WebClient` call inherits whatever headers the previous login
//! step left on the instance — non-deterministic and impossible to
//! mirror byte-for-byte without coupling logout to the entire
//! preceding flow. We therefore send only the baseline User-Agent
//! (set globally on the reqwest client) and the per-session cookie
//! jar. The server has never been observed to require step-specific
//! headers on these endpoints.
//!
//! ## Documented divergence: `Accept: */*`
//!
//! reqwest 0.12 (via hyper) auto-injects `Accept: */*` on every
//! request and exposes no public API to suppress it. This matches
//! the divergence already documented in `qr_finalize.rs` for the
//! `return.aspx` POSTs and is semantically inert per RFC 9110
//! §12.5.1 (`*/*` is the implicit default when `Accept` is absent).
//!
//! # Failure policy
//!
//! Best-effort: if any step fails we capture the error but **still
//! attempt the remaining steps**, then return the **first** error
//! encountered.
//!
//! ## Two intentional divergences from WPF
//!
//! 1. **WPF short-circuits internally.** `WebClient.DownloadString`
//!    throws `WebException` on any non-2xx response, and WPF's
//!    `Logout()` (Login.cs L884-909) is a flat sequence of
//!    `DownloadString` / `UploadString` calls with no `try`/`catch`
//!    inside the method itself — so a failed step 1 means steps 2
//!    and 3 never run. We deliberately do the opposite: every
//!    server-side cleanup endpoint gets a chance to fire even if a
//!    transient blip kills the first one. The thing we're trying to
//!    do (server-side session invalidation) is naturally idempotent
//!    and the steps are independent, so running all three is
//!    strictly safer than the WPF behaviour.
//!
//! 2. **WPF's callers swallow the error.** `App.xaml.cs` L72-76 and
//!    `MainWindow.xaml.cs` L237-241 both wrap `Logout()` in
//!    `try { } catch { }`, treating it as fire-and-forget. We
//!    return `Result<(), LoginError>` so callers can log / surface
//!    failures if they want. Callers wanting exact WPF semantics
//!    (run + ignore everything) can simply do
//!    `let _ = logout(&client).await;`.
//!
//! ## Why FIRST error and not all
//!
//! The first error is generally the most diagnostic — subsequent
//! failures are typically cascades from the same network or
//! session issue (e.g. step 1 dies on a TLS error and steps 2/3
//! fail for the same reason; the step 1 error is what the human
//! needs to see). Returning a `Vec<LoginError>` would be more
//! complete but would force every caller to write reduction logic
//! for a payload that, in practice, has one root cause.
//!
//! # Cookie jar
//!
//! Deliberately not cleared. Mirrors WPF, which never clears its
//! `WebClient`'s cookie jar inside `Logout()` either — the design
//! relies on the server-side endpoints invalidating the session.
//! For our long-lived process the supported pattern for fully
//! isolating a new session is to drop the [`BeanfunClient`] and
//! construct a fresh one — see the "Cookie jar" section of the
//! [`client`](super::super::client) module docs.

use crate::services::beanfun::{BeanfunClient, LoginError, LoginRegion};

use super::ensure_success;

/// Drive the WPF `Logout()` sequence: 2-3 region-aware HTTP calls.
///
/// All steps run regardless of earlier failures (best-effort —
/// see module docs). Returns `Ok(())` if every step succeeds, or
/// the **first** error encountered otherwise. The caller is free
/// to ignore the result (`let _ = logout(&client).await;`) for
/// exact WPF fire-and-forget semantics.
pub async fn logout(client: &BeanfunClient) -> Result<(), LoginError> {
    let mut first_err: Option<LoginError> = None;

    if let Err(e) = remove_bflogin_session(client).await {
        first_err.get_or_insert(e);
    }
    if let Err(e) = logout_aspx(client).await {
        first_err.get_or_insert(e);
    }
    if client.config().region == LoginRegion::TW {
        if let Err(e) = erase_token(client).await {
            first_err.get_or_insert(e);
        }
    }

    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Step 1 — `GET portal_base/generic_handlers/remove_bflogin_session.ashx`.
///
/// WPF L898: tells the portal host (`tw.beanfun.com` /
/// `bfweb.hk.beanfun.com`) to forget the bflogin session id stored
/// against this user's cookies.
async fn remove_bflogin_session(client: &BeanfunClient) -> Result<(), LoginError> {
    let url = client.portal_url("generic_handlers/remove_bflogin_session.ashx")?;
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "remove_bflogin_session")
}

/// Step 2 — `GET {logout_host}/logout.aspx?service=999999_T0`.
///
/// WPF L899. `logout_host` is region-dependent (see module docs):
/// TW → `newlogin_base`, HK → `login_base`. The literal
/// `service=999999_T0` is what WPF hardcodes — not tied to any
/// real service code we observe, just a sentinel the logout
/// endpoint requires on the query string.
async fn logout_aspx(client: &BeanfunClient) -> Result<(), LoginError> {
    let mut url = match client.config().region {
        LoginRegion::TW => client.newlogin_url("logout.aspx")?,
        LoginRegion::HK => client.login_url("logout.aspx")?,
    };
    // Build the query string explicitly so the url crate handles any
    // encoding edge cases instead of relying on `.join()` to parse a
    // pre-formatted `?service=…` literal.
    url.query_pairs_mut().append_pair("service", "999999_T0");

    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "logout.aspx")
}

/// Step 3 — `POST newlogin_base/generic_handlers/erase_token.ashx`
/// with the form body `web_token=1` (TW only).
///
/// WPF L900-908. The literal `"1"` is a sentinel: WPF does not
/// post the user's actual `bfWebToken` value here, just a non-empty
/// string to satisfy the endpoint's form schema. The server identifies
/// the token to delete via the session cookie, not via this field.
///
/// Skipped for HK because WPF's L900 `if (App.LoginRegion == "TW")`
/// guard never fires there.
async fn erase_token(client: &BeanfunClient) -> Result<(), LoginError> {
    let url = client.newlogin_url("generic_handlers/erase_token.ashx")?;
    let resp = client
        .http()
        .post(url)
        .form(&[("web_token", "1")])
        .send()
        .await?;
    ensure_success(&resp, "erase_token")
}
