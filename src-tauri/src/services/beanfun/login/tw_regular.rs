//! Orchestrator for the **TW Regular** login flow — `account + password`
//! against the Taiwan portal.
//!
//! Sequence (one arrow per HTTP call, WPF reference line numbers in
//! `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`):
//!
//! 1. `GET /` → session key                             (L30-38, step 0)
//! 2. `GET Login/Index?pSKey=…` → antiforgery token     (L40-54)
//! 3. `POST Login/CheckAccountType` → captcha token     (L57-78)
//! 4. `POST Login/AccountLogin` → success / advance ck  (L80-113)
//! 5. `GET Login/SendLogin` → hidden-form payload       (L114-146)
//! 6. `POST return.aspx` (no-redirect) → bfWebToken     (L148-176)
//!
//! Each step lives in its own submodule so we can unit-test decode logic
//! without HTTP, and integration-test each branch via wiremock. This
//! file is intentionally thin — its only job is to wire the steps
//! together and build the final [`Session`].

use super::{
    account_login, check_account_type, check_recaptcha_required, get_login_index, get_session_key,
    post_return_aspx, send_login,
};
use crate::services::beanfun::{BeanfunClient, Credentials, LoginError, LoginRegion, Session};

/// Run the full TW Regular login flow.
///
/// Preconditions:
/// - `client.config().region` **must** be [`LoginRegion::TW`]. Calling
///   this on an HK-configured client is a programming error; we
///   `debug_assert` it. In release builds we still work since the
///   per-step calls are region-agnostic, but the HTTP endpoints would
///   be wrong.
///
/// On success returns a [`Session`] ready to drive subsequent calls
/// (service-account listing, game launch, logout).
pub async fn login_tw_regular(
    client: &BeanfunClient,
    creds: &Credentials,
) -> Result<Session, LoginError> {
    debug_assert_eq!(
        client.config().region,
        LoginRegion::TW,
        "login_tw_regular requires a TW-configured BeanfunClient"
    );

    let skey = get_session_key(client).await?;
    let index = get_login_index(client, &skey).await?;
    let index_url = index.index_url.as_str().to_owned();

    // As of 2026-06-25 the server may gate the account/password POSTs
    // behind a Google reCAPTCHA v2 challenge for this attempt (see the
    // `init_login` module docs). A v2 token cannot be produced
    // headlessly, so bail to the interactive WebView login when
    // required; otherwise the headless flow below is unchanged.
    if check_recaptcha_required(client, &skey, &index_url).await? {
        tracing::info!(
            step = "TwRegular",
            region = ?LoginRegion::TW,
            account_id = %creds.account,
            "reCAPTCHA required; deferring to interactive WebView login",
        );
        return Err(LoginError::RecaptchaRequired { skey });
    }

    let captcha = check_account_type(
        client,
        &skey,
        &creds.account,
        &index.verification_token,
        &index_url,
    )
    .await?;

    account_login(
        client,
        &skey,
        creds,
        &captcha,
        &index.verification_token,
        &index_url,
    )
    .await?;

    // WPF L124 — TW Regular's SendLogin Accept header. Differs from the
    // QR flow's Accept (which adds image/avif,image/webp,image/apng);
    // see `login/send_login.rs` module docs for the comparison.
    let form = send_login(
        client,
        &index_url,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    )
    .await?;
    let web_token = post_return_aspx(client, &form).await?;

    // Operator observability: emit a single success line per completed
    // login so "happy path" is no longer silent in operator logs.
    // `account_id` is non-secret (exposed verbatim via `SessionInfo` to
    // the frontend) and the skey / web_token values are deliberately
    // omitted — both are session bearers whose `Session::Debug` impl
    // already redacts them to keep `tracing` captures safe.
    tracing::info!(
        step = "TwRegular",
        region = ?LoginRegion::TW,
        account_id = %creds.account,
        "login flow completed successfully"
    );

    Ok(Session::new(
        LoginRegion::TW,
        skey,
        web_token,
        &creds.account,
        LoginRegion::TW.default_service_code(),
        LoginRegion::TW.default_service_region(),
    ))
}
