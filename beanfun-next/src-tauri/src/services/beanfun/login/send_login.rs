//! Step 4 of the TW Regular flow: `GET Login/SendLogin`.
//!
//! After `AccountLogin` succeeds the server hands back an HTML page
//! whose `<form>` holds all the opaque session tokens the portal expects
//! when the browser POSTs over to `beanfun_block/bflogin/return.aspx`.
//! WPF scrapes every non-submit `<input>` from that form — we do the
//! same via [`extract_hidden_inputs`](crate::core::parser::extract_hidden_inputs).
//!
//! WPF reference: `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin`
//! L114-146.

use reqwest::header;

use super::ensure_success;
use crate::core::parser::{extract_hidden_inputs, HiddenInput};
use crate::services::beanfun::{BeanfunClient, LoginError};

/// GET the SendLogin page and return its hidden form payload.
///
/// Returns [`LoginError::SendLoginNoFormData`] when the scrape finds
/// zero usable inputs (empty body, error page, or unexpected markup).
/// That mirrors the `errmsg = "SendLoginNoFormData"` branch at WPF L140.
pub async fn send_login(
    client: &BeanfunClient,
    index_url: &str,
) -> Result<Vec<HiddenInput>, LoginError> {
    let url = client.login_url("Login/SendLogin")?;

    let resp = client
        .http()
        .get(url)
        .header(
            header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header(header::REFERER, index_url)
        .send()
        .await?;

    ensure_success(&resp, "SendLogin")?;
    let body = client.bounded_text(resp).await?;
    let inputs = extract_hidden_inputs(&body);
    if inputs.is_empty() {
        return Err(LoginError::SendLoginNoFormData);
    }
    Ok(inputs)
}
