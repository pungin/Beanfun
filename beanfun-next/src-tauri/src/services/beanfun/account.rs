//! Account-management surface: list service accounts, fetch contracts,
//! create / rename them via the gamezone JSON handler, and drive the
//! WebForms-style add-account / change-password dialogs.
//!
//! Ports `BeanfunClient.Account.cs` (chunks 4.1 + 4.4 of P4):
//!
//! | This module                                     | WPF reference (`Account.cs`)                   |
//! |-------------------------------------------------|------------------------------------------------|
//! | [`get_accounts`]                                | `GetAccounts`                                  |
//! | `get_create_time` (private helper)              | `GetCreateTime`                                |
//! | [`get_service_contract`]                        | `GetServiceContract`                           |
//! | [`get_email`]                                   | `getEmail` (TW only; HK short-circuits empty)  |
//! | [`get_remain_point`]                            | `getRemainPoint`                               |
//! | [`add_service_account`]                         | `AddServiceAccount`                            |
//! | [`change_service_account_display_name`]         | `ChangeServiceAccountDisplayName`              |
//! | [`unconnected_game_init_add_account_payload`]   | `UnconnectedGame_InitAddAccountPayload` (+ private `_InitAccountPayload` helper) |
//! | [`unconnected_game_add_account_check`]          | `UnconnectedGame_AddAccountCheck`              |
//! | [`unconnected_game_add_account_check_nickname`] | `UnconnectedGame_AddAccountCheckNickName`      |
//! | [`unconnected_game_add_account`]                | `UnconnectedGame_AddAccount`                   |
//! | [`unconnected_game_change_password`]            | `UnconnectedGame_ChangePassword`               |
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
//!
//! # WPF deviation candidate (P4.4) — HK `change_password` uses `http://`
//!
//! `BeanfunClient.Account.cs::UnconnectedGame_ChangePassword` L549-555
//! and L597-600 reach **three** HK steps with the literal `http://`
//! scheme: the `01Accounts.aspx` POST (step 3), the `03.aspx` GET
//! (step 4), and the `03.aspx` POST (step 5). Every other HK code
//! path in the same file (including the immediately preceding step 1
//! `auth.aspx` GET and step 2 `01Accounts.aspx` GET) uses `https://`.
//! This three-line gap looks like an upstream typo, but the server's
//! actual behaviour against an HTTP request is unverified from our
//! side: it may reply `301 → https://...` (in which case `reqwest`'s
//! default redirect policy follows, end-state identical to HTTPS),
//! or it may accept the HTTP request directly (in which case the
//! cookies travel in plaintext once).
//!
//! Per the project's "1:1 functional alignment with WPF" rule, our
//! port sends the same `http://` scheme on those three HK steps.
//! This doc comment is the trace; if the P10 security review
//! concludes "must be HTTPS", flip the scheme in
//! `change_password_url` (the single helper that gates all three
//! sites) and add a regression test there.
//!
//! # WPF deviation (P4.4) — `verify_code` extraction shape
//!
//! `BeanfunClient.Account.cs::UnconnectedGame_ChangePassword` L608-611
//! does:
//!
//! ```csharp
//! regex = new Regex("verify_code=(.*)");
//! return regex.IsMatch(this.ResponseUri.ToString())
//!     ? ("verify_code" + regex.Match(...).Groups[1].Value)
//!     : null;
//! ```
//!
//! and `UnconnectedGame_ChangePassword.xaml.cs` L30-35 then does
//! `result.StartsWith("verify_code")` + `result.Replace("verify_code", "")`
//! to recover the bare token before showing it to the user.
//!
//! Two design choices in that snippet are **not** business-relevant
//! and we deliberately do not mirror them byte-for-byte:
//!
//! 1. **The literal `"verify_code"` prefix** is a sentinel
//!    discriminator — WPF's return type is `string`, so the only way to
//!    pack three outcomes (`null` / `lblErrorMessage` / success-with-token)
//!    into one return is to brand the success path with a magic prefix
//!    the caller can `StartsWith` on. We split outcomes at the type
//!    level via [`ChangePasswordOutcome`], so the prefix has no place
//!    on the wire-equivalent surface and our `ChangePasswordOutcome::VerifyCodeSent`
//!    variant carries the **bare token**.
//!
//! 2. **The greedy `(.*)` capture** is the lazy-regex equivalent of
//!    "everything from `verify_code=` to end of string", because
//!    C#'s `Uri` doesn't expose a structured query parser without
//!    pulling in `HttpUtility`. We use a **bounded `verify_code=([^&]*)`**
//!    regex that terminates at the next `&` (matching how a real query
//!    parser would tokenise the URL). The two diverge only when the
//!    redirect URL has `verify_code=` followed by another `&`-delimited
//!    parameter or a `#` fragment — in that case WPF would surface the
//!    trailing junk concatenated to the token (and the user would
//!    presumably need to manually strip it before pasting), while we
//!    surface the clean token. Real-world Beanfun appears to emit
//!    `?verify_code=<token>` as the sole / final query parameter so
//!    both implementations produce identical output in practice; the
//!    bounded regex is the strictly safer default.
//!
//! If a future audit demands strict WPF byte-equivalence here, switch
//! `verify_code_regex()` back to `verify_code=(.*)` and update the
//! corresponding unit test (`extract_verify_code_from_url_with_extra_query_terminates_at_ampersand`).

use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::core::parser::{
    capture_first, extract_account_limit_notice, extract_service_account_create_time,
    extract_service_accounts, extract_viewstate,
};
use crate::core::time::dt_compact_now;

use super::client::{BeanfunClient, LoginRegion};
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
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

/// Fetch the logged-in user's e-mail address (TW only).
///
/// Mirrors `BeanfunClient.cs::getEmail` (L243-259):
///
/// 1. If `session.region == HK`: return `Ok("")` immediately without
///    firing a request — the HK portal does not expose this endpoint
///    and WPF short-circuits the same way.
/// 2. Otherwise: `GET https://tw.beanfun.com/beanfun_block/loader.ashx?service_code=999999&service_region=T0`
///    with `Referer: https://tw.beanfun.com/`.
/// 3. Regex-match
///    `BeanFunBlock.LoggedInUserData.Email = "(.*)";BeanFunBlock.LoggedInUserData.MessageCount`
///    on the body and return the captured group.
/// 4. If the regex does not match: return `Ok("")` (WPF same).
///
/// # Why no HK endpoint?
///
/// WPF's `getEmail` hard-codes `tw.beanfun.com` and explicitly short-
/// circuits on HK — there is no HK equivalent of the TW loader page
/// that exposes the e-mail in the JavaScript payload. The empty-
/// string return is the WPF contract for HK callers; the UI layer
/// hides the "e-mail" row when the call returns empty anyway (see
/// `AccountList.xaml.cs` L204-214 → `m_GetEmail_Click`).
///
/// # Errors
///
/// - [`LoginError::Http`] on transport failure (WPF swallows this as
///   the return-value becomes `""`; we surface the error so higher
///   layers can log / retry — the command-layer wrapper can map back
///   to `""` if WPF-exact behaviour is required).
/// - [`LoginError::BodyTooLarge`] if the loader page exceeds the
///   configured cap (unlikely in practice — WPF never encountered
///   this, but our bounded reader is a defensive layer).
pub async fn get_email(client: &BeanfunClient, session: &Session) -> Result<String, LoginError> {
    if session.region == LoginRegion::HK {
        return Ok(String::new());
    }

    let url = client.portal_url("beanfun_block/loader.ashx")?;
    let referer = client.config().endpoints.portal_base.as_str().to_owned();
    let resp = client
        .http()
        .get(url)
        .query(&[("service_code", "999999"), ("service_region", "T0")])
        .header(reqwest::header::REFERER, referer)
        .send()
        .await?;
    ensure_success(&resp, "loader.ashx (get_email)")?;
    let body = client.bounded_text(resp).await?;

    Ok(capture_first(email_regex(), &body).unwrap_or_default())
}

/// Fetch the remaining Beanfun points balance for the current session.
///
/// Mirrors `BeanfunClient.cs::getRemainPoint` (L214-241):
///
/// 1. `GET {portal_base}beanfun_block/generic_handlers/get_remain_point.ashx?webtoken=1`
///    — no custom headers, the `bfWebToken` cookie comes from the
///    jar automatically.
/// 2. Regex-match `"RemainPoint" : "(.*)" }` (note the surrounding
///    spaces — WPF's literal pattern) and parse the capture as a
///    signed 32-bit integer.
/// 3. Return `Ok(0)` when the regex does not match **or** the capture
///    fails to parse — WPF wraps both paths in a blanket `catch {
///    return 0; }`.
///
/// # Why the exact regex shape?
///
/// The server emits the JSON with a single space on either side of
/// the colon (`"RemainPoint" : "1234" }`). WPF treats the shape as
/// a fingerprint and anchors with the literal-space pattern; we
/// preserve the spacing so any server-side change to the layout
/// would fail our test suite the same way it would fail WPF — and
/// deliberately so, since the server-shaped regex is the only
/// indicator we have that the endpoint still speaks the expected
/// dialect.
///
/// # Errors
///
/// - [`LoginError::Http`] on transport failure. (WPF's blanket catch
///   would treat this as `0`; we surface the error so the command
///   layer can log. If strict WPF parity is required, the command
///   wrapper maps `Err` back to `0`.)
/// - [`LoginError::BodyTooLarge`] if the payload exceeds the
///   configured cap.
pub async fn get_remain_point(
    client: &BeanfunClient,
    _session: &Session,
) -> Result<i32, LoginError> {
    let url = client.portal_url("beanfun_block/generic_handlers/get_remain_point.ashx")?;
    let resp = client
        .http()
        .get(url)
        .query(&[("webtoken", "1")])
        .send()
        .await?;
    ensure_success(&resp, "get_remain_point.ashx")?;
    let body = client.bounded_text(resp).await?;

    Ok(capture_first(remain_point_regex(), &body)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0))
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

// =============================================================================
// P4.4 — WebForms account-management surface
//
// Below this line: types, public functions, and private helpers that port the
// `UnconnectedGame_*` WebForms flow (add-account dialog and change-password
// dialog). The P4.1 JSON / read surface above stays untouched.
// =============================================================================

// -----------------------------------------------------------------------------
// P4.4 — Public types
// -----------------------------------------------------------------------------

/// Round-trippable view-state triplet that the add-account dialog
/// threads through three POSTs to `02.aspx`
/// (`init_add_account_payload` → `add_account_check[_nickname]` →
/// `add_account`).
///
/// WPF stuffs these three strings into a mutable `NameValueCollection`
/// that the UI mutates between calls. We package them as an owned
/// struct so the service layer is the sole authority on what gets
/// posted: the caller can store / pass it around but cannot accidentally
/// inject extra fields. The HK-only `__VIEWSTATEENCRYPTED` empty-string
/// field is materialised by `build_viewstate_payload_prefix` off
/// `region`, so callers don't need to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddAccountSession {
    /// `__VIEWSTATE` value parsed from the most recent `02.aspx`
    /// response (or the initial `auth.aspx → 02.aspx` GET / POST pair
    /// for the very first session).
    pub viewstate: String,
    /// `__VIEWSTATEGENERATOR`. WPF treats this as required for the
    /// account-management pages (unlike the verify flow, which makes
    /// it optional).
    pub viewstate_generator: String,
    /// `__EVENTVALIDATION`. Always required after the first `02.aspx`
    /// POST returns it.
    pub event_validation: String,
    /// Captured at session-creation time so `build_viewstate_payload_prefix`
    /// knows whether to splice in the HK-only `__VIEWSTATEENCRYPTED`
    /// field. We snapshot here rather than re-reading
    /// `client.config().region` so the session can be safely held across
    /// region changes (purely defensive — production never swaps regions
    /// mid-session).
    pub region: LoginRegion,
}

/// Initial state returned by [`unconnected_game_init_add_account_payload`].
///
/// The `session` field round-trips through the rest of the add-account
/// flow; the other three are one-shot UI metadata WPF stuffs into the
/// dialog header (game title, length range, optional nickname-check
/// button). They live on this struct (not on the session) precisely
/// because they are *not* threaded through subsequent POSTs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddAccountInit {
    /// View-state triplet for the next POST (`add_account_check` or
    /// `add_account`). Caller stores and re-passes verbatim.
    pub session: AddAccountSession,
    /// `<span id="lblGameName">…</span>` content — game title shown in
    /// the dialog header (e.g. "新楓之谷").
    pub game_name: String,
    /// `<span id="lblAccountLen">…</span>` content — account-id length
    /// range as a hyphen-separated string (e.g. `"6 - 12"`). The dialog
    /// uses this for client-side length validation; we surface it
    /// verbatim because the format is server-controlled.
    pub account_len: String,
    /// Whether the page rendered a "check nickname" hyperlink (WPF
    /// L283: `response.Contains("<a id=\"lbtnCheckNickName\"")`).
    /// `false` ⇒ caller hides the nickname row, matching
    /// `UnconnectedGame_AddAccount.xaml.cs` L32-37.
    pub check_nickname_supported: bool,
}

/// Outcome of either [`unconnected_game_add_account_check`] or
/// [`unconnected_game_add_account_check_nickname`]: the POST always
/// succeeds at the HTTP level (server returns 200 with a fresh page),
/// and the result is the *next* view-state triplet plus the optional
/// `lblErrorMessage` text the page surfaces.
///
/// `error_message == ""` matches the WPF "passed the check" branch
/// (`UnconnectedGame_AddAccount.xaml.cs` L61 / L83 treat empty as the
/// "unknown error" sentinel after a `null` payload short-circuit, but
/// any **populated** value is shown verbatim to the user). We preserve
/// the same shape so callers can re-use WPF's branching logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    /// Refreshed view-state triplet from the response — caller passes
    /// this back into the next call (`add_account` or another check).
    pub session: AddAccountSession,
    /// `<span id="lblErrorMessage">…</span>` content from the response,
    /// empty when the span is absent. WPF passes this string straight
    /// to `lblErrorMessage.Content` in the dialog, so we pass through
    /// the server text verbatim (no i18n / classification).
    pub error_message: String,
}

/// Outcome of [`unconnected_game_add_account`].
///
/// WPF returns a `string` where `""` means success and any non-empty
/// value is `lblErrorMessage` text. Our enum makes the two paths
/// mutually exclusive at the type level so callers cannot
/// accidentally branch on the wrong condition.
///
/// The `null` return path WPF uses for early-validation failures
/// (empty name / pwd) is **not** a valid runtime outcome here — the
/// public function rejects those inputs with `Err(LoginError::Unknown(_))`
/// instead, so that callers can distinguish "user submitted empty
/// fields" from "server said no" without nesting `Option<…>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddAccountOutcome {
    /// Server accepted the submission (WPF: `result == ""` at
    /// `UnconnectedGame_AddAccount.xaml.cs` L221).
    Success,
    /// Server rejected with a displayable message (WPF: `result != "" && result != null`).
    /// Carries the verbatim `lblErrorMessage` content.
    ErrorMessage(String),
}

/// Outcome of [`unconnected_game_change_password`].
///
/// The 5-step flow ends with one of:
/// - server emitting a `verify_code=<token>` query parameter on the
///   final redirect URL (success path — caller surfaces the token to
///   the user so they can paste it into the Beanfun verify dialog),
/// - server rendering a non-empty `lblErrorMessage` span (rejection),
/// - both signals absent (catch-all, WPF returns `null` and the UI
///   shows a generic "UnknownError"; we surface this as
///   `Err(LoginError::Unknown(_))`).
///
/// The `verify_code` token we carry in [`Self::VerifyCodeSent`] is the
/// **content after `verify_code=`** terminated at the next `&` — i.e.
/// exactly what the WPF dialog ends up displaying after its
/// `result.Replace("verify_code", "")` strip
/// (`UnconnectedGame_ChangePassword.xaml.cs` L30-35). See the
/// "WPF deviation: `verify_code` extraction shape" section in the
/// module docs for why we drop WPF's sentinel-prefix + greedy-regex
/// shape rather than mirroring it on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePasswordOutcome {
    /// Server confirmed the password-reset request and emitted a
    /// verification token. Carries the token (without the
    /// `verify_code=` prefix).
    VerifyCodeSent(String),
    /// Server rejected the submission with a displayable
    /// `lblErrorMessage` body.
    ErrorMessage(String),
}

// -----------------------------------------------------------------------------
// P4.4 — Public functions
// -----------------------------------------------------------------------------

/// Open the add-account dialog: GET `auth.aspx?channel=accounts_management…`
/// to seed cookies and parse the initial view-state, then POST `02.aspx`
/// (with `imgbtn_AddAccount.x/y=0`) to render the AddAccount form.
///
/// Returns the parsed [`AddAccountInit`] (game name, account-length
/// range, `check_nickname_supported` flag, plus the round-trippable
/// [`AddAccountSession`]).
///
/// Mirrors `BeanfunClient.Account.cs::UnconnectedGame_InitAddAccountPayload`
/// (L211-287).
///
/// # Errors
///
/// - [`LoginError::AccountMgmtMissingViewState`] /
///   [`LoginError::AccountMgmtMissingViewStateGenerator`] from the GET
///   step (WPF L191-201).
/// - All five `AccountMgmtMissing*` variants from the POST step
///   (`__VIEWSTATE`, `__VIEWSTATEGENERATOR`, `__EVENTVALIDATION`,
///   `lblGameName`, `lblAccountLen`).
/// - [`LoginError::Http`] / [`LoginError::Unknown`] for transport / non-2xx.
pub async fn unconnected_game_init_add_account_payload(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
) -> Result<AddAccountInit, LoginError> {
    let (viewstate, viewstate_generator) =
        init_account_payload(client, session, service_code, service_region).await?;

    let url = mgmt_url(client, "accounts_management/02.aspx")?;
    let form: Vec<(&str, String)> = vec![
        ("__VIEWSTATE", viewstate),
        ("__VIEWSTATEGENERATOR", viewstate_generator),
        ("__EVENTTARGET", String::new()),
        ("__EVENTARGUMENT", String::new()),
        ("imgbtn_AddAccount.x", "0".to_owned()),
        ("imgbtn_AddAccount.y", "0".to_owned()),
    ];
    let resp = client.http().post(url).form(&form).send().await?;
    ensure_success(&resp, "accounts_management/02.aspx (init POST)")?;
    let body = client.bounded_text(resp).await?;

    let session = parse_viewstate_triplet(client, &body)?;
    let game_name = capture_first(lbl_game_name_regex(), &body)
        .ok_or(LoginError::AccountMgmtMissingGameName)?;
    let account_len = capture_first(lbl_account_len_regex(), &body)
        .ok_or(LoginError::AccountMgmtMissingAccountLen)?;
    let check_nickname_supported = body.contains(r#"<a id="lbtnCheckNickName""#);

    Ok(AddAccountInit {
        session,
        game_name,
        account_len,
        check_nickname_supported,
    })
}

/// POST `02.aspx` with `__EVENTTARGET=lbtnCheckAccount` to ask the
/// server to validate the candidate account-id (and optional display
/// name) before final submission.
///
/// `account_dn` is the optional display-name field — `Some("")` and
/// `Some(non_empty)` both opt into the `t1` (TW) / `txtServiceAccountDN`
/// (HK) field, while `None` skips it entirely (matches WPF's
/// `txtServiceAccountDN != null` gate at L302).
///
/// Mirrors `BeanfunClient.Account.cs::UnconnectedGame_AddAccountCheck`
/// (L289-359). Shares the inner POST + parse loop with
/// [`unconnected_game_add_account_check_nickname`] via the
/// private `add_account_check_inner` helper.
///
/// # Errors
///
/// As for [`unconnected_game_init_add_account_payload`]'s POST step
/// (the same three `AccountMgmtMissing*` view-state variants).
pub async fn unconnected_game_add_account_check(
    client: &BeanfunClient,
    session: &Session,
    mgmt_session: &AddAccountSession,
    name: &str,
    account_dn: Option<&str>,
) -> Result<CheckOutcome, LoginError> {
    let _ = session;
    add_account_check_inner(client, mgmt_session, "lbtnCheckAccount", name, account_dn).await
}

/// POST `02.aspx` with `__EVENTTARGET=lbtnCheckNickName` to ask the
/// server to validate the display name (the account-id field is sent
/// empty for this endpoint, matching WPF L372).
///
/// Mirrors `BeanfunClient.Account.cs::UnconnectedGame_AddAccountCheckNickName`
/// (L361-430).
///
/// # Errors
///
/// As for [`unconnected_game_add_account_check`].
pub async fn unconnected_game_add_account_check_nickname(
    client: &BeanfunClient,
    session: &Session,
    mgmt_session: &AddAccountSession,
    account_dn: Option<&str>,
) -> Result<CheckOutcome, LoginError> {
    let _ = session;
    add_account_check_inner(client, mgmt_session, "lbtnCheckNickName", "", account_dn).await
}

/// POST `02.aspx` with the full add-account form (id + password ×2 +
/// optional display name + `chkBox1=on` + `imgbtn_Submit.x/y=0`) to
/// finalise account creation.
///
/// Returns [`AddAccountOutcome::Success`] when the response carries no
/// (or empty) `lblErrorMessage`, otherwise
/// [`AddAccountOutcome::ErrorMessage`] carrying the message text.
///
/// Mirrors `BeanfunClient.Account.cs::UnconnectedGame_AddAccount`
/// (L432-483).
///
/// # Errors
///
/// - [`LoginError::Unknown`] when any of `name` / `new_password` /
///   `new_password_confirm` is empty (WPF L442-447 returns `null`,
///   which the dialog renders as a generic "UnknownError"). We surface
///   the typed error so the dialog can pre-validate at the call site
///   too.
/// - [`LoginError::Http`] / [`LoginError::Unknown`] for transport /
///   non-2xx.
pub async fn unconnected_game_add_account(
    client: &BeanfunClient,
    session: &Session,
    mgmt_session: &AddAccountSession,
    name: &str,
    new_password: &str,
    new_password_confirm: &str,
    account_dn: Option<&str>,
) -> Result<AddAccountOutcome, LoginError> {
    let _ = session;
    if name.is_empty() {
        return Err(LoginError::Unknown(
            "add_account: account name is empty".into(),
        ));
    }
    if new_password.is_empty() {
        return Err(LoginError::Unknown(
            "add_account: new_password is empty".into(),
        ));
    }
    if new_password_confirm.is_empty() {
        return Err(LoginError::Unknown(
            "add_account: new_password_confirm is empty".into(),
        ));
    }

    let url = mgmt_url(client, "accounts_management/02.aspx")?;
    let form = build_add_account_form(
        mgmt_session,
        name,
        new_password,
        new_password_confirm,
        account_dn,
    );
    let resp = client.http().post(url).form(&form).send().await?;
    ensure_success(&resp, "accounts_management/02.aspx (add POST)")?;
    let body = client.bounded_text(resp).await?;

    let lbl = extract_lbl_error_message(&body);
    if lbl.is_empty() {
        Ok(AddAccountOutcome::Success)
    } else {
        Ok(AddAccountOutcome::ErrorMessage(lbl))
    }
}

/// Drive the 5-step change-password flow:
///
/// 1. GET `auth.aspx?channel=accounts_management…` (cookie seed,
///    discard view-state).
/// 2. GET `accounts_management/01Accounts.aspx` (parse view-state
///    triplet).
/// 3. POST `01Accounts.aspx` with
///    `__EVENTTARGET=gvServiceAccountList`, `__EVENTARGUMENT=ChangePassword$<num>`
///    (cookie seed, response discarded).
/// 4. GET `accounts_management/03.aspx` (parse view-state triplet).
/// 5. POST `03.aspx` with `txtEmail` + `imgbtn_Submit.x/y=0` (parse
///    final URL for `verify_code=…` or response body for
///    `lblErrorMessage`).
///
/// `num` is the row index inside `gvServiceAccountList` the user
/// clicked on (WPF passes `int`; we use `i32` to match — the WPF call
/// site is `MainWindow.xaml.cs::ResetPassword_Click`).
///
/// HK steps 3-5 use **`http://`** by design (not a typo — it's
/// what WPF does at `Account.cs` L549-555 / L597-600). See
/// "WPF deviation candidate" in the module docs for why we preserve
/// it. The `change_password_url` helper centralises the scheme switch.
///
/// Mirrors `BeanfunClient.Account.cs::UnconnectedGame_ChangePassword`
/// (L485-612).
///
/// # Errors
///
/// - All three `AccountMgmtMissing*` view-state variants (raised by
///   either of the two parse steps — step 2 or step 4).
/// - [`LoginError::Http`] / [`LoginError::Unknown`] for transport /
///   non-2xx on any of the five HTTP calls.
pub async fn unconnected_game_change_password(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
    num: i32,
    email: &str,
) -> Result<ChangePasswordOutcome, LoginError> {
    // Step 1 — discard return value (WPF L492 calls this purely for
    // its cookie side-effects).
    init_account_payload(client, session, service_code, service_region).await?;

    // Step 2 — GET 01Accounts.aspx and parse the triplet.
    let step2_url = mgmt_url(client, "accounts_management/01Accounts.aspx")?;
    let resp = client.http().get(step2_url).send().await?;
    ensure_success(&resp, "accounts_management/01Accounts.aspx (GET)")?;
    let body = client.bounded_text(resp).await?;
    let step2_session = parse_viewstate_triplet(client, &body)?;

    // Step 3 — POST 01Accounts.aspx (HK uses http://, see module docs).
    let step3_url = change_password_url(client, "accounts_management/01Accounts.aspx")?;
    let mut step3_form: Vec<(&str, String)> = Vec::new();
    build_viewstate_payload_prefix(&step2_session, &mut step3_form);
    step3_form.push(("__EVENTTARGET", "gvServiceAccountList".to_owned()));
    step3_form.push(("__EVENTARGUMENT", format!("ChangePassword${num}")));
    step3_form.push(("x", "0".to_owned()));
    step3_form.push(("y", "0".to_owned()));
    let resp = client
        .http()
        .post(step3_url)
        .form(&step3_form)
        .send()
        .await?;
    ensure_success(&resp, "accounts_management/01Accounts.aspx (POST)")?;
    // WPF L539-555 immediately overwrites this response by GETting
    // 03.aspx, so we deliberately do not consume the body either.
    drop(resp);

    // Step 4 — GET 03.aspx (HK uses http://) and parse the triplet.
    let step4_url = change_password_url(client, "accounts_management/03.aspx")?;
    let resp = client.http().get(step4_url).send().await?;
    ensure_success(&resp, "accounts_management/03.aspx (GET)")?;
    let body = client.bounded_text(resp).await?;
    let step4_session = parse_viewstate_triplet(client, &body)?;

    // Step 5 — POST 03.aspx (HK uses http://) and classify outcome.
    let step5_url = change_password_url(client, "accounts_management/03.aspx")?;
    let mut step5_form: Vec<(&str, String)> = Vec::new();
    build_viewstate_payload_prefix(&step4_session, &mut step5_form);
    step5_form.push(("txtEmail", email.to_owned()));
    step5_form.push(("imgbtn_Submit.x", "0".to_owned()));
    step5_form.push(("imgbtn_Submit.y", "0".to_owned()));
    let resp = client
        .http()
        .post(step5_url)
        .form(&step5_form)
        .send()
        .await?;
    ensure_success(&resp, "accounts_management/03.aspx (POST)")?;
    let final_url = resp.url().clone();
    let body = client.bounded_text(resp).await?;

    let lbl = extract_lbl_error_message(&body);
    if !lbl.is_empty() {
        return Ok(ChangePasswordOutcome::ErrorMessage(lbl));
    }
    if let Some(token) = extract_verify_code_from_url(&final_url) {
        return Ok(ChangePasswordOutcome::VerifyCodeSent(token));
    }
    Err(LoginError::Unknown(
        "change_password: response carried neither lblErrorMessage nor verify_code=…".into(),
    ))
}

// -----------------------------------------------------------------------------
// P4.4 — Private helpers
// -----------------------------------------------------------------------------

/// Build a portal URL prefixed with the region literal segment
/// (`TW/` or `HK/`) — every `UnconnectedGame_*` endpoint sits below
/// `https://{portal_host}/{region_segment}/...` rather than the
/// `beanfun_block/...` root used by [`auth_aspx`].
///
/// Private to `account.rs` because `WebForms` URL shape is
/// irrelevant outside this module.
fn mgmt_url(client: &BeanfunClient, suffix: &str) -> Result<Url, LoginError> {
    let region_segment = match client.config().region {
        LoginRegion::TW => "TW/",
        LoginRegion::HK => "HK/",
    };
    let path = format!("{region_segment}{suffix}");
    client.portal_url(&path)
}

/// Like [`mgmt_url`] but flips the scheme to `http://` for HK clients.
///
/// Used by the three `UnconnectedGame_ChangePassword` steps that WPF
/// reaches with `http://` literals in HK region (`Account.cs` L549-555
/// / L597-600). TW callers get back the unchanged HTTPS URL; HK
/// callers get an `http://` URL. See the "WPF deviation candidate"
/// section in the module docs for the rationale.
fn change_password_url(client: &BeanfunClient, suffix: &str) -> Result<Url, LoginError> {
    let mut url = mgmt_url(client, suffix)?;
    if client.config().region == LoginRegion::HK {
        url.set_scheme("http").map_err(|()| {
            LoginError::InvalidUrl(format!(
                "change_password_url: failed to switch scheme to http for `{suffix}`"
            ))
        })?;
    }
    Ok(url)
}

/// GET `auth.aspx?channel=accounts_management&page_and_query=01.aspx?…&web_token=…`
/// and parse the `__VIEWSTATE` + `__VIEWSTATEGENERATOR` pair from the
/// response. Used as the first step of both
/// [`unconnected_game_init_add_account_payload`] and
/// [`unconnected_game_change_password`].
///
/// Mirrors private `BeanfunClient.Account.cs::UnconnectedGame_InitAccountPayload`
/// (L174-209) — does **not** parse `__EVENTVALIDATION` (the GET
/// response does not carry one yet).
async fn init_account_payload(
    client: &BeanfunClient,
    session: &Session,
    service_code: &str,
    service_region: &str,
) -> Result<(String, String), LoginError> {
    let url = mgmt_url(client, "auth.aspx")?;
    // `page_and_query` is itself a relative URL — reqwest URL-encodes
    // it for us so `?` becomes `%3F` and `&` becomes `%26`, matching
    // WPF's hardcoded `01.aspx%3FServiceCode%3D…%26ServiceRegion%3D…`
    // byte sequence at L186.
    let inner = format!("01.aspx?ServiceCode={service_code}&ServiceRegion={service_region}");
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("channel", "accounts_management"),
            ("page_and_query", inner.as_str()),
            ("web_token", session.web_token.as_str()),
        ])
        .send()
        .await?;
    ensure_success(&resp, "accounts_management auth.aspx (GET)")?;
    let body = client.bounded_text(resp).await?;

    let form = extract_viewstate(&body).map_err(|_| LoginError::AccountMgmtMissingViewState)?;
    let viewstate_generator = form
        .viewstate_generator
        .ok_or(LoginError::AccountMgmtMissingViewStateGenerator)?;
    Ok((form.viewstate, viewstate_generator))
}

/// Strict variant of [`extract_viewstate`] that demands all three
/// hidden fields and stamps the client's region into the resulting
/// [`AddAccountSession`].
///
/// Used by every parse site that follows a WebForms POST: the server
/// always emits all three fields after a POST (unlike the initial GET
/// in [`init_account_payload`], which lacks `__EVENTVALIDATION`).
fn parse_viewstate_triplet(
    client: &BeanfunClient,
    html: &str,
) -> Result<AddAccountSession, LoginError> {
    let form = extract_viewstate(html).map_err(|_| LoginError::AccountMgmtMissingViewState)?;
    let viewstate_generator = form
        .viewstate_generator
        .ok_or(LoginError::AccountMgmtMissingViewStateGenerator)?;
    let event_validation = form
        .event_validation
        .ok_or(LoginError::AccountMgmtMissingEventValidation)?;
    Ok(AddAccountSession {
        viewstate: form.viewstate,
        viewstate_generator,
        event_validation,
        region: client.config().region,
    })
}

/// Append the four (TW: three) view-state hidden inputs to `form` in
/// the exact order WPF posts them at `Account.cs` L260-264 / L346-350
/// / L417-421 / L527-531 / L580-585:
///
/// 1. `__VIEWSTATE`
/// 2. `__VIEWSTATEGENERATOR`
/// 3. `__VIEWSTATEENCRYPTED` (HK only, value `""`)
/// 4. `__EVENTVALIDATION`
///
/// Centralised so every POST site automatically gets the HK-only
/// encrypted-marker right.
fn build_viewstate_payload_prefix(
    session: &AddAccountSession,
    form: &mut Vec<(&'static str, String)>,
) {
    form.push(("__VIEWSTATE", session.viewstate.clone()));
    form.push(("__VIEWSTATEGENERATOR", session.viewstate_generator.clone()));
    if session.region == LoginRegion::HK {
        form.push(("__VIEWSTATEENCRYPTED", String::new()));
    }
    form.push(("__EVENTVALIDATION", session.event_validation.clone()));
}

/// Append the optional display-name field. WPF L302-308 / L373-378 /
/// L454-460 all do the same `if (txtServiceAccountDN != null)` gate
/// with a region-keyed field name (`t1` for TW, `txtServiceAccountDN`
/// for HK).
fn push_account_dn(
    region: LoginRegion,
    form: &mut Vec<(&'static str, String)>,
    account_dn: Option<&str>,
) {
    if let Some(dn) = account_dn {
        let key = match region {
            LoginRegion::TW => "t1",
            LoginRegion::HK => "txtServiceAccountDN",
        };
        form.push((key, dn.to_owned()));
    }
}

/// Shared body of [`unconnected_game_add_account_check`] and
/// [`unconnected_game_add_account_check_nickname`] — the only
/// per-call differences are `event_target` (`lbtnCheckAccount` vs
/// `lbtnCheckNickName`) and `account_id` (the candidate id vs `""`).
///
/// Builds the 8 (TW) / 9 (HK) field POST body, fires it at `02.aspx`,
/// and parses the next view-state triplet + `lblErrorMessage`.
async fn add_account_check_inner(
    client: &BeanfunClient,
    mgmt_session: &AddAccountSession,
    event_target: &'static str,
    account_id: &str,
    account_dn: Option<&str>,
) -> Result<CheckOutcome, LoginError> {
    let url = mgmt_url(client, "accounts_management/02.aspx")?;

    let mut form: Vec<(&'static str, String)> = Vec::new();
    build_viewstate_payload_prefix(mgmt_session, &mut form);
    form.push(("__EVENTTARGET", event_target.to_owned()));
    form.push(("__EVENTARGUMENT", String::new()));
    form.push(("txtServiceAccountID", account_id.to_owned()));
    push_account_dn(mgmt_session.region, &mut form, account_dn);
    form.push(("txtNewPwd", String::new()));
    form.push(("txtNewPwd2", String::new()));

    let resp = client.http().post(url).form(&form).send().await?;
    ensure_success(&resp, "accounts_management/02.aspx (check POST)")?;
    let body = client.bounded_text(resp).await?;

    let session = parse_viewstate_triplet(client, &body)?;
    let error_message = extract_lbl_error_message(&body);
    Ok(CheckOutcome {
        session,
        error_message,
    })
}

/// Build the full add-account POST body used by
/// [`unconnected_game_add_account`]. Field order matches
/// `Account.cs` L451-465 verbatim.
fn build_add_account_form(
    mgmt_session: &AddAccountSession,
    name: &str,
    new_password: &str,
    new_password_confirm: &str,
    account_dn: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut form: Vec<(&'static str, String)> = Vec::new();
    build_viewstate_payload_prefix(mgmt_session, &mut form);
    form.push(("__EVENTTARGET", String::new()));
    form.push(("__EVENTARGUMENT", String::new()));
    form.push(("txtServiceAccountID", name.to_owned()));
    push_account_dn(mgmt_session.region, &mut form, account_dn);
    form.push(("txtNewPwd", new_password.to_owned()));
    form.push(("txtNewPwd2", new_password_confirm.to_owned()));
    form.push(("chkBox1", "on".to_owned()));
    form.push(("imgbtn_Submit.x", "0".to_owned()));
    form.push(("imgbtn_Submit.y", "0".to_owned()));
    form
}

/// Memoised regex for `<span id="lblGameName">…</span>`. Mirrors WPF
/// `Account.cs` L266 verbatim.
fn lbl_game_name_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"<span id="lblGameName">(.*)</span>"#).expect("lblGameName regex")
    })
}

/// Memoised regex for `<span id="lblAccountLen">…</span>`. Mirrors WPF
/// `Account.cs` L274 verbatim.
fn lbl_account_len_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"<span id="lblAccountLen">(.*)</span>"#).expect("lblAccountLen regex")
    })
}

/// Memoised regex for `<span id="lblErrorMessage" style="color:Red;">…</span>`.
/// Mirrors WPF `Account.cs` L352 / L478 / L590 verbatim.
fn lbl_error_message_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"<span id="lblErrorMessage" style="color:Red;">(.*)</span>"#)
            .expect("lblErrorMessage regex")
    })
}

/// Extract the `lblErrorMessage` body, returning `""` on absence
/// (matching WPF's `regex.IsMatch ? Groups[1].Value : ""` ternary).
fn extract_lbl_error_message(html: &str) -> String {
    capture_first(lbl_error_message_regex(), html).unwrap_or_default()
}

/// Memoised regex for the `BeanFunBlock.LoggedInUserData.Email` JavaScript
/// assignment inside the TW `loader.ashx` response. Mirrors WPF
/// `BeanfunClient.cs` L252-253 verbatim.
///
/// The trailing `;BeanFunBlock.LoggedInUserData.MessageCount` anchor is
/// inherited from WPF — it bounds the `(.*)` greedy capture so the
/// match stops before the next JS assignment on the same line.
fn email_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"BeanFunBlock\.LoggedInUserData\.Email = "(.*)";BeanFunBlock\.LoggedInUserData\.MessageCount"#,
        )
        .expect("email regex")
    })
}

/// Memoised regex for the `"RemainPoint" : "…"` JSON field emitted by
/// `get_remain_point.ashx`. Mirrors WPF `BeanfunClient.cs` L231 verbatim,
/// **including** the literal spaces on either side of the colon — the
/// server-shaped formatting is effectively part of the contract.
fn remain_point_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""RemainPoint" : "(.*)" \}"#).expect("remain_point regex"))
}

/// Memoised regex for the `verify_code=<token>` query parameter on the
/// final `03.aspx` POST redirect URL. Mirrors WPF `Account.cs` L608.
fn verify_code_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"verify_code=([^&]*)").expect("verify_code regex"))
}

/// Extract the verification token from the post-step-5 redirect URL,
/// stripping the `verify_code=` prefix and terminating at the next
/// `&` query-parameter boundary.
///
/// WPF's three-step round-trip
/// (`regex.Match(ResponseUri).Groups[1].Value` →
/// `"verify_code" + groups[1].Value` → caller's
/// `result.Replace("verify_code", "")`,
/// `UnconnectedGame_ChangePassword.xaml.cs` L30-35) is collapsed into
/// a single helper that returns the bare token directly. The sentinel
/// prefix exists in WPF only because the `string` return type can't
/// disambiguate success from `lblErrorMessage`; our typed
/// [`ChangePasswordOutcome`] enum carries that semantic on the type
/// itself.
///
/// We diverge from WPF's greedy `verify_code=(.*)` capture by using
/// `verify_code=([^&]*)` so trailing `&other=…` query parameters or
/// `#fragment` suffixes don't get spliced into the token. See the
/// "WPF deviation: `verify_code` extraction shape" section of the
/// module docs for why this is functionally aligned with the WPF UX
/// (and strictly safer).
///
/// Returns `None` when the URL has no `verify_code=` parameter.
fn extract_verify_code_from_url(url: &Url) -> Option<String> {
    let url_str = url.as_str();
    verify_code_regex()
        .captures(url_str)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
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

    // =========================================================================
    // P4.4 — WebForms account-management helper tests
    // =========================================================================

    use super::super::client::ClientConfig;

    fn tw_client_for_unit_tests() -> BeanfunClient {
        BeanfunClient::new(ClientConfig::for_region(LoginRegion::TW)).expect("client builds in TW")
    }

    fn hk_client_for_unit_tests() -> BeanfunClient {
        BeanfunClient::new(ClientConfig::for_region(LoginRegion::HK)).expect("client builds in HK")
    }

    fn fake_session(region: LoginRegion) -> AddAccountSession {
        AddAccountSession {
            viewstate: "VS_TOKEN".to_owned(),
            viewstate_generator: "GEN_TOKEN".to_owned(),
            event_validation: "EV_TOKEN".to_owned(),
            region,
        }
    }

    // -------------------------------------------------------------------------
    // mgmt_url — region literal segment under portal_base
    // -------------------------------------------------------------------------

    #[test]
    fn mgmt_url_tw_uses_uppercase_tw_segment() {
        let client = tw_client_for_unit_tests();
        let url = mgmt_url(&client, "accounts_management/02.aspx").unwrap();
        assert_eq!(
            url.as_str(),
            "https://tw.beanfun.com/TW/accounts_management/02.aspx"
        );
    }

    #[test]
    fn mgmt_url_hk_uses_uppercase_hk_segment() {
        let client = hk_client_for_unit_tests();
        let url = mgmt_url(&client, "accounts_management/02.aspx").unwrap();
        assert_eq!(
            url.as_str(),
            "https://bfweb.hk.beanfun.com/HK/accounts_management/02.aspx"
        );
    }

    #[test]
    fn mgmt_url_supports_top_level_auth_aspx_under_region_segment() {
        let client = tw_client_for_unit_tests();
        let url = mgmt_url(&client, "auth.aspx").unwrap();
        assert_eq!(url.as_str(), "https://tw.beanfun.com/TW/auth.aspx");
    }

    // -------------------------------------------------------------------------
    // change_password_url — HK switches to http://, TW stays https://
    // -------------------------------------------------------------------------

    #[test]
    fn change_password_url_tw_keeps_https() {
        let client = tw_client_for_unit_tests();
        let url = change_password_url(&client, "accounts_management/03.aspx").unwrap();
        assert_eq!(
            url.as_str(),
            "https://tw.beanfun.com/TW/accounts_management/03.aspx",
            "TW must stay on HTTPS"
        );
    }

    #[test]
    fn change_password_url_hk_switches_to_http() {
        let client = hk_client_for_unit_tests();
        let url = change_password_url(&client, "accounts_management/03.aspx").unwrap();
        assert_eq!(
            url.as_str(),
            "http://bfweb.hk.beanfun.com/HK/accounts_management/03.aspx",
            "HK must switch to http:// to mirror WPF L549-555 / L597-600"
        );
    }

    // -------------------------------------------------------------------------
    // parse_viewstate_triplet — typed errors for each missing field
    // -------------------------------------------------------------------------

    #[test]
    fn parse_viewstate_triplet_happy_path_carries_region() {
        let client = hk_client_for_unit_tests();
        let html = r#"
            <input id="__VIEWSTATE" value="VS1" />
            <input id="__VIEWSTATEGENERATOR" value="GEN1" />
            <input id="__EVENTVALIDATION" value="EV1" />
        "#;
        let session = parse_viewstate_triplet(&client, html).unwrap();
        assert_eq!(session.viewstate, "VS1");
        assert_eq!(session.viewstate_generator, "GEN1");
        assert_eq!(session.event_validation, "EV1");
        assert_eq!(
            session.region,
            LoginRegion::HK,
            "session must remember the client's region for later __VIEWSTATEENCRYPTED routing"
        );
    }

    #[test]
    fn parse_viewstate_triplet_missing_viewstate_typed_error() {
        let client = tw_client_for_unit_tests();
        let html = r#"<input id="__VIEWSTATEGENERATOR" value="GEN1" />
                      <input id="__EVENTVALIDATION" value="EV1" />"#;
        assert!(matches!(
            parse_viewstate_triplet(&client, html).unwrap_err(),
            LoginError::AccountMgmtMissingViewState
        ));
    }

    #[test]
    fn parse_viewstate_triplet_missing_generator_typed_error() {
        let client = tw_client_for_unit_tests();
        let html = r#"<input id="__VIEWSTATE" value="VS1" />
                      <input id="__EVENTVALIDATION" value="EV1" />"#;
        assert!(matches!(
            parse_viewstate_triplet(&client, html).unwrap_err(),
            LoginError::AccountMgmtMissingViewStateGenerator
        ));
    }

    #[test]
    fn parse_viewstate_triplet_missing_event_validation_typed_error() {
        let client = tw_client_for_unit_tests();
        let html = r#"<input id="__VIEWSTATE" value="VS1" />
                      <input id="__VIEWSTATEGENERATOR" value="GEN1" />"#;
        assert!(matches!(
            parse_viewstate_triplet(&client, html).unwrap_err(),
            LoginError::AccountMgmtMissingEventValidation
        ));
    }

    // -------------------------------------------------------------------------
    // build_viewstate_payload_prefix — HK splices in __VIEWSTATEENCRYPTED
    // -------------------------------------------------------------------------

    #[test]
    fn build_viewstate_payload_prefix_tw_emits_3_fields_in_order() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        build_viewstate_payload_prefix(&fake_session(LoginRegion::TW), &mut form);
        let keys: Vec<&str> = form.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec!["__VIEWSTATE", "__VIEWSTATEGENERATOR", "__EVENTVALIDATION"],
            "TW must NOT emit __VIEWSTATEENCRYPTED"
        );
    }

    #[test]
    fn build_viewstate_payload_prefix_hk_emits_4_fields_with_encrypted_marker() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        build_viewstate_payload_prefix(&fake_session(LoginRegion::HK), &mut form);
        let kvs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(
            kvs,
            vec![
                ("__VIEWSTATE", "VS_TOKEN"),
                ("__VIEWSTATEGENERATOR", "GEN_TOKEN"),
                ("__VIEWSTATEENCRYPTED", ""),
                ("__EVENTVALIDATION", "EV_TOKEN"),
            ],
            "HK must splice __VIEWSTATEENCRYPTED='' between generator and event_validation"
        );
    }

    // -------------------------------------------------------------------------
    // push_account_dn — region-keyed display-name field
    // -------------------------------------------------------------------------

    #[test]
    fn push_account_dn_some_tw_uses_t1_field_name() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        push_account_dn(LoginRegion::TW, &mut form, Some("AcME"));
        assert_eq!(form, vec![("t1", "AcME".to_owned())]);
    }

    #[test]
    fn push_account_dn_some_hk_uses_long_field_name() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        push_account_dn(LoginRegion::HK, &mut form, Some("AcME"));
        assert_eq!(form, vec![("txtServiceAccountDN", "AcME".to_owned())]);
    }

    #[test]
    fn push_account_dn_none_skips_field_entirely() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        push_account_dn(LoginRegion::TW, &mut form, None);
        assert!(form.is_empty(), "None must add no fields, not even empty");
    }

    /// Empty-string DN still opts into the field (matches WPF L302
    /// `txtServiceAccountDN != null` — the C# null-check, not an
    /// emptiness check).
    #[test]
    fn push_account_dn_some_empty_still_adds_empty_field() {
        let mut form: Vec<(&'static str, String)> = Vec::new();
        push_account_dn(LoginRegion::TW, &mut form, Some(""));
        assert_eq!(form, vec![("t1", String::new())]);
    }

    // -------------------------------------------------------------------------
    // build_add_account_form — full POST body shape & order
    // -------------------------------------------------------------------------

    #[test]
    fn build_add_account_form_tw_field_count_and_order() {
        let form = build_add_account_form(
            &fake_session(LoginRegion::TW),
            "myAccount",
            "P@ssw0rd!",
            "P@ssw0rd!",
            Some("MyDN"),
        );
        let keys: Vec<&str> = form.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                "__VIEWSTATE",
                "__VIEWSTATEGENERATOR",
                "__EVENTVALIDATION",
                "__EVENTTARGET",
                "__EVENTARGUMENT",
                "txtServiceAccountID",
                "t1",
                "txtNewPwd",
                "txtNewPwd2",
                "chkBox1",
                "imgbtn_Submit.x",
                "imgbtn_Submit.y",
            ],
            "TW with DN must produce exactly 12 fields in this WPF-aligned order"
        );
    }

    #[test]
    fn build_add_account_form_hk_with_dn_inserts_encrypted_and_long_dn_field() {
        let form = build_add_account_form(
            &fake_session(LoginRegion::HK),
            "myAccount",
            "pwd",
            "pwd",
            Some("MyDN"),
        );
        let keys: Vec<&str> = form.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                "__VIEWSTATE",
                "__VIEWSTATEGENERATOR",
                "__VIEWSTATEENCRYPTED", // HK-only marker
                "__EVENTVALIDATION",
                "__EVENTTARGET",
                "__EVENTARGUMENT",
                "txtServiceAccountID",
                "txtServiceAccountDN", // HK-only DN field name
                "txtNewPwd",
                "txtNewPwd2",
                "chkBox1",
                "imgbtn_Submit.x",
                "imgbtn_Submit.y",
            ],
            "HK with DN must produce 13 fields including __VIEWSTATEENCRYPTED + txtServiceAccountDN"
        );
    }

    #[test]
    fn build_add_account_form_no_dn_skips_dn_field() {
        let form = build_add_account_form(
            &fake_session(LoginRegion::TW),
            "myAccount",
            "pwd",
            "pwd",
            None,
        );
        assert!(
            form.iter()
                .all(|(k, _)| *k != "t1" && *k != "txtServiceAccountDN"),
            "No DN passed ⇒ neither t1 nor txtServiceAccountDN must appear"
        );
    }

    // -------------------------------------------------------------------------
    // extract_lbl_error_message — present / absent / empty span
    // -------------------------------------------------------------------------

    #[test]
    fn extract_lbl_error_message_present_returns_text() {
        let html = r#"<span id="lblErrorMessage" style="color:Red;">該帳號已存在</span>"#;
        assert_eq!(extract_lbl_error_message(html), "該帳號已存在");
    }

    #[test]
    fn extract_lbl_error_message_absent_returns_empty() {
        assert_eq!(extract_lbl_error_message("<html>nothing</html>"), "");
    }

    /// A present-but-empty span behaves like "no error" (WPF L605
    /// `if (lblErrorMessage != "") return lblErrorMessage;` returns
    /// the empty string back to caller, which is then treated as
    /// `"verify_code…"` lookup).
    #[test]
    fn extract_lbl_error_message_empty_span_returns_empty_string() {
        let html = r#"<span id="lblErrorMessage" style="color:Red;"></span>"#;
        assert_eq!(extract_lbl_error_message(html), "");
    }

    // -------------------------------------------------------------------------
    // extract_verify_code_from_url — strip prefix, terminate at &
    // -------------------------------------------------------------------------

    #[test]
    fn extract_verify_code_from_url_present_strips_prefix() {
        let url = Url::parse(
            "https://tw.beanfun.com/TW/accounts_management/03.aspx?verify_code=ABC123XYZ",
        )
        .unwrap();
        assert_eq!(
            extract_verify_code_from_url(&url).as_deref(),
            Some("ABC123XYZ")
        );
    }

    #[test]
    fn extract_verify_code_from_url_absent_returns_none() {
        let url = Url::parse("https://tw.beanfun.com/TW/accounts_management/03.aspx").unwrap();
        assert_eq!(extract_verify_code_from_url(&url), None);
    }

    /// Server may append further query params after the verify_code
    /// token (`?verify_code=ABC&other=1`). Our regex must terminate at
    /// the next `&` so we don't capture the trailing junk.
    #[test]
    fn extract_verify_code_from_url_with_extra_query_terminates_at_ampersand() {
        let url = Url::parse("https://tw.beanfun.com/?verify_code=ABC123&trailing=junk").unwrap();
        assert_eq!(
            extract_verify_code_from_url(&url).as_deref(),
            Some("ABC123")
        );
    }
}
