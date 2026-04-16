//! Parse the HTML body of `game_server_account_list.aspx` into a list of
//! `ServiceAccount` rows plus the optional "amount limit" notice.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Account.cs` (L87-125 in
//! [`GetAccounts`](https://github.com/…)):
//!
//! ```csharp
//! regex = new Regex(
//!     "onclick=\"([^\"]*)\"><div id=\"(\\w+)\" sn=\"(\\d+)\" name=\"([^\"]+)\""
//! );
//! ```
//!
//! The capture groups are:
//!
//! | # | Field       | Notes                                                 |
//! |---|-------------|-------------------------------------------------------|
//! | 1 | `onclick`   | empty string ⇒ account is disabled                   |
//! | 2 | `id`        | service account id (alphanumeric)                    |
//! | 3 | `sn`        | serial number (digits only)                          |
//! | 4 | `name`      | display name; must be **HTML-decoded** downstream    |
//!
//! Additionally, the page may contain a server-side notice when the account
//! quota is reached:
//!
//! ```csharp
//! regex = new Regex(
//!     "<div id=\"divServiceAccountAmountLimitNotice\" class=\"InnerContent\">(.*)</div>"
//! );
//! ```
//!
//! We surface the two as separate functions so callers can skip the notice
//! lookup when they only care about the list.

use regex::Regex;
use std::sync::OnceLock;

/// One row from the service-account listing table.
///
/// Field names mirror the legacy C# `ServiceAccount` class verbatim (`sid` /
/// `ssn` / `sname`) so grep-replace from the old code base lands cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAccountRow {
    /// `true` when the row's anchor has a non-empty `onclick` handler.
    /// WPF test: `match.Groups[1].Value != ""`.
    pub is_enable: bool,
    /// ASP.NET-generated `id` attribute of the inner `<div>` — used as the
    /// service-account identifier everywhere downstream.
    pub sid: String,
    /// Numeric serial number (`sn="…"`).
    pub ssn: String,
    /// Display name (`name="…"`) with HTML entities decoded
    /// (`WebUtility.HtmlDecode` in WPF).
    pub sname: String,
}

/// Extract every service-account row from a `game_server_account_list.aspx`
/// body.
///
/// Returns an empty `Vec` when the list has no rows (the page may still
/// render the amount-limit notice in that case). Consumer code preserves the
/// document order; any stable sort should be layered on top.
///
/// Rows where the inner capture groups are empty are skipped to match the
/// `if (...Value == "") continue;` guard in WPF.
pub fn extract_service_accounts(html: &str) -> Vec<ServiceAccountRow> {
    account_row_regex()
        .captures_iter(html)
        .filter_map(|caps| {
            let onclick = caps.get(1)?.as_str();
            let sid = caps.get(2)?.as_str();
            let ssn = caps.get(3)?.as_str();
            let sname_raw = caps.get(4)?.as_str();

            // WPF: `if (Groups[2].Value == "" || Groups[3].Value == "" || Groups[4].Value == "") continue;`
            if sid.is_empty() || ssn.is_empty() || sname_raw.is_empty() {
                return None;
            }

            Some(ServiceAccountRow {
                is_enable: !onclick.is_empty(),
                sid: sid.to_owned(),
                ssn: ssn.to_owned(),
                sname: html_escape::decode_html_entities(sname_raw).into_owned(),
            })
        })
        .collect()
}

/// Extract the "account amount limit" notice (red banner shown when the user
/// can no longer create new service accounts), if present.
///
/// Returns the raw HTML / text inside the notice `<div>` — callers are
/// responsible for any further trimming or localisation. WPF decides between
/// "auth re-login" and a translated form of the server text outside this
/// parser; we stay pure here.
pub fn extract_account_limit_notice(html: &str) -> Option<String> {
    amount_limit_notice_regex()
        .captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn account_row_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"onclick="([^"]*)"><div id="(\w+)" sn="(\d+)" name="([^"]+)""#)
            .expect("account row regex must compile")
    })
}

fn amount_limit_notice_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"<div id="divServiceAccountAmountLimitNotice" class="InnerContent">(.*)</div>"#,
        )
        .expect("amount limit notice regex must compile")
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Full page snippet modelled after the actual
    /// `game_server_account_list.aspx` markup (attribute order and sibling
    /// positioning preserved).
    const MULTI_ROW_HTML: &str = r##"
<div class="list">
  <a href="#" onclick="doLogin('abc123')"><div id="abc123" sn="12345" name="RegularAccount"></div></a>
  <a href="#"><div id="def456" sn="67890" name="DisabledAccount"></div></a>
  <a href="#" onclick="doLogin('ghi789')"><div id="ghi789" sn="11111" name="Fran&#231;ois"></div></a>
</div>
"##;

    #[test]
    fn extracts_multiple_rows_with_enabled_flag() {
        let rows = extract_service_accounts(MULTI_ROW_HTML);
        assert_eq!(
            rows.len(),
            2,
            "the disabled row has no onclick and therefore does not match the WPF regex"
        );
        assert_eq!(
            rows[0],
            ServiceAccountRow {
                is_enable: true,
                sid: "abc123".into(),
                ssn: "12345".into(),
                sname: "RegularAccount".into(),
            }
        );
    }

    /// WPF regex strictly requires an `onclick="…">` immediately before the
    /// inner `<div>`, so a row whose anchor omits `onclick` will simply not
    /// match. We reproduce that behaviour verbatim: the row is silently
    /// dropped from the list. A dedicated fixture is kept for clarity.
    #[test]
    fn row_without_onclick_is_silently_dropped() {
        let html = r##"<a href="#"><div id="aaa" sn="1" name="NoOnclick"></div></a>"##;
        assert!(extract_service_accounts(html).is_empty());
    }

    #[test]
    fn empty_onclick_value_marks_row_disabled() {
        // `onclick=""` matches the regex but produces `is_enable = false`,
        // matching WPF's `match.Groups[1].Value != ""` check.
        let html = r#"<a onclick=""><div id="disabled1" sn="42" name="NoHandler"></div></a>"#;
        let rows = extract_service_accounts(html);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].is_enable);
        assert_eq!(rows[0].sid, "disabled1");
    }

    #[test]
    fn html_entities_in_name_are_decoded() {
        // `Fran&#231;ois` → `François`; `&amp;` → `&`; `&lt;` → `<`.
        let rows = extract_service_accounts(MULTI_ROW_HTML);
        let francois = rows
            .iter()
            .find(|r| r.sid == "ghi789")
            .expect("should find ghi789");
        assert_eq!(francois.sname, "François");

        let html = r#"<a onclick="x"><div id="q1" sn="1" name="Tom &amp; Jerry"></div></a>"#;
        let rows = extract_service_accounts(html);
        assert_eq!(rows[0].sname, "Tom & Jerry");
    }

    /// Empty sid/ssn/sname fields must be filtered out — mirrors the
    /// `continue;` guard in WPF. The regex requires at least one `\w` /
    /// `\d` / non-`"` char so the only realistic way to hit this today is
    /// a hand-crafted HTML, but we still lock the behaviour in.
    #[test]
    fn rows_with_internally_empty_fields_are_skipped() {
        // This doesn't match the regex at all because `\d+` and `\w+`
        // require at least one character — which is exactly the behaviour
        // the WPF guard encodes. We therefore assert **no rows** are
        // returned.
        let html = r#"<a onclick="x"><div id="" sn="" name=""></div></a>"#;
        assert!(extract_service_accounts(html).is_empty());
    }

    #[test]
    fn notice_when_present() {
        let html = r#"<div id="divServiceAccountAmountLimitNotice" class="InnerContent">You have reached the 5-account limit.</div>"#;
        assert_eq!(
            extract_account_limit_notice(html).as_deref(),
            Some("You have reached the 5-account limit.")
        );
    }

    #[test]
    fn notice_absent_returns_none() {
        assert_eq!(
            extract_account_limit_notice("<div id='other'>nope</div>"),
            None
        );
    }

    #[test]
    fn empty_document_yields_empty_list() {
        assert!(extract_service_accounts("").is_empty());
        assert_eq!(extract_account_limit_notice(""), None);
    }
}
