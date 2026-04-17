//! Account-management surface: list service accounts, fetch contracts,
//! and create / rename them via the gamezone JSON handler.
//!
//! Ports the read-side and the JSON-shaped management endpoints of
//! `BeanfunClient.Account.cs` (chunk 4.1 of P4):
//!
//! | This module                                     | WPF reference (`Account.cs`)                   |
//! |-------------------------------------------------|------------------------------------------------|
//! | [`get_accounts`]                                | `GetAccounts`                                  |
//! | `get_create_time` (private helper)              | `GetCreateTime`                                |
//! | [`get_service_contract`]                        | `GetServiceContract`                           |
//! | [`add_service_account`]                         | `AddServiceAccount`                            |
//! | [`change_service_account_display_name`]         | `ChangeServiceAccountDisplayName`              |
//!
//! WebForms-shaped management endpoints (`UnconnectedGame_*`,
//! `UnconnectedGame_ChangePassword`) live in chunk 4.4.
//!
//! # State model
//!
//! Following the P3 convention, every public function takes
//! `(&BeanfunClient, &Session, ...)`:
//!
//! - [`BeanfunClient`] holds HTTP plumbing (cookies, region, timeouts).
//!   The bfWebToken cookie is on the jar from `login_*` finishing, so
//!   we don't pass it explicitly to most calls; the few endpoints that
//!   take `web_token` as a *URL query parameter* (e.g. `auth.aspx`)
//!   read it from `session.web_token`.
//! - [`Session`] holds post-login state (`web_token`, `region`, etc).
//!
//! # Account ordering
//!
//! [`get_accounts`] sorts the returned [`AccountListResult::accounts`]
//! by ascending `ssn` (deterministic default — matches the *first* sort
//! pass WPF runs at `Account.cs` L130-135). WPF then layers a
//! user-defined order on top via `AccountList.ApplyAccountOrder`, which
//! reads from persistent storage. That second pass belongs to the
//! storage / UI command layers (P5+), not the service layer — bringing
//! it here would couple this module to disk I/O for a feature that
//! can be applied as a pure transformation by the caller.
//!
//! # Internationalisation
//!
//! The `account_amount_limit_notice` returned by the server is a
//! Traditional-Chinese banner. WPF detects the substring `"進階認證"`
//! and replaces the whole string with a localised resource lookup, and
//! otherwise runs the text through `I18n.ToSimplified()`. Both of
//! those concerns are presentation-layer responsibilities (the service
//! layer doesn't know the user's UI language), so we surface a typed
//! [`AmountLimitNotice`] instead — the UI layer can branch on
//! `AuthReLoginRequired` and pass any `Other(s)` text through its own
//! i18n pipeline.
//!
//! # Wire-level divergences from WPF (semantically inert)
//!
//! - **`Accept-Encoding`**: WPF sends `identity` for `DownloadString` /
//!   `UploadString` and `gzip, deflate, br` for `UploadStringGZip`. We
//!   let `reqwest` advertise `gzip, deflate` automatically (driven by
//!   the `gzip` / `deflate` cargo features) on every request. The
//!   server picks an encoding it understands and `reqwest` transparently
//!   inflates — net body content is identical to WPF.
//! - **`Accept: */*`**: `reqwest` sends this on every request; WPF's
//!   `WebClient` does not. The server's response is unaffected.

use crate::core::parser::{
    extract_account_limit_notice, extract_service_account_create_time, extract_service_accounts,
};
use crate::core::time::dt_compact_now;
use serde::Deserialize;

use super::client::BeanfunClient;
use super::error::LoginError;
use super::login::ensure_success;
use super::session::Session;

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// One row from the user's service-account list, plus the few extra
/// fields WPF carries on the equivalent C# class.
///
/// Field names mirror the legacy `BeanfunClient.ServiceAccount` C# class
/// verbatim (`sid` / `ssn` / `sname` / `screatetime` / …) so grep-replace
/// from the old code base lands cleanly. The `Option` types reflect WPF's
/// nullable `string` fields (the constructor used inside `GetAccounts`
/// leaves `slastusedtime` / `sauthtype` `null`, and `screatetime` becomes
/// `null` whenever the per-row `GetCreateTime` HTTP call fails).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAccount {
    /// `true` when the row's anchor has a non-empty `onclick` handler
    /// (WPF: `match.Groups[1].Value != ""`). Disabled accounts still
    /// show in the UI but cannot be launched.
    pub is_enable: bool,
    /// WPF default `true`. Always set by [`get_accounts`] today; the
    /// field exists for parity with WPF's two-arg constructor used
    /// elsewhere in the legacy code base.
    pub visible: bool,
    /// WPF default `false`. As above.
    pub is_inherited: bool,
    /// Service-account id (the `<div id="…">` inner attribute).
    pub sid: String,
    /// Numeric serial number (`sn="…"`).
    pub ssn: String,
    /// Display name (`name="…"`) with HTML entities decoded by the
    /// underlying [`extract_service_accounts`] parser
    /// (matches WPF `WebUtility.HtmlDecode`).
    pub sname: String,
    /// Server-side creation timestamp scraped from the per-account
    /// `game_start_step2.aspx` page. `None` when the scrape fails — WPF
    /// returns `null` in that case (`GetCreateTime`'s `catch` block) and
    /// the OTP flow tolerates `null` here (it re-fetches if needed).
    pub screatetime: Option<String>,
    /// WPF default `null` — never populated by `GetAccounts`.
    /// Reserved for future flows that bring it in (e.g. `last_used_at`
    /// from a separate management endpoint).
    pub slastusedtime: Option<String>,
    /// WPF default `null` — never populated by `GetAccounts`.
    pub sauthtype: Option<String>,
}

/// Server-side notice shown when the user has hit the account quota.
///
/// WPF stuffs the localised text directly into a UI string (`I18n.ToSimplified`
/// / `TryFindResource("AuthReLogin")`). We keep the service layer i18n-free
/// and let the UI choose what to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmountLimitNotice {
    /// No `divServiceAccountAmountLimitNotice` element on the page.
    None,
    /// The notice contained the substring `"進階認證"` — WPF treats this
    /// as a sentinel for "user must complete advance verification before
    /// they can add more accounts" and shows a fixed `AuthReLogin`
    /// resource string. Carries no payload because the original text is
    /// irrelevant once classified.
    AuthReLoginRequired,
    /// Any other notice text. Carries the raw, **Traditional Chinese**
    /// string verbatim from the server — the UI layer may run it through
    /// a simplified-Chinese converter for HK users (matching WPF
    /// `I18n.ToSimplified`) or display as-is.
    Other(String),
}

/// Result of [`get_accounts`]: the sorted account list plus the optional
/// quota notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountListResult {
    /// Service accounts sorted by ascending `ssn` (WPF
    /// `accountList.Sort((x, y) => x.ssn.CompareTo(y.ssn))`). Callers
    /// that want a different order — e.g. user-defined drag-and-drop
    /// from persistent storage — should layer that transformation on
    /// top.
    pub accounts: Vec<ServiceAccount>,
    /// Server-side quota notice, classified into a typed
    /// [`AmountLimitNotice`] so callers can dispatch without string
    /// comparisons.
    pub amount_limit_notice: AmountLimitNotice,
}

// -----------------------------------------------------------------------------
// Public functions
// -----------------------------------------------------------------------------

/// List the service accounts the logged-in user can launch into the
/// given service / region.
///
/// Mirrors `BeanfunClient.Account.cs::GetAccounts` (L65-143):
///
/// 1. `GET auth.aspx?channel=game_zone&page_and_query=…&web_token=…`
///    purely for cookie side-effects (response discarded; matches WPF
///    "this.DownloadString(...)" with no return capture).
/// 2. `GET game_zone/game_server_account_list.aspx?sc=&sr=&dt=…` —
///    the actual list page, parsed for rows + the optional quota notice.
/// 3. For each row, fire `get_create_time` to scrape that account's
///    creation timestamp. Any individual failure is silenced to `None`
///    on that row's `screatetime` (matching WPF `GetCreateTime`'s
///    `catch { return null; }`).
/// 4. Sort the rows by ascending `ssn` (WPF first-pass sort).
///
/// # Errors
///
/// - [`LoginError::Http`] on a transport / network failure during steps
///   1 or 2.
/// - [`LoginError::Unknown`] when step 1 or step 2 returns a non-2xx.
/// - [`LoginError::BodyTooLarge`] if the list page exceeds
///   [`super::ClientConfig::max_body_size`].
///
/// Per-row `get_create_time` failures are **not** surfaced — they
/// degrade to `None` on that account's `screatetime` field, matching
/// WPF's silent fallback. The list itself is still returned.
pub async fn get_accounts(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
) -> Result<AccountListResult, LoginError> {
    auth_aspx(client, session, service_code, service_region).await?;
    let body = fetch_account_list_html(client, service_code, service_region).await?;

    let mut accounts: Vec<ServiceAccount> = Vec::new();
    for row in extract_service_accounts(&body) {
        let screatetime = get_create_time(client, service_code, service_region, &row.ssn).await;
        accounts.push(ServiceAccount {
            is_enable: row.is_enable,
            visible: true,
            is_inherited: false,
            sid: row.sid,
            ssn: row.ssn,
            sname: row.sname,
            screatetime,
            slastusedtime: None,
            sauthtype: None,
        });
    }

    // WPF: `accountList.Sort((x, y) => x.ssn.CompareTo(y.ssn));`
    accounts.sort_by(|a, b| a.ssn.cmp(&b.ssn));

    let amount_limit_notice = classify_amount_limit_notice(&body);

    Ok(AccountListResult {
        accounts,
        amount_limit_notice,
    })
}

/// Fetch the `GetServiceContract` HTML body for a given service / region
/// (the EULA / ToS shown before account creation).
///
/// Mirrors `BeanfunClient.Account.cs::GetServiceContract` (L669-686):
///
/// - `POST gamezone.ashx` form `strFunction=GetServiceContract`, `sc`, `sr`.
/// - On empty response body: returns `Ok(String::new())` (WPF returns
///   `""`).
/// - On `intResult != 1` (or missing `intResult`): returns
///   `Ok(String::new())` (WPF same).
/// - Otherwise: returns the JSON `strResult` field.
///
/// # Errors
///
/// - [`LoginError::Http`] on transport failure.
/// - [`LoginError::Json`] when the response body is non-empty but not
///   valid JSON (WPF `JObject.Parse` would throw a `JsonReaderException`
///   here, which `MainWindow` catches via the outer try/catch).
pub async fn get_service_contract(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
) -> Result<String, LoginError> {
    let body = post_gamezone(
        client,
        session,
        &[
            ("strFunction", "GetServiceContract"),
            ("sc", service_code),
            ("sr", service_region),
        ],
    )
    .await?;

    if body.is_empty() {
        return Ok(String::new());
    }

    let parsed: GamezoneContractResponse = serde_json::from_str(&body)?;
    if parsed.int_result != Some(1) {
        return Ok(String::new());
    }
    Ok(parsed.str_result.unwrap_or_default())
}

/// Create a new service account under the given `service_code` /
/// `service_region`.
///
/// Mirrors `BeanfunClient.Account.cs::AddServiceAccount` (L614-638):
///
/// - On empty `name`: returns `Ok(false)` *without firing the request*
///   (matches WPF early-return).
/// - On `POST gamezone.ashx` form
///   `strFunction=AddServiceAccount, npsc=, npsr=, sc=, sr=, sadn=name, sag=`
///   :
///     - empty body → `Ok(false)` (WPF same).
///     - `intResult != 1` (or missing) → `Ok(false)` (WPF same).
///     - `intResult == 1` → `Ok(true)`.
///
/// # Errors
///
/// - [`LoginError::Http`] on transport failure (WPF would throw
///   `WebException`).
/// - [`LoginError::Json`] on JSON parse failure (WPF would throw
///   `JsonReaderException`).
pub async fn add_service_account(
    client: &BeanfunClient,
    session: &Session,
    name: &str,
    service_code: &str,
    service_region: &str,
) -> Result<bool, LoginError> {
    if name.is_empty() {
        return Ok(false);
    }

    let body = post_gamezone(
        client,
        session,
        &[
            ("strFunction", "AddServiceAccount"),
            ("npsc", ""),
            ("npsr", ""),
            ("sc", service_code),
            ("sr", service_region),
            ("sadn", name),
            ("sag", ""),
        ],
    )
    .await?;

    parse_int_result_eq_one(&body)
}

/// Rename an existing service account.
///
/// Mirrors `BeanfunClient.Account.cs::ChangeServiceAccountDisplayName`
/// (L640-667). WPF's signature takes the whole `ServiceAccount` so the
/// call site can early-out on `newName == account.sname`; we mirror
/// that exactly.
///
/// - On empty `new_name` **or** `new_name == account.sname`: returns
///   `Ok(false)` *without firing the request*.
/// - On `POST gamezone.ashx` form
///   `strFunction=ChangeServiceAccountDisplayName, sl=game_code, said=account.sid, nsadn=new_name`
///   :
///     - empty body → `Ok(false)`.
///     - `intResult != 1` (or missing) → `Ok(false)`.
///     - `intResult == 1` → `Ok(true)`.
///
/// `game_code` is the canonical `"{sc}_{sr}"` string the UI carries
/// (WPF builds it inline at the call site too).
///
/// # Errors
///
/// As for [`add_service_account`].
pub async fn change_service_account_display_name(
    client: &BeanfunClient,
    session: &Session,
    new_name: &str,
    game_code: &str,
    account: &ServiceAccount,
) -> Result<bool, LoginError> {
    if new_name.is_empty() || new_name == account.sname {
        return Ok(false);
    }

    let body = post_gamezone(
        client,
        session,
        &[
            ("strFunction", "ChangeServiceAccountDisplayName"),
            ("sl", game_code),
            ("said", &account.sid),
            ("nsadn", new_name),
        ],
    )
    .await?;

    parse_int_result_eq_one(&body)
}

// -----------------------------------------------------------------------------
// Private helpers
// -----------------------------------------------------------------------------

/// Fire `auth.aspx?channel=game_zone&page_and_query=…&web_token=…` and
/// discard the body (WPF L78-80 does the same — the call exists purely
/// for cookie side-effects on the portal host).
async fn auth_aspx(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
) -> Result<(), LoginError> {
    let url = client.portal_url("beanfun_block/auth.aspx")?;
    // The inner string passed as `page_and_query` is itself a relative
    // URL; reqwest's `.query()` URL-encodes it for us, producing the
    // exact `%3F` / `%3D` byte sequence WPF builds inline.
    let inner = format!("game_start.aspx?service_code_and_region={service_code}_{service_region}");
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("channel", "game_zone"),
            ("page_and_query", inner.as_str()),
            ("web_token", session.web_token.as_str()),
        ])
        .send()
        .await?;
    ensure_success(&resp, "auth.aspx")?;
    // Body intentionally not consumed: WPF discards it too.
    Ok(())
}

/// `GET game_zone/game_server_account_list.aspx?sc=&sr=&dt=…` and
/// return the response body. Caller parses it.
async fn fetch_account_list_html(
    client: &BeanfunClient,
    service_code: &str,
    service_region: &str,
) -> Result<String, LoginError> {
    let url = client.portal_url("beanfun_block/game_zone/game_server_account_list.aspx")?;
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("sc", service_code),
            ("sr", service_region),
            ("dt", dt_compact_now().as_str()),
        ])
        .send()
        .await?;
    ensure_success(&resp, "game_server_account_list.aspx")?;
    client.bounded_text(resp).await
}

/// Scrape the `ServiceAccountCreateTime` literal from a single
/// account's `game_start_step2.aspx` page.
///
/// **Errors are silenced.** WPF's `GetCreateTime` wraps the entire body
/// in `try { … } catch { return null; }` (L147-171), so any HTTP
/// failure, parse failure, or empty match degrades to `None` here. The
/// per-row screatetime field stays `None` and the OTP flow tolerates
/// that.
async fn get_create_time(
    client: &BeanfunClient,
    service_code: &str,
    service_region: &str,
    sn: &str,
) -> Option<String> {
    let url = client
        .portal_url("beanfun_block/game_zone/game_start_step2.aspx")
        .ok()?;
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("service_code", service_code),
            ("service_region", service_region),
            ("sotp", sn),
            ("dt", dt_compact_now().as_str()),
        ])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = client.bounded_text(resp).await.ok()?;
    extract_service_account_create_time(&body)
}

/// Build, send, and read-as-text a POST to
/// `generic_handlers/gamezone.ashx` with a form body. Returns the raw
/// response body (caller parses JSON / classifies).
///
/// The `_session` parameter is currently unused — gamezone.ashx
/// authenticates via the bfWebToken cookie that's already on the jar
/// from the login flow — but we keep it on every public function's
/// signature for consistency, and to lock in the "all account calls
/// require an active session" contract at the type level.
async fn post_gamezone(
    client: &BeanfunClient,
    _session: &Session,
    form: &[(&str, &str)],
) -> Result<String, LoginError> {
    let url = client.portal_url("generic_handlers/gamezone.ashx")?;
    let resp = client.http().post(url).form(form).send().await?;
    ensure_success(&resp, "gamezone.ashx")?;
    client.bounded_text(resp).await
}

/// Parse the gamezone JSON envelope and return `true` iff `intResult`
/// is exactly `1`.
///
/// Returns `Ok(false)` when:
/// - `body` is empty (WPF early-returns `false` on empty response).
/// - `intResult` is missing or any value other than `1`.
///
/// Returns `Err(LoginError::Json)` only when the body is non-empty and
/// not valid JSON — matches WPF's `JObject.Parse` throw behaviour.
fn parse_int_result_eq_one(body: &str) -> Result<bool, LoginError> {
    if body.is_empty() {
        return Ok(false);
    }
    let env: GamezoneIntResultResponse = serde_json::from_str(body)?;
    Ok(env.int_result == Some(1))
}

/// Classify the optional `divServiceAccountAmountLimitNotice` text into
/// a typed [`AmountLimitNotice`]. Pure function over the parser output.
fn classify_amount_limit_notice(body: &str) -> AmountLimitNotice {
    match extract_account_limit_notice(body) {
        None => AmountLimitNotice::None,
        Some(text) if text.contains("進階認證") => AmountLimitNotice::AuthReLoginRequired,
        Some(text) => AmountLimitNotice::Other(text),
    }
}

// -----------------------------------------------------------------------------
// JSON envelope deserialisers
// -----------------------------------------------------------------------------

/// Envelope returned by gamezone.ashx for the boolean-result endpoints
/// (`AddServiceAccount`, `ChangeServiceAccountDisplayName`).
///
/// Only `intResult` is read; the field is `Option<i64>` so a payload
/// that omits it deserialises into `None` (matching WPF's
/// `jsonData["intResult"] == null` check).
#[derive(Debug, Deserialize)]
struct GamezoneIntResultResponse {
    #[serde(rename = "intResult")]
    int_result: Option<i64>,
}

/// Envelope returned by gamezone.ashx for `GetServiceContract`. Carries
/// both `intResult` (gate) and `strResult` (payload).
#[derive(Debug, Deserialize)]
struct GamezoneContractResponse {
    #[serde(rename = "intResult")]
    int_result: Option<i64>,
    #[serde(rename = "strResult")]
    str_result: Option<String>,
}

// -----------------------------------------------------------------------------
// Tests — pure helpers only. End-to-end HTTP coverage lives in
// `tests/account.rs`.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // classify_amount_limit_notice
    // -------------------------------------------------------------------------

    #[test]
    fn classify_amount_limit_notice_absent_is_none() {
        assert_eq!(
            classify_amount_limit_notice("<html>nothing here</html>"),
            AmountLimitNotice::None
        );
    }

    #[test]
    fn classify_amount_limit_notice_with_advance_auth_keyword_is_auth_re_login() {
        let html = r#"<div id="divServiceAccountAmountLimitNotice" class="InnerContent">您必須完成進階認證才能新增帳號。</div>"#;
        assert_eq!(
            classify_amount_limit_notice(html),
            AmountLimitNotice::AuthReLoginRequired
        );
    }

    #[test]
    fn classify_amount_limit_notice_other_text_preserved_verbatim() {
        let html = r#"<div id="divServiceAccountAmountLimitNotice" class="InnerContent">已達 5 個服務帳號上限。</div>"#;
        assert_eq!(
            classify_amount_limit_notice(html),
            AmountLimitNotice::Other("已達 5 個服務帳號上限。".to_owned())
        );
    }

    /// Substring match must trigger even when the notice text contains
    /// surrounding words. WPF uses `notice.Contains("進階認證")`, which
    /// is the same semantics.
    #[test]
    fn classify_amount_limit_notice_substring_match_anywhere_in_text() {
        let html = r#"<div id="divServiceAccountAmountLimitNotice" class="InnerContent">PREFIX 進階認證 SUFFIX</div>"#;
        assert_eq!(
            classify_amount_limit_notice(html),
            AmountLimitNotice::AuthReLoginRequired
        );
    }

    // -------------------------------------------------------------------------
    // parse_int_result_eq_one
    // -------------------------------------------------------------------------

    #[test]
    fn parse_int_result_empty_body_is_false_no_json_call() {
        assert!(!parse_int_result_eq_one("").unwrap());
    }

    #[test]
    fn parse_int_result_one_is_true() {
        assert!(parse_int_result_eq_one(r#"{"intResult":1}"#).unwrap());
    }

    #[test]
    fn parse_int_result_zero_is_false() {
        assert!(!parse_int_result_eq_one(r#"{"intResult":0}"#).unwrap());
    }

    #[test]
    fn parse_int_result_missing_field_is_false() {
        assert!(!parse_int_result_eq_one(r#"{"other":"value"}"#).unwrap());
    }

    /// WPF treats null `intResult` as "not 1" (the explicit
    /// `jsonData["intResult"] == null` short-circuit at L634). Our
    /// `Option<i64>` deserialises JSON `null` into `None` for the same
    /// outcome.
    #[test]
    fn parse_int_result_null_is_false() {
        assert!(!parse_int_result_eq_one(r#"{"intResult":null}"#).unwrap());
    }

    #[test]
    fn parse_int_result_other_positive_int_is_false() {
        assert!(!parse_int_result_eq_one(r#"{"intResult":2}"#).unwrap());
    }

    #[test]
    fn parse_int_result_invalid_json_returns_err() {
        let err = parse_int_result_eq_one("not json").unwrap_err();
        assert!(matches!(err, LoginError::Json(_)));
    }
}
