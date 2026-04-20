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

use std::sync::OnceLock;

use chrono::Local;
use percent_encoding::percent_decode_str;
use regex::Regex;

use crate::core::parser::{capture_first, extract_service_account_create_time};
use crate::core::time::{dt_compact_now, dt_iso_now};
use crate::core::wcdes::decrypt_hex;
use crate::services::beanfun::account::ServiceAccount;
use crate::services::beanfun::client::{BeanfunClient, LoginRegion};
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
    let secret_code = step_2_get_secret_code(client).await?;
    step_3_record_start(client, account, &step1, service_code, service_region).await?;
    step_4_long_poll(client, &step1.long_polling_key).await?;
    let envelope = step_5_get_otp(
        client,
        session,
        account,
        &step1,
        &secret_code,
        service_code,
        service_region,
    )
    .await?;
    step_6_decrypt(&envelope)
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
    let url = client.portal_url("beanfun_block/game_zone/game_start_step2.aspx")?;
    let resp = client
        .http()
        .get(url)
        .query(&[
            ("service_code", service_code),
            ("service_region", service_region),
            ("sotp", account.ssn.as_str()),
            ("dt", dt_compact_now().as_str()),
        ])
        .send()
        .await?;
    ensure_success(&resp, "game_start_step2.aspx")?;
    let body = client.bounded_text(resp).await?;

    let long_polling_key = parse_long_polling_key(&body)?;
    let unk_data = match client.config().region {
        LoginRegion::TW => Some(parse_unk_data(&body)?),
        LoginRegion::HK => None,
    };
    let screatetime = match account.screatetime.as_deref() {
        Some(s) => s.to_string(),
        None => parse_screatetime_fallback(&body)?,
    };

    Ok(Step1Data {
        long_polling_key,
        unk_data,
        screatetime,
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

    let resp = client.http().post(url).form(&form).send().await?;
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

/// Step 5 — read the `1;{key}{ciphertext_hex}` envelope from
/// `get_webstart_otp.ashx`.
///
/// The URL is built **as a string** rather than via reqwest's `.query()`
/// builder because two of the parameters require WPF-specific encoding
/// that the form-urlencoder would emit differently:
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
async fn step_5_get_otp(
    client: &BeanfunClient,
    session: &Session,
    account: &ServiceAccount,
    step1: &Step1Data,
    secret_code: &str,
    service_code: &str,
    service_region: &str,
) -> Result<String, LoginError> {
    let url = build_get_webstart_otp_url(
        client,
        session,
        account,
        step1,
        secret_code,
        service_code,
        service_region,
        tick_count_ms(),
    )?;
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
fn parse_long_polling_key(html: &str) -> Result<String, LoginError> {
    capture_first(long_polling_key_regex(), html).ok_or_else(|| {
        LoginError::OtpMissingLongPollingKey {
            snippet: snippet_for_diagnostics(html),
        }
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
fn parse_screatetime_fallback(html: &str) -> Result<String, LoginError> {
    extract_service_account_create_time(html).ok_or(LoginError::OtpMissingCreateTime)
}

/// Extract the `m_strSecretCode` JS literal from step 2's response.
fn parse_secret_code(html: &str) -> Result<String, LoginError> {
    capture_first(secret_code_regex(), html).ok_or(LoginError::OtpMissingSecretCode)
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
) -> Result<String, LoginError> {
    let base = client.portal_url("beanfun_block/generic_handlers/get_webstart_otp.ashx")?;
    // WPF replaces only spaces with `%20`; every other char in the
    // screatetime format (`yyyy-MM-dd HH:mm:ss`) is already URL-safe.
    let create_time_encoded = step1.screatetime.replace(' ', "%20");
    Ok(format!(
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
    ))
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

    #[test]
    fn step5_url_replaces_screatetime_spaces_with_percent20() {
        // Build a tiny client + session and verify the URL string
        // contains `%20` (not `+`) where screatetime had a space.
        use crate::services::beanfun::client::ClientConfig;
        let client = BeanfunClient::new(ClientConfig::for_region(LoginRegion::TW)).unwrap();
        let session = Session::new(
            LoginRegion::TW,
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
            long_polling_key: "LPK".to_string(),
            unk_data: None,
            screatetime: "2024-01-15 12:34:56".to_string(),
        };
        let url = build_get_webstart_otp_url(
            &client, &session, &account, &step1, "SECRET", "610074", "T9", 12345,
        )
        .unwrap();

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
