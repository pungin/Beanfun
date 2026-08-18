//! OTP retrieval flow — port of `BeanfunClient.OTP.cs::GetOTP`.
//!
//! Issues a 5-step HTTP sequence against the Beanfun portal that
//! produces an opaque DES-ECB ciphertext, then decrypts it locally
//! with [`crate::core::wcdes::decrypt_hex`] into the 8-character OTP
//! string the launcher hands to the game client.
//!
//! ```text
//!   1. GET  game_zone/game_start_step2.aspx       -> longPollingKey [+ TW unkData] [+ screatetime fallback]
//!   2. GET  generic_handlers/get_cookies.ashx     -> m_strSecretCode
//!   3. POST generic_handlers/record_service_start.ashx  (response discarded; primes server-side state)
//!   4. GET  get_result.ashx?meth=GetResultByLongPolling (response discarded; long-poll trigger)
//!   5. GET  generic_handlers/get_webstart_otp.ashx -> "1;{key8}{ciphertext_hex}"
//!   6. WCDES decrypt -> trim trailing NULs -> OTP
//! ```
//!
//! # State model
//!
//! Same shape as the rest of P3/P4: every call takes
//! `(client: &BeanfunClient, session: &Session, account: &ServiceAccount, …)`.
//! Notably `account` is borrowed **immutably** — WPF mutates
//! `acc.screatetime` when the input was `null` (L64), but we keep the
//! input pure and use a local `String` for the fallback path instead.
//!
//! # Region asymmetry: step 2 host
//!
//! Step 2 (`get_cookies.ashx`) is the **only** OTP step that uses a
//! region-asymmetric host:
//!
//! | Region | Host (`loginHost` in WPF L26-31) |
//! |--------|----------------------------------|
//! | TW     | `tw.newlogin.beanfun.com`        |
//! | HK     | `login.hk.beanfun.com`           |
//!
//! Our existing [`super::Endpoints`] schema has `newlogin_base` (which
//! always points at TW, by design — see the [`super::Endpoints`] doc for
//! why) and `login_base` (TW = `login.beanfun.com`, HK =
//! `login.hk.beanfun.com`). The OTP step 2 host happens to align with
//! `newlogin_url` for TW and `login_url` for HK, so we branch on
//! [`super::LoginRegion`] inside `step_2_get_secret_code` rather than
//! adding a fourth base URL to `Endpoints` for this single call.
//!
//! For wiremock-based integration tests this is transparent — the test
//! harness routes both `login_base` and `newlogin_base` at the same
//! mock server, so the region branch picks the right helper but the
//! request still lands on the mock.
//!
//! # WPF dev artifacts (NOT ported)
//!
//! - `ServicePointManager.Expect100Continue = false` (L90): WPF's
//!   final wire behaviour after this assignment is "no `Expect:
//!   100-continue` header". reqwest's default behaviour is also "no
//!   `Expect: 100-continue` header" (and reqwest exposes no toggle to
//!   enable it). The end state is byte-equivalent, so the global
//!   mutation is dropped.
//! - `// Thread.Sleep(5000);` (L98): commented in WPF source.
//! - `// Console.WriteLine(Environment.TickCount);` (L99): commented in
//!   WPF source.
//!
//! # `ppppp=` literal (1:1 verbatim)
//!
//! Step 5's URL contains a hardcoded 64-character uppercase hex
//! literal as the `ppppp` query parameter (`1F552AEAFF976018F942B...`).
//! WPF concatenates it inline; we lift it to a `const` for visibility
//! but the bytes are byte-for-byte identical to WPF L101. The
//! provenance of this literal is **unknown**: it appears to be a
//! protocol-level constant the server validates against. Do not
//! change it without empirical verification.
//!
//! # Empty captures (deliberate divergence from WPF)
//!
//! Every value that step 5 splices into its query string is scraped
//! with a `(.*)` capture group copied verbatim from WPF. `(.*)`
//! matches the empty string, so a page shaped like
//! `var m_strSecretCode = '';` yields `Ok("")` rather than a "not
//! found" miss — and WPF forwards that straight into the URL as
//! `SecretCode=`.
//!
//! Steps 3 and 4 discard their response bodies (`drop(resp)`, mirroring
//! WPF's ignored `UploadString` return value) and `ensure_success`
//! only inspects the HTTP status, which these `.ashx` handlers keep at
//! 200 even when they reject the request. Step 5 is therefore the first
//! step that reads a body — so *every* upstream breakage collapses into
//! one opaque `OtpServerRejected("Query String Error")` there, with no
//! signal as to which parameter was actually bad.
//!
//! To keep failures attributable, `parse_long_polling_key` and
//! `parse_secret_code` — the two scrapes whose patterns really do use
//! `(.*)` — reject an empty capture and return their existing typed
//! `OtpMissing*` errors instead. This is the one place we knowingly
//! break the 1:1 WPF alignment: WPF's behaviour here is a diagnostic
//! dead end, not a protocol requirement.
//!
//! `CreateTime` needs no such guard — its regex is already
//! `([^"]+)`. It reaches step 5 empty by a different route:
//! [`ServiceAccount`] is `Deserialize` and arrives over IPC from the
//! frontend, so `screatetime: Some("")` bypasses the fallback scrape
//! that a `None` would have triggered. `step_1_init` treats that
//! empty string as absent.
//!
//! Note this does **not** cover greedy over-capture (a second `"` later
//! on the same line makes `key=(.*)"` swallow the rest of it). Tightening
//! those patterns to `([^"]*)` would change what gets matched on real
//! pages, so it is left alone pending captures from a live server.
//!
//! # Two step 5s
//!
//! Beanfun added a step 5; it did not retire the other one.
//! `POST get_webstart_otp_v2.ashx` takes a JSON body and answers with a
//! JSON `data` member. `GET get_webstart_otp.ashx` takes nine query
//! parameters and answers with a `1;…` envelope. Both are live, and
//! which one answers is per game rather than per region: MapleStory
//! moved to v2, while CSO, Elsword and Mabinogi did not.
//!
//! [`get_otp`] therefore picks in two stages, both on evidence rather
//! than on configuration. A page carrying the `m_objData`
//! launcher-handoff literal hands its OTP over through the launcher,
//! and what that handoff's obfuscated blob decodes to names the
//! endpoint: a `LaunchTicket` means v2, a `ppppp` payload means the
//! older handler, answered with the blob's own parameters rather than
//! with our session's. A page without the literal keeps the original
//! flow, which is HK today. Neither stage needs changing when a game or
//! a region moves.
//!
//! Either way the payload is *not* a new server round-trip: it travels
//! obfuscated inside `m_objData.data`, decoded by
//! [`crate::core::launch_data`]. Both requests also carry `CV`, `Hash`
//! and `arch`, which identify the official launcher build and come from
//! [`super::client_integrity`]. `docs/OTP-PROTOCOL-CHANGE.md` has the
//! full derivation.
//!
//! Reading the old handler as dead is what broke every game but
//! MapleStory (upstream #376). `0;        Query String Error` is what
//! it answers a request built from the wrong values — which a request
//! built from our session state is, on a page that handed over its
//! own.
//!
//! # The same three values on the legacy path
//!
//! The pre-v2 `GET` grew the identical `&CV=…&Hash=…&arch=…` suffix in
//! Game Manager 1.5.x — `GGM.Shared.Beanfun.BeanfunUrlBuilder.BuildOtpUrl`
//! appends it — and rejects requests that omit it. Sending it keeps the
//! legacy fallback above genuinely usable rather than doomed the moment
//! it is reached. HK, whose page carries no `m_objData` and whose portal
//! has not been observed to want the suffix, is left byte-identical.

use std::sync::OnceLock;

use chrono::Local;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;

use crate::core::launch_data::{decode_launch_data, LaunchPayload, LegacyOtpParams};
use crate::core::parser::{capture_first, extract_service_account_create_time};
use crate::core::redact::scrub;
use crate::core::time::{dt_compact_now, dt_iso_now};
use crate::core::wcdes::decrypt_hex;
use crate::services::beanfun::account::ServiceAccount;
use crate::services::beanfun::client::{BeanfunClient, LoginRegion};
use crate::services::beanfun::client_integrity::ClientIntegrity;
use crate::services::beanfun::error::LoginError;
use crate::services::beanfun::login::ensure_success;
use crate::services::beanfun::session::Session;

// -----------------------------------------------------------------------------
// Public API
// -----------------------------------------------------------------------------

/// Fetch a one-time password (OTP) for a given service account.
///
/// Mirrors `BeanfunClient.OTP.cs::GetOTP` (L12-151). Drives the full
/// 5-HTTP + 1-decrypt sequence and returns the decoded 8-character OTP
/// the launcher will splice into the game's IPC handshake.
///
/// # Defaults
///
/// `service_code` and `service_region` default to `"610074"` /
/// `"T9"` in WPF (L14-15) — i.e. the MapleStory production codes.
/// We require explicit values to keep the function surface honest;
/// callers can grab the defaults via
/// [`LoginRegion::default_service_code`] /
/// [`LoginRegion::default_service_region`].
///
/// # Errors
///
/// All seven WPF `errmsg` strings have a 1:1 typed counterpart:
///
/// | WPF errmsg                          | Rust variant                                   |
/// |-------------------------------------|------------------------------------------------|
/// | `OTPNoLongPollingKey:{response}`    | [`LoginError::OtpMissingLongPollingKey`]       |
/// | `OTPNoUnkData`                      | [`LoginError::OtpMissingUnkData`] (TW only)    |
/// | `OTPNoCreateTime`                   | [`LoginError::OtpMissingCreateTime`]           |
/// | `OTPNoSecretCode`                   | [`LoginError::OtpMissingSecretCode`]           |
/// | `OTPNoResponse`                     | [`LoginError::OtpEmptyResponse`]               |
/// | `GetOtpError\r\n{server msg}`       | [`LoginError::OtpServerRejected`]              |
/// | `DecryptOTPError`                   | [`LoginError::OtpDecryptionFailed`]            |
///
/// Transport-level failures (non-2xx, network, body too large) bubble
/// up as [`LoginError::Http`] / [`LoginError::Unknown`] /
/// [`LoginError::BodyTooLarge`], matching the catch-all WPF behaviour
/// of the surrounding `try { } catch { return null; }` (L141-150).
pub async fn get_otp(
    client: &BeanfunClient,
    session: &Session,
    account: &ServiceAccount,
    service_code: &str,
    service_region: &str,
) -> Result<String, LoginError> {
    let step1 = step_1_init(client, account, service_code, service_region).await?;

    // Branch on the page's own shape rather than on the region: a page
    // that carries `m_objData` hands its OTP over through the launcher,
    // and the blob decides which endpoint answers — not this branch,
    // and not the game. Observed on TW; HK's page has no such literal
    // and keeps the original flow. If HK changes later this needs no
    // change.
    if let Some(handoff) = &step1.launch {
        // Recording the start is what the page does alongside opening
        // the launcher, and nothing downstream depends on it — so a
        // migrated page missing a field it no longer has to carry must
        // not cost the user their password.
        if let Err(error) =
            step_3_record_start(client, account, &step1, service_code, service_region).await
        {
            tracing::warn!(%error, "could not record the service start; continuing");
        }

        // Deliberately no secret code and no long poll on this path.
        // The v2 request does not carry a secret code, and the page's
        // `GetResultByLongPolling` call is the launcher's installation
        // check — unrelated to the password, and it holds the
        // connection open.
        let integrity = resolve_client_integrity().await;

        // Which endpoint answers is decided by what the blob turned out
        // to carry, not by the game or the region. Both payloads are
        // live: MapleStory hands over a `LaunchTicket`, while Mabinogi,
        // Elsword and others hand over the pre-v2 parameters.
        let payload =
            decode_launch_data(&handoff.data).map_err(|e| LoginError::OtpDecryptionFailed {
                cause: format!("launch data: {e}"),
            })?;
        return match payload {
            LaunchPayload::Ticket(ticket) => {
                step_5_post_otp_v2(client, handoff, &ticket, &integrity, &step1.page_url).await
            }
            LaunchPayload::Legacy(params) => {
                // The page usually declares these two alongside the
                // blob, but nothing observed guarantees it — and the
                // pre-v2 flow already had its own sources for both, so
                // fall back to those rather than refuse to try.
                let web_token = match &handoff.web_token {
                    Some(token) => token.clone(),
                    None => session.web_token.clone(),
                };
                let secret_code = match &handoff.secret_code {
                    Some(code) => code.clone(),
                    None => step_2_get_secret_code(client).await?,
                };
                step_5_get_otp_from_handoff(
                    client,
                    handoff,
                    &params,
                    &web_token,
                    &secret_code,
                    &integrity,
                    &step1.page_url,
                )
                .await
            }
        };
    }

    let secret_code = step_2_get_secret_code(client).await?;
    step_3_record_start(client, account, &step1, service_code, service_region).await?;
    step_4_long_poll(client, &step1.long_polling_key).await?;
    let integrity = resolve_client_integrity().await;

    let url = build_get_webstart_otp_url(
        client,
        session,
        account,
        &step1,
        &secret_code,
        service_code,
        service_region,
        tick_count_ms(),
        // The suffix is a Game Manager convention, and the Game Manager
        // is a TW/OATW product; HK's legacy endpoint has not been
        // observed to want it, so its request stays as it was.
        match client.config().region {
            LoginRegion::TW => Some(&integrity),
            LoginRegion::HK => None,
        },
    )?;
    let envelope = step_5_get_otp(client, &url).await?;

    let result = step_6_decrypt(&envelope);
    if let Err(err) = &result {
        log_step_5_failure(err, "legacy", &url, &envelope);
    }
    result
}

/// Report a step 5 failure without putting the user's password in the
/// log.
///
/// [`LoginError::OtpDecryptionFailed`] is the one failure that arrives
/// holding a *successful* `1;…` envelope — its payload carries the live
/// OTP key and ciphertext, which must never be written down. Every
/// other variant carries only a rejection message.
///
/// That message is server-controlled and could echo back a parameter we
/// sent, so it goes through [`scrub`] even though its field name is
/// outside the leak guard's list. `scrub` only rewrites
/// credential-shaped `k=v` pairs, leaving a plain rejection like
/// `0;  Query String Error` intact.
///
/// Both `GET` routes share this rather than each keeping a copy: the
/// rule about the success envelope is exactly the kind that gets fixed
/// in one place and left wrong in the other. `route` says which one is
/// speaking, which the shared message otherwise loses.
fn log_step_5_failure(err: &LoginError, route: &str, url: &str, envelope: &str) {
    let raw_response = match err {
        LoginError::OtpDecryptionFailed { .. } => "<redacted: success envelope>".to_owned(),
        _ => scrub(envelope),
    };
    tracing::warn!(
        error = %err,
        route,
        request_url = %redact_otp_url(url),
        raw_response,
        raw_response_len = envelope.len(),
        "OTP step 5 failed — server response logged for diagnosis"
    );
}

// -----------------------------------------------------------------------------
// Step orchestration (private)
// -----------------------------------------------------------------------------

/// Outputs of step 1 that downstream steps need.
struct Step1Data {
    /// Server-issued long-polling key from the inline JS literal
    /// `GetResultByLongPolling&key=...`. Used by steps 4 and 5.
    long_polling_key: String,
    /// TW-only `(key, value)` extracted from the
    /// `MyAccountData.ServiceAccountCreateTime + "key=value";` literal.
    /// Both halves are URL-decoded already (matching WPF
    /// `Uri.UnescapeDataString`). Step 3 forwards them as an extra
    /// form field. `None` for HK.
    unk_data: Option<(String, String)>,
    /// Service-account creation timestamp. Either the input
    /// `account.screatetime` if it was `Some`, or the value
    /// fallback-parsed from this step's response body.
    screatetime: String,
    /// The page's own URL, query and all.
    ///
    /// Handlers under `generic_handlers` began checking `Referer`, and
    /// answer `The URL referrer is null or from a different domain!`
    /// without one. Being same-origin, and with beanfun's
    /// `strict-origin-when-cross-origin` policy, a browser sends this
    /// whole URL — so we send exactly it.
    page_url: String,
    /// The launcher-handoff literal, when the page carries one. Only
    /// the v2 OTP path reads it; `None` on pages without it.
    launch: Option<LaunchHandoff>,
}

/// The `m_objData` literal `game_start_step2.aspx` builds for the
/// native launcher.
///
/// Its `region` member is ignored: it is the constant
/// `"TW;Production"` and no request we make carries it.
struct LaunchHandoff {
    /// 36-character GUID, sent as `SN` on either OTP request.
    sn: String,
    /// Obfuscated blob carrying the OTP payload, decoded by
    /// [`decode_launch_data`].
    data: String,
    /// Present only on pages whose blob carries the pre-v2 payload —
    /// `ggm.js` forwards these to the launcher exactly when they exist,
    /// and the pre-v2 OTP URL is the one that needs them.
    web_token: Option<String>,
    secret_code: Option<String>,
}

/// Step 1 — fetch `game_start_step2.aspx`, extract the long-polling
/// key, and (TW only) the `unk_data` per-account form fragment.
/// Optionally falls back to scraping `screatetime` from the same
/// response if the caller didn't supply one.
async fn step_1_init(
    client: &BeanfunClient,
    account: &ServiceAccount,
    service_code: &str,
    service_region: &str,
) -> Result<Step1Data, LoginError> {
    let mut url = client.portal_url("beanfun_block/game_zone/game_start_step2.aspx")?;
    url.query_pairs_mut()
        .append_pair("service_code", service_code)
        .append_pair("service_region", service_region)
        .append_pair("sotp", account.ssn.as_str())
        .append_pair("dt", dt_compact_now().as_str());
    // Kept because the handlers below are now asked to name the page
    // that sent them; see `Step1Data::page_url`.
    let page_url = url.to_string();
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "game_start_step2.aspx")?;
    let body = client.bounded_text(resp).await?;

    // Read the handoff *first*: it decides which route this page is on,
    // and therefore which of the literals below are required at all.
    //
    // A migrated page has dropped the ones the old flow used — there is
    // no `GetResultByLongPolling&key=` on it any more, because the page
    // no longer polls; it opens the launcher. Parsing them before
    // looking would fail the whole retrieval before the handoff is even
    // read, and the reported error would name a literal that is *meant*
    // to be gone.
    let launch = parse_launch_handoff(&body);
    let migrated = launch.is_some();

    let long_polling_key = match parse_long_polling_key(&body) {
        Ok(key) => key,
        Err(_) if migrated => String::new(),
        Err(e) => return Err(e),
    };
    let unk_data = match client.config().region {
        // Still wanted when present — it is the service-start form's
        // anti-forgery field — but not worth failing a retrieval over
        // on a page that no longer has to carry it.
        LoginRegion::TW if migrated => parse_unk_data(&body).ok(),
        LoginRegion::TW => Some(parse_unk_data(&body)?),
        LoginRegion::HK => None,
    };
    // A stored `Some("")` is as unusable as `None` — both produce an
    // empty `CreateTime=` on step 5's URL, which the portal rejects
    // with a generic "Query String Error". Treat it as absent so the
    // fallback scrape gets its chance.
    let screatetime = match account.screatetime.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        // The v2 request does not carry a create time; only the
        // best-effort service-start form does.
        _ => match parse_screatetime_fallback(&body) {
            Ok(t) => t,
            Err(_) if migrated => String::new(),
            Err(e) => return Err(e),
        },
    };

    Ok(Step1Data {
        page_url,
        long_polling_key,
        unk_data,
        screatetime,
        launch,
    })
}

/// Step 2 — fetch the login host's `get_cookies.ashx` and scrape the
/// `m_strSecretCode` JS literal.
///
/// **Region branch**: TW uses the newlogin host, HK uses the login
/// host. See the module-level "Region asymmetry" doc for why we don't
/// add a fourth base URL to `Endpoints` for this one call.
async fn step_2_get_secret_code(client: &BeanfunClient) -> Result<String, LoginError> {
    let url = match client.config().region {
        LoginRegion::TW => client.newlogin_url("generic_handlers/get_cookies.ashx")?,
        LoginRegion::HK => client.login_url("generic_handlers/get_cookies.ashx")?,
    };
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "get_cookies.ashx")?;
    let body = client.bounded_text(resp).await?;
    parse_secret_code(&body)
}

/// Step 3 — POST to `record_service_start.ashx` with the per-account
/// form payload. Response is intentionally discarded; the call exists
/// only to prime server-side state for step 5.
async fn step_3_record_start(
    client: &BeanfunClient,
    account: &ServiceAccount,
    step1: &Step1Data,
    service_code: &str,
    service_region: &str,
) -> Result<(), LoginError> {
    let url = client.portal_url("beanfun_block/generic_handlers/record_service_start.ashx")?;

    let mut form: Vec<(&str, &str)> = vec![
        ("service_code", service_code),
        ("service_region", service_region),
        ("service_account_id", account.sid.as_str()),
        ("sotp", account.ssn.as_str()),
        ("service_account_display_name", account.sname.as_str()),
        ("service_account_create_time", step1.screatetime.as_str()),
    ];
    if let Some((k, v)) = &step1.unk_data {
        form.push((k.as_str(), v.as_str()));
    }

    // Same referrer requirement as the v2 endpoint — this handler
    // lives under `generic_handlers` too.
    let resp = client
        .http()
        .post(url)
        .header(reqwest::header::REFERER, step1.page_url.as_str())
        .form(&form)
        .send()
        .await?;
    ensure_success(&resp, "record_service_start.ashx")?;
    // Body deliberately not read — WPF discards `UploadString`'s
    // return value (L91-94). We must still consume the connection so
    // reqwest can return it to the pool, but we don't allocate
    // unnecessary text. `.bytes().await` would do it; calling
    // `.send()` already finishes when the headers arrive, so dropping
    // `resp` here is enough.
    drop(resp);
    Ok(())
}

/// Step 4 — `get_result.ashx` long-poll trigger. Response is also
/// discarded; the round-trip exists to drive the server-side OTP
/// generation pipeline before step 5 reads the result out.
async fn step_4_long_poll(
    client: &BeanfunClient,
    long_polling_key: &str,
) -> Result<(), LoginError> {
    let url = client.portal_url("generic_handlers/get_result.ashx")?;
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("meth", "GetResultByLongPolling"),
            ("key", long_polling_key),
            ("_", dt_iso_now().as_str()),
        ])
        .send()
        .await?;
    ensure_success(&resp, "get_result.ashx")?;
    drop(resp);
    Ok(())
}

/// Resolve the launcher identity off the async executor.
///
/// [`ClientIntegrity::resolve`] reads and hashes a ~1.3 MB file; a cold
/// cache makes that long enough to be worth handing to `spawn_blocking`.
/// A join failure (runtime shutting down) degrades to the bundled
/// constants rather than failing the OTP outright.
async fn resolve_client_integrity() -> ClientIntegrity {
    use crate::services::beanfun::ggm_hotfix;

    // 1. What the user pinned. An explicit choice outranks everything,
    //    including a newer published pair.
    if let Some(pinned) = tokio::task::spawn_blocking(ggm_hotfix::pinned)
        .await
        .ok()
        .flatten()
    {
        return ClientIntegrity::from_published(&pinned);
    }

    // 2 and 3. The GGM installed here, and what we published — whichever
    //    describes the newer build.
    //
    //    GGM updates itself, but only when it runs, and the people this
    //    app exists for are precisely the ones who never run it: they
    //    launch from here, not from the official site. An install that
    //    has sat untouched since Gamania last shipped reports what it was
    //    then, and those are the values beanfun stops accepting — so
    //    preferring it unconditionally would make the stalest machines
    //    the only ones the hotfix lever could never reach.
    //
    //    Preferring the published pair unconditionally trades that for
    //    the opposite hazard: a bad publish takes down users whose own
    //    install was fine. Comparing versions avoids both. A tie goes to
    //    the installed file, which is this machine's own truth rather
    //    than a claim about it.
    let local = tokio::task::spawn_blocking(ClientIntegrity::resolve_local)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "client-integrity resolve task failed");
            None
        });
    let published = ggm_hotfix::published().await;

    match (local, published) {
        (Some(local), Some(published)) => {
            if names_a_newer_build(&published.cv, &local.cv) {
                tracing::info!(
                    local = %local.cv,
                    published = %published.cv,
                    "published client-integrity is newer than the installed GGM"
                );
                ClientIntegrity::from_published(&published)
            } else {
                local
            }
        }
        (Some(local), None) => local,
        (None, Some(published)) => ClientIntegrity::from_published(&published),
        // 4. What we shipped with.
        (None, None) => ClientIntegrity::fallback(),
    }
}

/// Whether `candidate` names a strictly newer build than `current`.
///
/// Dotted numbers compared segment by segment, missing segments read as
/// zero so `1.5.1` beats `1.5`. Anything unparseable compares as zero,
/// which makes a malformed version lose rather than win — the safe
/// direction, since the loser is simply not used.
///
/// Not `core::version::is_newer`: that one answers a different question.
/// It compares release tags carrying a build timestamp, and treats a
/// matching timestamp as authoritative regardless of the numbers. A GGM
/// file version is four plain numbers with no stamp to match on.
fn names_a_newer_build(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect()
    }
    let (a, b) = (parts(candidate), parts(current));
    let width = a.len().max(b.len());
    for i in 0..width {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// Body of the v2 OTP request. Field names are PascalCase on the wire
/// except `arch`, matching the launcher verbatim.
#[derive(serde::Serialize)]
struct OtpV2Request<'a> {
    #[serde(rename = "SN")]
    sn: &'a str,
    #[serde(rename = "LaunchTicket")]
    launch_ticket: &'a str,
    #[serde(rename = "CV")]
    cv: &'a str,
    #[serde(rename = "Hash")]
    hash: &'a str,
    arch: &'a str,
}

/// Reply shape: `{ "result": 1, "data": "…", "message": null }`.
#[derive(serde::Deserialize)]
struct OtpV2Response {
    result: i64,
    data: Option<String>,
    message: Option<String>,
}

/// Step 5 (pre-v2, driven by the handoff) — `GET
/// get_webstart_otp.ashx` with the parameters the blob supplied.
///
/// This is the same endpoint the non-migrated path uses, but every
/// value comes from the page rather than from our own session state.
/// That matters most for `ppppp`: the WPF-era constant is stale, and
/// the live one arrives in the blob.
#[allow(clippy::too_many_arguments)]
async fn step_5_get_otp_from_handoff(
    client: &BeanfunClient,
    handoff: &LaunchHandoff,
    params: &LegacyOtpParams,
    web_token: &str,
    secret_code: &str,
    integrity: &ClientIntegrity,
    referer: &str,
) -> Result<String, LoginError> {
    let base = client.portal_url("beanfun_block/generic_handlers/get_webstart_otp.ashx")?;
    // As in the WPF builder: only the space in `CreateTime` needs
    // encoding; every other character in these values is URL-safe.
    let create_time = params.create_time.replace(' ', "%20");
    let url = format!(
        "{base}?SN={sn}&WebToken={web_token}&SecretCode={secret_code}&ppppp={ppppp}\
         &ServiceCode={sc}&ServiceRegion={sr}&ServiceAccount={sa}&CreateTime={create_time}\
         &d={tick}&CV={cv}&Hash={hash}&arch={arch}",
        sn = handoff.sn,
        ppppp = params.ppppp,
        sc = params.service_code,
        sr = params.service_region,
        sa = params.service_account,
        tick = tick_count_ms(),
        cv = utf8_percent_encode(&integrity.cv, ESCAPE_DATA_STRING),
        hash = utf8_percent_encode(&integrity.hash, ESCAPE_DATA_STRING),
        arch = utf8_percent_encode(integrity.arch, ESCAPE_DATA_STRING),
    );

    let resp = client
        .http()
        .get(&url)
        .header(reqwest::header::REFERER, referer)
        .send()
        .await?;
    ensure_success(&resp, "get_webstart_otp.ashx")?;
    let envelope = client.bounded_text(resp).await?;

    let result = step_6_decrypt(&envelope);
    if let Err(err) = &result {
        log_step_5_failure(err, "handoff/pre-v2", &url, &envelope);
    }
    result
}

/// Step 5 (v2) — `POST get_webstart_otp_v2.ashx` and decrypt the OTP
/// out of the JSON reply.
///
/// Not a replacement for the pre-v2 `GET`, though it read like one
/// while MapleStory was the only game anyone tested: the two are
/// siblings, and a game hands over whichever payload its own endpoint
/// wants ([`step_5_get_otp_from_handoff`] is the other half). So a
/// `Query String Error` from the old handler means the request was
/// built wrong, not that the handler is gone.
async fn step_5_post_otp_v2(
    client: &BeanfunClient,
    handoff: &LaunchHandoff,
    launch_ticket: &str,
    integrity: &ClientIntegrity,
    referer: &str,
) -> Result<String, LoginError> {
    let url = client.portal_url("beanfun_block/generic_handlers/get_webstart_otp_v2.ashx")?;
    let resp = client
        .http()
        .post(url)
        .header(reqwest::header::REFERER, referer)
        .json(&OtpV2Request {
            sn: &handoff.sn,
            launch_ticket,
            cv: &integrity.cv,
            hash: &integrity.hash,
            arch: integrity.arch,
        })
        .send()
        .await?;
    ensure_success(&resp, "get_webstart_otp_v2.ashx")?;
    let body = client.bounded_text(resp).await?;

    let parsed: OtpV2Response = serde_json::from_str(&body).map_err(|_| {
        tracing::warn!(
            body_len = body.len(),
            // Byte-slicing the body directly would panic on a preview
            // boundary that lands mid-codepoint.
            body_preview = %scrub(&snippet_for_diagnostics(&body)),
            "get_webstart_otp_v2.ashx returned a body that is not the expected JSON"
        );
        LoginError::OtpEmptyResponse
    })?;

    if parsed.result != 1 {
        return Err(LoginError::OtpServerRejected {
            // Prefer the server's own wording; fall back to the code so
            // the failure is never reported as an empty string.
            message: parsed
                .message
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| format!("result={}", parsed.result)),
        });
    }

    let payload = parsed
        .data
        .filter(|d| !d.is_empty())
        .ok_or(LoginError::OtpEmptyResponse)?;
    decrypt_otp_payload(&payload)
}

/// Step 5 — read the `1;{key}{ciphertext_hex}` envelope from
/// `get_webstart_otp.ashx`.
///
/// Takes the URL pre-built by [`build_get_webstart_otp_url`] (see that
/// function for why it is assembled as a string) so the caller keeps a
/// copy to log if the envelope turns out to be a rejection.
async fn step_5_get_otp(client: &BeanfunClient, url: &str) -> Result<String, LoginError> {
    let resp = client.http().get(url).send().await?;
    ensure_success(&resp, "get_webstart_otp.ashx")?;
    client.bounded_text(resp).await
}

/// Step 6 — split the `1;{key}{cipher}` envelope, DES-ECB-decrypt
/// `cipher` with `key`, then trim NUL bytes from both ends.
///
/// Pure function — no I/O. Extracted so unit tests can cover every
/// rejection branch (empty body, single segment, server-rejection,
/// invalid hex, non-block-aligned ciphertext) without spinning up
/// wiremock.
///
/// # 1:1 alignment notes
///
/// - **Splitter**: WPF L108 `response.Split(';')` followed by
///   `responses[1]` (L114) extracts only the **second** segment and
///   discards anything past the second `;`. We use `split(';')` +
///   index `[1]` rather than `splitn(2, ';')` so multi-`;`
///   adversarial server responses behave identically.
/// - **Key prefix length**: WPF L126 `response.Substring(0, 8)` is
///   char-based and would either succeed (≥ 8 UTF-16 units) or throw
///   `ArgumentOutOfRangeException` (< 8). We:
///   1. reject `payload.len() < 8` early as
///      [`LoginError::OtpDecryptionFailed`] (matches WPF's caught
///      exception → outer `errmsg = GetOtpError`),
///   2. additionally guard against `is_char_boundary(8) == false`
///      so we never panic on byte-8-mid-multibyte adversarial input
///      (WPF's char-based slice cannot panic; we restore that
///      invariant with an explicit typed error).
/// - **NUL trim**: WPF L131 `otp.Trim('\0')` strips NULs from
///   **both** ends. We use `trim_matches('\0')` (not
///   `trim_end_matches`) to preserve that exact semantics even
///   though production OTP payloads never carry leading NULs.
fn step_6_decrypt(envelope: &str) -> Result<String, LoginError> {
    if envelope.is_empty() {
        return Err(LoginError::OtpEmptyResponse);
    }
    let parts: Vec<&str> = envelope.split(';').collect();
    if parts.len() < 2 {
        return Err(LoginError::OtpEmptyResponse);
    }
    let status = parts[0];
    let payload = parts[1];
    if status != "1" {
        return Err(LoginError::OtpServerRejected {
            message: payload.to_string(),
        });
    }
    decrypt_otp_payload(payload)
}

/// Decrypt a `{8-char ASCII key}{ciphertext hex}` OTP payload.
///
/// Shared by both protocol versions: the pre-v2 envelope puts this
/// after `1;`, and `get_webstart_otp_v2.ashx` returns the same shape
/// as its JSON `data` member. The 40-character `data` observed from
/// the official launcher is exactly 8 + 32 hex = 8 + two DES blocks,
/// which is what identifies it as this construction.
fn decrypt_otp_payload(payload: &str) -> Result<String, LoginError> {
    if payload.len() < 8 {
        return Err(LoginError::OtpDecryptionFailed {
            cause: format!(
                "payload too short to contain 8-byte key prefix (got {} bytes)",
                payload.len()
            ),
        });
    }
    if !payload.is_char_boundary(8) {
        return Err(LoginError::OtpDecryptionFailed {
            cause: "key prefix straddles a multi-byte UTF-8 boundary".to_string(),
        });
    }
    let (key, cipher_hex) = payload.split_at(8);
    let plain = decrypt_hex(cipher_hex, key).map_err(|e| LoginError::OtpDecryptionFailed {
        cause: e.to_string(),
    })?;
    Ok(plain.trim_matches('\0').to_string())
}

// -----------------------------------------------------------------------------
// Pure parsing helpers (unit-tested below)
// -----------------------------------------------------------------------------

/// Extract the `key=...` value from the inline JS literal
/// `GetResultByLongPolling&key=ABCDEF"` that step 1 returns.
///
/// Matches the WPF regex at L36 verbatim: the closing `"` is part of
/// the pattern so the capture group stops at it.
///
/// **Deliberate divergence from WPF** (see the "Empty captures" note
/// in the module docs): an empty capture is rejected here rather than
/// forwarded as `SN=` on step 5's URL.
fn parse_long_polling_key(html: &str) -> Result<String, LoginError> {
    capture_first(long_polling_key_regex(), html)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LoginError::OtpMissingLongPollingKey {
            snippet: snippet_for_diagnostics(html),
        })
}

/// Extract the `(key, value)` pair from the TW-only inline JS literal
/// `MyAccountData.ServiceAccountCreateTime + "k=v";`.
///
/// Both halves are percent-decoded, mirroring WPF's
/// `Uri.UnescapeDataString` calls (L53-54). `Uri.UnescapeDataString`
/// only decodes `%XX` sequences and treats `+` as a literal `+`,
/// which matches `percent_encoding::percent_decode_str` exactly
/// (form-encoded `+` → space would be the wrong choice here). Step 3
/// will then re-encode via reqwest's form builder.
fn parse_unk_data(html: &str) -> Result<(String, String), LoginError> {
    let caps = unk_data_regex()
        .captures(html)
        .ok_or(LoginError::OtpMissingUnkData)?;
    let raw_key = caps.get(1).map_or("", |m| m.as_str());
    let raw_value = caps.get(2).map_or("", |m| m.as_str());
    let key = percent_decode_str(raw_key)
        .decode_utf8()
        .map_err(|_| LoginError::OtpMissingUnkData)?
        .into_owned();
    let value = percent_decode_str(raw_value)
        .decode_utf8()
        .map_err(|_| LoginError::OtpMissingUnkData)?
        .into_owned();
    Ok((key, value))
}

/// Fallback path for `account.screatetime == None`: re-parse the
/// `ServiceAccountCreateTime: "..."` literal from step 1's response.
///
/// Re-uses the same regex as P4.1's
/// [`crate::core::parser::extract_service_account_create_time`] so we
/// keep one source of truth for the pattern.
///
/// Needs no empty-capture guard (unlike its two sibling parsers): the
/// shared regex is `ServiceAccountCreateTime: "([^"]+)"`, whose `+`
/// already makes an empty match impossible.
fn parse_screatetime_fallback(html: &str) -> Result<String, LoginError> {
    extract_service_account_create_time(html).ok_or(LoginError::OtpMissingCreateTime)
}

/// Extract `m_objData` from step 1's response.
///
/// Returns `None` when the literal is absent or either member is
/// missing — the page shape differs by region, and only the v2 OTP
/// path needs this, so a miss is not an error at scrape time.
fn parse_launch_handoff(html: &str) -> Option<LaunchHandoff> {
    let block = capture_first(launch_object_regex(), html)?;
    let sn = capture_first(launch_sn_regex(), &block)?;
    let data = capture_first(launch_data_regex(), &block)?;
    if sn.is_empty() || data.is_empty() {
        return None;
    }
    Some(LaunchHandoff {
        sn,
        data,
        web_token: capture_first(launch_web_token_regex(), &block).filter(|v| !v.is_empty()),
        secret_code: capture_first(launch_secret_code_regex(), &block).filter(|v| !v.is_empty()),
    })
}

/// Extract the `m_strSecretCode` JS literal from step 2's response.
///
/// **Deliberate divergence from WPF** (see the "Empty captures" note
/// in the module docs): the pattern's `'(.*)'` matches a literal
/// `var m_strSecretCode = '';` and captures an empty string, which
/// WPF would forward as `SecretCode=` on step 5's URL. Rejecting it
/// here surfaces `OtpMissingSecretCode` at step 2 — the step that
/// actually failed — instead of an opaque step-5 server rejection.
fn parse_secret_code(html: &str) -> Result<String, LoginError> {
    capture_first(secret_code_regex(), html)
        .filter(|s| !s.is_empty())
        .ok_or(LoginError::OtpMissingSecretCode)
}

// -----------------------------------------------------------------------------
// Regex helpers (compiled once, OnceLock convention shared with parser/*)
// -----------------------------------------------------------------------------

fn long_polling_key_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"GetResultByLongPolling&key=(.*)""#)
            .expect("long polling key regex must compile")
    })
}

fn unk_data_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Pattern is byte-for-byte WPF L46 (`MyAccountData.ServiceAccountCreateTime
    // \\+ \"(.*)=(.*)\";`). Note the `.` between `MyAccountData` and
    // `ServiceAccountCreateTime` is **not** escaped — WPF leaves it as a regex
    // wildcard. We mirror that exactly so any divergence in adversarial server
    // output behaves the same as WPF (the 1:1 alignment audit caught an
    // earlier escaped `\.` here).
    RE.get_or_init(|| {
        Regex::new(r#"MyAccountData.ServiceAccountCreateTime \+ "(.*)=(.*)";"#)
            .expect("unk data regex must compile")
    })
}

fn secret_code_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"var m_strSecretCode = '(.*)';"#).expect("secret code regex must compile")
    })
}

/// The `m_objData = { … }` object literal. `(?s)` lets it span lines
/// (the page pretty-prints it) and the lazy `.*?` stops at the first
/// closing brace, which is the end of this flat literal.
fn launch_object_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?s)var m_objData\s*=\s*\{(.*?)\}"#)
            .expect("launch object regex must compile")
    })
}

/// `"sn"` inside that literal. Applied to the captured block, not the
/// whole page, so the generic key name cannot match something else.
fn launch_sn_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""sn"\s*:\s*"([^"]*)""#).expect("launch sn regex must compile"))
}

/// `"data"` inside that literal — the obfuscated `LaunchTicket` blob.
fn launch_data_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#""data"\s*:\s*"([^"]*)""#).expect("launch data regex must compile")
    })
}

/// `"webToken"` inside that literal. Only pages carrying the pre-v2
/// payload declare it.
fn launch_web_token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#""webToken"\s*:\s*"([^"]*)""#).expect("launch web token regex must compile")
    })
}

/// `"secretCode"` inside that literal. As above.
fn launch_secret_code_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#""secretCode"\s*:\s*"([^"]*)""#)
            .expect("launch secret code regex must compile")
    })
}

// -----------------------------------------------------------------------------
// Step 5 URL builder + tick counter (private)
// -----------------------------------------------------------------------------

/// 64-character uppercase hex literal sent as `ppppp=` on step 5.
///
/// Verbatim copy of WPF L101. Provenance is unknown — the server
/// appears to validate it as a protocol constant. Do not modify
/// without empirical verification against the production server.
const PPPPP_LITERAL: &str = "1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA";

/// Mirror WPF's `Environment.TickCount` for the step 5 `d=` cache
/// buster.
///
/// .NET's `Environment.TickCount` is a 32-bit signed millisecond
/// counter that wraps around every ~24.8 days. The server only uses
/// the value as an opaque cache buster (it never validates the
/// magnitude or sign), so any reasonably-unique `i32` works. We use
/// the bottom 32 bits of `Local::now().timestamp_millis()` to keep
/// the type and overall shape identical to WPF.
fn tick_count_ms() -> i32 {
    Local::now().timestamp_millis() as i32
}

/// Build step 5's URL as a literal string.
///
/// Argument order mirrors the WPF URL template (L100-102) so a side-
/// by-side diff against `BeanfunClient.OTP.cs` is mechanical.
///
/// The URL is assembled **as a string** rather than via reqwest's
/// `.query()` builder because two parameters require WPF-specific
/// encoding that the form-urlencoder would emit differently:
///
/// 1. `CreateTime` contains a literal space (e.g. `2024-01-15 12:34:56`)
///    that WPF replaces with `%20` (L101 `acc.screatetime.Replace(" ", "%20")`).
///    reqwest's `.query()` would emit `+` instead, which most servers
///    accept but is **not** byte-identical to the WPF wire format.
/// 2. `ppppp` is a 64-char uppercase hex literal that must appear
///    verbatim — no encoding, no normalisation.
///
/// All other characters in the URL (cookies, sids, hex digits) are
/// already URL-safe, so a literal `format!` is sufficient.
/// Characters `Uri.EscapeDataString` leaves alone: the RFC 3986
/// unreserved set (`ALPHA / DIGIT / "-" / "." / "_" / "~"`).
///
/// [`NON_ALPHANUMERIC`] escapes the four punctuation marks too, so they
/// are removed to match .NET exactly — a dotted version would otherwise
/// go out as `1%2E5%2E0%2E2`.
const ESCAPE_DATA_STRING: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[allow(clippy::too_many_arguments)]
fn build_get_webstart_otp_url(
    client: &BeanfunClient,
    session: &Session,
    account: &ServiceAccount,
    step1: &Step1Data,
    secret_code: &str,
    service_code: &str,
    service_region: &str,
    tick: i32,
    integrity: Option<&ClientIntegrity>,
) -> Result<String, LoginError> {
    let base = client.portal_url("beanfun_block/generic_handlers/get_webstart_otp.ashx")?;
    // WPF replaces only spaces with `%20`; every other char in the
    // screatetime format (`yyyy-MM-dd HH:mm:ss`) is already URL-safe.
    let create_time_encoded = step1.screatetime.replace(' ', "%20");
    let mut url = format!(
        "{base}?SN={sn}&WebToken={web_token}&SecretCode={secret_code}&ppppp={ppppp}&ServiceCode={sc}&ServiceRegion={sr}&ServiceAccount={sid}&CreateTime={create_time}&d={tick}",
        base = base,
        sn = step1.long_polling_key,
        web_token = session.web_token,
        secret_code = secret_code,
        ppppp = PPPPP_LITERAL,
        sc = service_code,
        sr = service_region,
        sid = account.sid,
        create_time = create_time_encoded,
        tick = tick,
    );
    if let Some(integrity) = integrity {
        use std::fmt::Write as _;
        // Appended last, in this order, exactly as `BuildOtpUrl` does.
        // Infallible for a String sink; the result is discarded rather
        // than unwrapped to keep the builder panic-free.
        let _ = write!(
            url,
            "&CV={cv}&Hash={hash}&arch={arch}",
            cv = utf8_percent_encode(&integrity.cv, ESCAPE_DATA_STRING),
            hash = utf8_percent_encode(&integrity.hash, ESCAPE_DATA_STRING),
            arch = utf8_percent_encode(integrity.arch, ESCAPE_DATA_STRING),
        );
    }
    Ok(url)
}

/// Secret-bearing query parameters of step 5's URL. Their values are
/// replaced with a length when the URL is logged.
///
/// `ppppp` is here because it stopped being a constant. It was exempt
/// while the only one we ever sent was [`PPPPP_LITERAL`], printed a few
/// lines above in this same file — nothing was disclosed by logging a
/// value the reader could already see. The handoff route takes it from
/// the launch blob instead, and that one may be per-session or
/// per-launch, so on that path the exemption would write a live value
/// to disk.
const OTP_URL_SECRET_PARAMS: [&str; 5] =
    ["SN", "WebToken", "SecretCode", "ServiceAccount", "ppppp"];

/// Rewrite step 5's URL so it is safe to put in a log line.
///
/// Every parameter *name* survives, and the non-secret values stay
/// verbatim on purpose — a reformatted `CreateTime` or a wrong
/// `ServiceCode` are exactly what a rejection diagnosis needs to see.
/// The session-scoped values collapse to `<N chars>`, which still
/// distinguishes "absent", "truncated" and "present" without putting a
/// live token on disk. A length is enough for `ppppp` too: what a
/// diagnosis wants from it is whether it is the 64-character constant
/// or the 96-character value the blob now supplies.
fn redact_otp_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let redacted = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) if OTP_URL_SECRET_PARAMS.contains(&key) => {
                format!("{key}=<{} chars>", value.len())
            }
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{redacted}")
}

// -----------------------------------------------------------------------------
// Misc helpers
// -----------------------------------------------------------------------------

/// Truncate `body` to a small bounded snippet for inclusion in
/// diagnostic error messages. WPF stuffs the entire response body
/// into `errmsg` (L39 `"OTPNoLongPollingKey:" + response`); we cap at
/// a reasonable length so the error doesn't carry several MB of HTML
/// around if the server returns an unexpected page.
fn snippet_for_diagnostics(body: &str) -> String {
    const LIMIT: usize = 256;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        // Find a char boundary at or before LIMIT to avoid splitting
        // a multi-byte UTF-8 sequence. `floor_char_boundary` would be
        // cleaner but is unstable; this loop is O(LIMIT) worst case.
        let mut end = LIMIT;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &body[..end])
    }
}

// -----------------------------------------------------------------------------
// Tests (pure helpers; integration tests live in tests/otp.rs)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn a_newer_published_version_wins() {
        // The case that motivated comparing at all: an install left alone
        // since before Gamania's last release.
        assert!(names_a_newer_build("1.5.0.2", "1.4.9.9"));
        assert!(names_a_newer_build("1.5.1", "1.5.0.2"));
        assert!(names_a_newer_build("2.0", "1.9.9.9"));
    }

    #[test]
    fn an_equal_or_older_published_version_does_not() {
        // A tie goes to the installed file, and a publish that names an
        // older build is a mistake that must not take working machines down.
        assert!(!names_a_newer_build("1.5.0.2", "1.5.0.2"));
        assert!(
            !names_a_newer_build("1.5.0", "1.5.0.0"),
            "missing segments read as zero"
        );
        assert!(!names_a_newer_build("1.4.0", "1.5.0.2"));
    }

    #[test]
    fn an_unreadable_version_loses() {
        // Whatever cannot be parsed compares as zero. Losing is the safe
        // direction: the loser is simply not used.
        assert!(!names_a_newer_build("", "1.0"));
        assert!(!names_a_newer_build("not.a.version", "0.0.1"));
        assert!(names_a_newer_build("0.0.1", "not.a.version"));
    }

    use super::*;

    // -------------------------------------------------------------------------
    // parse_long_polling_key
    // -------------------------------------------------------------------------

    #[test]
    fn long_polling_key_extracts_value_between_equals_and_quote() {
        let html = r#"<script>x = "GetResultByLongPolling&key=ABC123XYZ"</script>"#;
        assert_eq!(parse_long_polling_key(html).unwrap(), "ABC123XYZ");
    }

    #[test]
    fn long_polling_key_missing_returns_typed_error_with_snippet() {
        let html = "<html>no key here</html>";
        match parse_long_polling_key(html).unwrap_err() {
            LoginError::OtpMissingLongPollingKey { snippet } => {
                assert!(snippet.contains("no key here"));
            }
            other => panic!("expected OtpMissingLongPollingKey, got {other:?}"),
        }
    }

    #[test]
    fn long_polling_key_snippet_is_bounded_for_giant_bodies() {
        let html = format!("<html>{}</html>", "x".repeat(5000));
        match parse_long_polling_key(&html).unwrap_err() {
            LoginError::OtpMissingLongPollingKey { snippet } => {
                assert!(
                    snippet.len() <= 260,
                    "snippet should be bounded, got {} chars",
                    snippet.len()
                );
            }
            other => panic!("expected OtpMissingLongPollingKey, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // parse_unk_data
    // -------------------------------------------------------------------------

    #[test]
    fn unk_data_decodes_url_encoded_key_and_value() {
        let html = r#"foo = MyAccountData.ServiceAccountCreateTime + "k%5Bx%5D=v%20bar";"#;
        let (k, v) = parse_unk_data(html).unwrap();
        assert_eq!(k, "k[x]");
        assert_eq!(v, "v bar");
    }

    #[test]
    fn unk_data_missing_returns_typed_error() {
        let html = "<html>nothing useful</html>";
        assert!(matches!(
            parse_unk_data(html).unwrap_err(),
            LoginError::OtpMissingUnkData
        ));
    }

    // -------------------------------------------------------------------------
    // parse_screatetime_fallback
    // -------------------------------------------------------------------------

    #[test]
    fn screatetime_fallback_present_returns_value() {
        let html = r#"x = ServiceAccountCreateTime: "2024-01-15 12:34:56"; y = 1;"#;
        assert_eq!(
            parse_screatetime_fallback(html).unwrap(),
            "2024-01-15 12:34:56"
        );
    }

    #[test]
    fn screatetime_fallback_absent_returns_typed_error() {
        let html = "<html>nothing relevant</html>";
        assert!(matches!(
            parse_screatetime_fallback(html).unwrap_err(),
            LoginError::OtpMissingCreateTime
        ));
    }

    // -------------------------------------------------------------------------
    // parse_secret_code
    // -------------------------------------------------------------------------

    #[test]
    fn secret_code_extracts_value_between_single_quotes() {
        let html = r#"<script>var m_strSecretCode = 'sEcReT-1234';</script>"#;
        assert_eq!(parse_secret_code(html).unwrap(), "sEcReT-1234");
    }

    #[test]
    fn secret_code_missing_returns_typed_error() {
        let html = "<html>no secret here</html>";
        assert!(matches!(
            parse_secret_code(html).unwrap_err(),
            LoginError::OtpMissingSecretCode
        ));
    }

    // -------------------------------------------------------------------------
    // Empty captures — see the module-level "Empty captures" note.
    //
    // Each of these pages *matches* its regex but captures "". Before
    // the non-empty filter they returned `Ok("")`, which step 5 spliced
    // into its URL as a bare `Param=` and the portal bounced with a
    // generic "Query String Error" — attributing an upstream failure to
    // the wrong step.
    // -------------------------------------------------------------------------

    #[test]
    fn empty_secret_code_capture_is_rejected() {
        let html = r#"<script>var m_strSecretCode = '';</script>"#;
        assert!(matches!(
            parse_secret_code(html).unwrap_err(),
            LoginError::OtpMissingSecretCode
        ));
    }

    #[test]
    fn empty_long_polling_key_capture_is_rejected() {
        let html = r#"<script>x = "GetResultByLongPolling&key=";</script>"#;
        assert!(matches!(
            parse_long_polling_key(html).unwrap_err(),
            LoginError::OtpMissingLongPollingKey { .. }
        ));
    }

    /// The `CreateTime` scrape needs no guard of its own — its regex
    /// uses `([^"]+)`, so an empty value is a miss, not an empty
    /// capture. Pinned here so a future loosening of that pattern to
    /// `(.*)` fails loudly instead of silently reopening the hole.
    #[test]
    fn empty_screatetime_is_a_miss_not_an_empty_capture() {
        let html = r#"x = ServiceAccountCreateTime: ""; y = 1;"#;
        assert!(matches!(
            parse_screatetime_fallback(html).unwrap_err(),
            LoginError::OtpMissingCreateTime
        ));
    }

    // -------------------------------------------------------------------------
    // step_6_decrypt
    // -------------------------------------------------------------------------

    #[test]
    fn step6_empty_envelope_returns_empty_response_error() {
        assert!(matches!(
            step_6_decrypt("").unwrap_err(),
            LoginError::OtpEmptyResponse
        ));
    }

    #[test]
    fn step6_single_segment_returns_empty_response_error() {
        // No `;` separator at all → `split(';')` yields 1 segment →
        // `parts.len() < 2` → OtpEmptyResponse, matching WPF L109-112
        // `responses.Length < 2` branch.
        assert!(matches!(
            step_6_decrypt("only-one-part").unwrap_err(),
            LoginError::OtpEmptyResponse
        ));
    }

    #[test]
    fn step6_status_not_one_surfaces_server_message_verbatim() {
        match step_6_decrypt("0;maintenance in progress").unwrap_err() {
            LoginError::OtpServerRejected { message } => {
                assert_eq!(message, "maintenance in progress");
            }
            other => panic!("expected OtpServerRejected, got {other:?}"),
        }
    }

    #[test]
    fn step6_payload_shorter_than_key_prefix_is_decrypt_error() {
        // status = "1", payload = "ABC" (only 3 chars, < 8-byte key prefix).
        match step_6_decrypt("1;ABC").unwrap_err() {
            LoginError::OtpDecryptionFailed { cause } => {
                assert!(cause.contains("payload too short"));
            }
            other => panic!("expected OtpDecryptionFailed, got {other:?}"),
        }
    }

    #[test]
    fn step6_invalid_hex_is_decrypt_error() {
        // status = "1", key = "12345678", ciphertext = "ZZ" (not hex).
        match step_6_decrypt("1;12345678ZZZZZZZZZZZZZZZZ").unwrap_err() {
            LoginError::OtpDecryptionFailed { cause } => {
                assert!(!cause.is_empty(), "cause should describe the wcdes error");
            }
            other => panic!("expected OtpDecryptionFailed, got {other:?}"),
        }
    }

    #[test]
    fn step6_happy_path_decrypts_and_trims_nul_padding() {
        // Generate a valid encrypted envelope: encrypt 8 bytes with a
        // known key, then assert decrypt round-trips back.
        use crate::core::wcdes::encrypt_hex;
        let key = "ABCDEFGH"; // 8 ASCII bytes
        let plain = "12345678"; // exactly one DES block
        let cipher_hex = encrypt_hex(plain, key).unwrap();
        let envelope = format!("1;{key}{cipher_hex}");
        assert_eq!(step_6_decrypt(&envelope).unwrap(), plain);
    }

    #[test]
    fn step6_trims_trailing_nul_bytes() {
        // Encrypt a string that decrypts cleanly to 8 bytes including
        // trailing NULs (e.g. "AB\0\0\0\0\0\0"), and assert the NULs
        // are stripped — matching WPF L131 `otp.Trim('\0')`.
        use crate::core::wcdes::encrypt_hex;
        let key = "ABCDEFGH";
        let plain = "AB\0\0\0\0\0\0"; // 8 bytes, NUL-padded
        let cipher_hex = encrypt_hex(plain, key).unwrap();
        let envelope = format!("1;{key}{cipher_hex}");
        assert_eq!(step_6_decrypt(&envelope).unwrap(), "AB");
    }

    #[test]
    fn step6_trims_leading_nul_bytes_too() {
        // WPF L131 `otp.Trim('\0')` strips NULs from BOTH ends. We
        // mirror that with `trim_matches('\0')`. Production OTP
        // payloads never carry leading NULs but the contract is
        // observable so we lock it down — earlier alignment audit
        // caught a `trim_end_matches` regression here.
        use crate::core::wcdes::encrypt_hex;
        let key = "ABCDEFGH";
        let plain = "\0\0AB\0\0\0\0"; // 8 bytes, NULs at both ends
        let cipher_hex = encrypt_hex(plain, key).unwrap();
        let envelope = format!("1;{key}{cipher_hex}");
        assert_eq!(step_6_decrypt(&envelope).unwrap(), "AB");
    }

    #[test]
    fn step6_extra_semicolons_after_payload_are_ignored() {
        // WPF L108 `Split(';')` + L114 `responses[1]` extracts only
        // the second segment and silently drops anything after it.
        // We must behave identically (i.e. NOT use `splitn(2, ';')`
        // which would fold the trailing junk into the payload and
        // corrupt the cipher hex slice).
        use crate::core::wcdes::encrypt_hex;
        let key = "ABCDEFGH";
        let plain = "12345678";
        let cipher_hex = encrypt_hex(plain, key).unwrap();
        // Append a third `;segment` that WPF would discard.
        let envelope = format!("1;{key}{cipher_hex};junk;more");
        assert_eq!(step_6_decrypt(&envelope).unwrap(), plain);
    }

    #[test]
    fn step6_payload_with_multibyte_char_straddling_byte_8_is_typed_error() {
        // Byte length is ≥ 8 so the `< 8` guard does NOT trigger,
        // but a multi-byte UTF-8 character crosses byte index 8 so
        // `split_at(8)` would panic without the `is_char_boundary`
        // guard. WPF's char-based `Substring(0, 8)` cannot panic on
        // this input — it would slice 8 *characters* and continue
        // (eventually erroring inside DecryStrHex). We surface a
        // typed error instead of panicking, which is strictly safer
        // for adversarial server output and matches the spirit of
        // WPF's "always reach the catch block, never crash" model.
        // Layout: bytes 0..6 = "ABCDEFG", bytes 7..10 = '中' (3
        // bytes), bytes 10..12 = "HI" → byte 8 is mid-'中'.
        let payload = "ABCDEFG中HI";
        assert!(
            !payload.is_char_boundary(8),
            "test fixture invariant: byte 8 must straddle a char"
        );
        let envelope = format!("1;{payload}");
        match step_6_decrypt(&envelope).unwrap_err() {
            LoginError::OtpDecryptionFailed { cause } => {
                assert!(
                    cause.contains("multi-byte"),
                    "cause should explain the boundary issue, got: {cause}"
                );
            }
            other => panic!("expected OtpDecryptionFailed, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // build_get_webstart_otp_url
    // -------------------------------------------------------------------------

    /// Shared fixture for the legacy URL-builder tests.
    fn step5_url_fixture(region: LoginRegion, integrity: Option<&ClientIntegrity>) -> String {
        use crate::services::beanfun::client::ClientConfig;
        let client = BeanfunClient::new(ClientConfig::for_region(region)).unwrap();
        let session = Session::new(
            region,
            "SKEY_X",
            "WEB_TOKEN_X",
            "ACCOUNT_ID_X",
            "610074",
            "T9",
        );
        let account = ServiceAccount {
            is_enable: true,
            visible: true,
            is_inherited: false,
            sid: "SID_1".to_string(),
            ssn: "SSN_1".to_string(),
            sname: "name".to_string(),
            screatetime: Some("2024-01-15 12:34:56".to_string()),
            slastusedtime: None,
            sauthtype: None,
        };
        let step1 = Step1Data {
            page_url: "https://tw.beanfun.com/beanfun_block/game_zone/game_start_step2.aspx"
                .to_string(),
            long_polling_key: "LPK".to_string(),
            unk_data: None,
            screatetime: "2024-01-15 12:34:56".to_string(),
            // These tests cover the legacy URL builder, which is the
            // path taken precisely when the page carried no handoff.
            launch: None,
        };
        build_get_webstart_otp_url(
            &client, &session, &account, &step1, "SECRET", "610074", "T9", 12345, integrity,
        )
        .unwrap()
    }

    fn test_integrity() -> ClientIntegrity {
        ClientIntegrity {
            cv: "1.5.0.2".to_string(),
            hash: "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06".to_string(),
            arch: "x64",
        }
    }

    #[test]
    fn step5_url_replaces_screatetime_spaces_with_percent20() {
        // Build a tiny client + session and verify the URL string
        // contains `%20` (not `+`) where screatetime had a space.
        let url = step5_url_fixture(LoginRegion::TW, None);

        assert!(
            url.contains("CreateTime=2024-01-15%2012:34:56"),
            "got: {url}"
        );
        assert!(
            !url.contains("CreateTime=2024-01-15+12:34:56"),
            "got: {url}"
        );
        assert!(
            url.contains(&format!("ppppp={PPPPP_LITERAL}")),
            "got: {url}"
        );
        assert!(url.contains("WebToken=WEB_TOKEN_X"), "got: {url}");
        assert!(url.contains("SN=LPK"), "got: {url}");
        assert!(url.contains("SecretCode=SECRET"), "got: {url}");
        assert!(url.contains("ServiceCode=610074"), "got: {url}");
        assert!(url.contains("ServiceRegion=T9"), "got: {url}");
        assert!(url.contains("ServiceAccount=SID_1"), "got: {url}");
        assert!(url.contains("d=12345"), "got: {url}");
    }

    // -------------------------------------------------------------------------
    // build_get_webstart_otp_url — client-integrity suffix
    // -------------------------------------------------------------------------

    #[test]
    fn step5_url_appends_client_integrity_suffix_after_the_cache_buster() {
        // `BuildOtpUrl` appends CV/Hash/arch last, in that order, after
        // `&d=`. Pin the whole tail so a future reordering is caught.
        let integrity = test_integrity();
        let url = step5_url_fixture(LoginRegion::TW, Some(&integrity));

        assert!(
            url.ends_with(
                "&d=12345&CV=1.5.0.2&Hash=dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06&arch=x64"
            ),
            "got: {url}",
        );
    }

    #[test]
    fn step5_url_omits_the_suffix_entirely_when_integrity_is_absent() {
        // The HK path: not an empty `&CV=&Hash=&arch=`, but no suffix at
        // all, so HK's request stays byte-identical to before.
        let url = step5_url_fixture(LoginRegion::HK, None);

        assert!(url.ends_with("&d=12345"), "got: {url}");
        assert!(!url.contains("CV="), "got: {url}");
        assert!(!url.contains("Hash="), "got: {url}");
        assert!(!url.contains("arch="), "got: {url}");
    }

    #[test]
    fn step5_url_leaves_unreserved_characters_in_the_suffix_unescaped() {
        // `Uri.EscapeDataString` preserves `-._~`; a naive
        // `NON_ALPHANUMERIC` set would emit `1%2E5%2E0%2E2` and the
        // server would reject the version.
        let integrity = ClientIntegrity {
            cv: "1.5.0.2-beta_1~x".to_string(),
            hash: "abc".to_string(),
            arch: "x64",
        };
        let url = step5_url_fixture(LoginRegion::TW, Some(&integrity));

        assert!(url.contains("&CV=1.5.0.2-beta_1~x"), "got: {url}");
        assert!(!url.contains("%2E"), "got: {url}");
        assert!(!url.contains("%2D"), "got: {url}");
        assert!(!url.contains("%5F"), "got: {url}");
        assert!(!url.contains("%7E"), "got: {url}");
    }

    #[test]
    fn step5_url_percent_encodes_reserved_characters_in_the_suffix() {
        // Defensive: a malformed local launcher version must not be
        // able to inject extra query parameters into the request.
        let integrity = ClientIntegrity {
            cv: "1.0&Hash=evil".to_string(),
            hash: "aa".to_string(),
            arch: "x64",
        };
        let url = step5_url_fixture(LoginRegion::TW, Some(&integrity));

        assert!(url.contains("&CV=1.0%26Hash%3Devil"), "got: {url}");
        // Exactly one real `Hash=` parameter survives.
        assert_eq!(url.matches("&Hash=").count(), 1, "got: {url}");
    }

    // -------------------------------------------------------------------------
    // parse_launch_handoff
    // -------------------------------------------------------------------------

    /// Shaped like the real page: pretty-printed across lines, quoted
    /// keys, and a `region` member we deliberately ignore.
    const LAUNCH_LITERAL: &str = r#"
        var supportService = ['300148', '610074'];
        var m_objData = {
            "region": "TW;Production",
            "sn": "11111111-2222-3333-4444-555555555555",
            "data": "5abcdef0123456789"
        };
        var ggmCallback = false;
    "#;

    /// The shape every non-MapleStory game was observed to serve: the
    /// same literal plus the two values the pre-v2 OTP URL needs.
    const LAUNCH_LITERAL_WITH_TOKENS: &str = r#"
        var m_objData = {
            "region": "TW;Production",
            "sn": "c65aa8c4-ff3a-4372-a842-7e69990f8bee",
            "webToken": "aaaabbbbccccddddeeeeffff00001111",
            "secretCode": "1111000fffeeeeddddccccbbbbaaaa22",
            "data": "7abcdef0123456789"
        };
    "#;

    #[test]
    fn launch_handoff_extracts_sn_and_data() {
        let handoff = parse_launch_handoff(LAUNCH_LITERAL).expect("literal should parse");
        assert_eq!(handoff.sn, "11111111-2222-3333-4444-555555555555");
        assert_eq!(handoff.data, "5abcdef0123456789");
    }

    /// A page that declares neither token is the `LaunchTicket` shape;
    /// nothing may invent values it did not send.
    #[test]
    fn launch_handoff_without_tokens_leaves_them_absent() {
        let handoff = parse_launch_handoff(LAUNCH_LITERAL).expect("literal should parse");
        assert!(handoff.web_token.is_none());
        assert!(handoff.secret_code.is_none());
    }

    #[test]
    fn launch_handoff_extracts_the_tokens_when_the_page_declares_them() {
        let handoff =
            parse_launch_handoff(LAUNCH_LITERAL_WITH_TOKENS).expect("literal should parse");
        assert_eq!(handoff.sn, "c65aa8c4-ff3a-4372-a842-7e69990f8bee");
        assert_eq!(
            handoff.web_token.as_deref(),
            Some("aaaabbbbccccddddeeeeffff00001111")
        );
        assert_eq!(
            handoff.secret_code.as_deref(),
            Some("1111000fffeeeeddddccccbbbbaaaa22")
        );
    }

    #[test]
    fn launch_handoff_absent_is_none_not_an_error() {
        // The legacy path must keep working on a page without the
        // literal, so a miss is `None` rather than a failure.
        assert!(parse_launch_handoff("<html>no launcher handoff here</html>").is_none());
    }

    #[test]
    fn launch_handoff_with_empty_members_is_none() {
        let html = r#"var m_objData = { "sn": "", "data": "" };"#;
        assert!(parse_launch_handoff(html).is_none());
    }

    #[test]
    fn launch_handoff_stops_at_the_literals_own_brace() {
        // A later object on the page must not be absorbed into the
        // capture by a greedy match.
        let html = r#"
            var m_objData = { "sn": "SN-1", "data": "D-1" };
            var other = { "sn": "SN-2", "data": "D-2" };
        "#;
        let handoff = parse_launch_handoff(html).expect("should parse");
        assert_eq!(handoff.sn, "SN-1");
        assert_eq!(handoff.data, "D-1");
    }

    // -------------------------------------------------------------------------
    // OtpV2Response
    // -------------------------------------------------------------------------

    #[test]
    fn otp_v2_response_parses_the_observed_shape() {
        let parsed: OtpV2Response =
            serde_json::from_str(r#"{"result":1,"data":"abcdefgh0123","message":null}"#).unwrap();
        assert_eq!(parsed.result, 1);
        assert_eq!(parsed.data.as_deref(), Some("abcdefgh0123"));
        assert!(parsed.message.is_none());
    }

    #[test]
    fn otp_v2_response_carries_a_server_message_on_failure() {
        let parsed: OtpV2Response =
            serde_json::from_str(r#"{"result":0,"data":null,"message":"nope"}"#).unwrap();
        assert_eq!(parsed.result, 0);
        assert_eq!(parsed.message.as_deref(), Some("nope"));
    }

    // -------------------------------------------------------------------------
    // decrypt_otp_payload
    // -------------------------------------------------------------------------

    #[test]
    fn otp_payload_round_trips_through_the_shared_decryptor() {
        // 8 bytes of plaintext = one DES block, giving 8 + 16 hex = a
        // 24-character payload of the same shape as the wire format.
        let key = "a1b2c3d4";
        let cipher_hex = crate::core::wcdes::encrypt_hex("OTP12345", key).unwrap();
        let payload = format!("{key}{cipher_hex}");
        assert_eq!(decrypt_otp_payload(&payload).unwrap(), "OTP12345");
    }

    /// The launcher's reply carries a 40-character `data`. That is
    /// exactly an 8-character key plus 32 hex characters — two DES
    /// blocks — which is what identifies it as the same envelope the
    /// pre-v2 protocol used. Pinned so the reasoning survives.
    #[test]
    fn observed_v2_payload_length_is_key_plus_whole_des_blocks() {
        let hex_len = 40 - 8;
        assert_eq!(hex_len % 2, 0);
        assert_eq!((hex_len / 2) % 8, 0);
    }

    // -------------------------------------------------------------------------
    // redact_otp_url
    // -------------------------------------------------------------------------

    #[test]
    fn redact_otp_url_hides_secrets_but_keeps_diagnostic_values() {
        let url = format!(
            "https://bfweb.hk.beanfun.com/x.ashx?SN=LPK&WebToken=WEB_TOKEN_X&SecretCode=SECRET&ppppp={PPPPP_LITERAL}&ServiceCode=610074&ServiceRegion=T9&ServiceAccount=SID_1&CreateTime=2024-01-15%2012:34:56&d=12345"
        );
        let redacted = redact_otp_url(&url);

        // Secrets gone, but their presence and length still visible.
        for secret in ["LPK", "WEB_TOKEN_X", "SECRET", "SID_1"] {
            assert!(
                !redacted.contains(secret),
                "{secret} must not survive redaction, got: {redacted}"
            );
        }
        assert!(redacted.contains("WebToken=<11 chars>"), "got: {redacted}");
        assert!(redacted.contains("SN=<3 chars>"), "got: {redacted}");

        // `ppppp` goes the same way. On the handoff route its value
        // comes from the launch blob rather than from this file, and a
        // per-launch value must not be written to a log — while the
        // length still tells the two known shapes apart.
        assert!(
            !redacted.contains(PPPPP_LITERAL),
            "a live ppppp must not survive redaction, got: {redacted}"
        );
        assert!(redacted.contains("ppppp=<64 chars>"), "got: {redacted}");

        // The values a rejection diagnosis actually needs stay verbatim.
        assert!(
            redacted.contains("CreateTime=2024-01-15%2012:34:56"),
            "got: {redacted}"
        );
        assert!(redacted.contains("ServiceCode=610074"), "got: {redacted}");
        assert!(redacted.contains("ServiceRegion=T9"), "got: {redacted}");
        assert!(redacted.contains("d=12345"), "got: {redacted}");
    }

    #[test]
    fn redact_otp_url_passes_through_a_query_less_url() {
        let url = "https://bfweb.hk.beanfun.com/x.ashx";
        assert_eq!(redact_otp_url(url), url);
    }

    // -------------------------------------------------------------------------
    // tick_count_ms
    // -------------------------------------------------------------------------

    #[test]
    fn tick_count_ms_returns_i32_smoke() {
        // We can't pin the value, but two calls within the same
        // microsecond should produce close (or equal) results.
        let a = tick_count_ms();
        let b = tick_count_ms();
        assert!(b.wrapping_sub(a).abs() < 1_000, "got a={a}, b={b}");
    }
}
