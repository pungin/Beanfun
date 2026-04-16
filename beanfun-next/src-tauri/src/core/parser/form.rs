//! Extract **hidden form fields** from an ASP.NET WebForms / Razor HTML
//! page — essentially a port of the `<input[^>]+>` scrape that WPF uses
//! to re-submit the `SendLogin` stash to `return.aspx`.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Login.cs::TwRegularLogin` L117-144 and the
//! identical block in `QRCodeLogin` L553-580:
//!
//! ```csharp
//! foreach (Match tag in Regex.Matches(sendLoginHtml, @"<input[^>]+>", ...))
//! {
//!     string tagStr = tag.Value;
//!     Match nameMatch = Regex.Match(tagStr, @"name\s*=\s*['""]([^'""]+)['""]", ...);
//!     Match valMatch  = Regex.Match(tagStr, @"value\s*=\s*['""]([^'""]*)['""]", ...);
//!     if (nameMatch.Success && valMatch.Success
//!         && tagStr.IndexOf("type=\"submit\"", OrdinalIgnoreCase) == -1)
//!         payload.Add(nameMatch.Groups[1].Value, valMatch.Groups[1].Value);
//! }
//! ```
//!
//! # What counts as a "hidden" field for our purposes
//!
//! Everything in the page that is NOT an obvious user-visible submit
//! button. The WPF code filters exclusively on `type="submit"` (lowercased
//! exact match with surrounding double-quotes) — any other input type
//! (hidden, text that the server pre-filled, radio with a `value` attr, …)
//! is forwarded. We preserve that exact heuristic so our output is byte
//! identical to what the WPF flow would post.
//!
//! # Duplicates
//!
//! `Vec<(String, String)>` (rather than `HashMap`) keeps insertion order
//! and tolerates duplicate names, matching WPF's `NameValueCollection`.
//! The final `x-www-form-urlencoded` body is equivalent either way, but
//! keeping order helps when diffing HTTP captures against the WPF client.

use std::sync::OnceLock;

use regex::Regex;

/// One `(name, value)` pair scraped from the HTML, in document order.
pub type HiddenInput = (String, String);

/// Scrape every non-submit `<input>` tag in `html` that carries both a
/// `name="…"` and `value="…"` attribute, returning the `(name, value)`
/// pairs in document order.
///
/// Returns an empty `Vec` when the page has no matchable inputs — the
/// caller decides whether that's an error (e.g. `SendLoginNoFormData` in
/// the login flow).
pub fn extract_hidden_inputs(html: &str) -> Vec<HiddenInput> {
    input_tag_regex()
        .find_iter(html)
        .filter_map(|tag| scrape_input_pair(tag.as_str()))
        .collect()
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Pull a `(name, value)` pair from the contents of a single `<input …>`
/// tag. Returns `None` when either attribute is missing or the tag is a
/// submit button (per the WPF exclusion).
fn scrape_input_pair(tag: &str) -> Option<HiddenInput> {
    if is_submit_button(tag) {
        return None;
    }

    let name = capture_attr(name_attr_regex(), tag)?;
    let value = capture_attr(value_attr_regex(), tag)?;
    Some((name, value))
}

/// Match the WPF substring check
/// `tag.IndexOf("type=\"submit\"", OrdinalIgnoreCase) == -1`.
///
/// Deliberately case-insensitive on the `type` key only; we match
/// `submit` case-sensitively because the WPF probe used the literal
/// string `type="submit"`. Real ASP.NET output always renders the value
/// lowercase anyway.
fn is_submit_button(tag: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?i)type\s*=\s*["']submit["']"#).expect("submit detector regex must compile")
    });
    re.is_match(tag)
}

/// Return the first single or double-quoted capture group content, owned.
fn capture_attr(re: &Regex, tag: &str) -> Option<String> {
    re.captures(tag)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
}

fn input_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // `(?is)` = case-insensitive + dot matches newline, mirroring the
    // `RegexOptions.IgnoreCase | RegexOptions.Singleline` passed to WPF's
    // `Regex.Matches`. `[^>]+` keeps the scan within one tag.
    RE.get_or_init(|| Regex::new(r"(?is)<input[^>]+>").expect("input tag regex must compile"))
}

fn name_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)name\s*=\s*['"]([^'"]+)['"]"#).expect("name attribute regex must compile")
    })
}

fn value_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Note: the value group uses `*` (not `+`) so an empty `value=""` is
    // preserved — WPF's regex does the same, and empty values are common
    // for `__EVENTTARGET` / `__EVENTARGUMENT` hidden fields.
    RE.get_or_init(|| {
        Regex::new(r#"(?i)value\s*=\s*['"]([^'"]*)['"]"#)
            .expect("value attribute regex must compile")
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Modelled on the real `SendLogin` HTML: a couple of ASP.NET hidden
    /// fields plus a submit button we must skip.
    const SEND_LOGIN_FIXTURE: &str = r#"
<html><body>
  <form action="https://tw.beanfun.com/beanfun_block/bflogin/return.aspx" method="post">
    <input type="hidden" name="SessionKey" value="SKEY_ABC123" />
    <input type="hidden" name="AuthKey" value="AUTH_ABC456" />
    <input type="hidden" name="ServiceCode" value="" />
    <input type="hidden" name="ServiceRegion" value="" />
    <input type="hidden" name="ServiceAccountSN" value="0" />
    <input type="submit" name="btn_submit" value="Submit" />
  </form>
</body></html>
"#;

    #[test]
    fn scrapes_every_hidden_field_preserves_order_skips_submit() {
        let pairs = extract_hidden_inputs(SEND_LOGIN_FIXTURE);
        assert_eq!(
            pairs,
            vec![
                ("SessionKey".to_string(), "SKEY_ABC123".to_string()),
                ("AuthKey".to_string(), "AUTH_ABC456".to_string()),
                ("ServiceCode".to_string(), "".to_string()),
                ("ServiceRegion".to_string(), "".to_string()),
                ("ServiceAccountSN".to_string(), "0".to_string()),
            ],
            "submit button must be dropped; order must be document order; \
             empty values preserved"
        );
    }

    #[test]
    fn empty_html_yields_empty_vec() {
        assert!(extract_hidden_inputs("").is_empty());
        assert!(extract_hidden_inputs("<html><body>nothing here</body></html>").is_empty());
    }

    #[test]
    fn single_quoted_attributes_are_accepted() {
        // WPF regex accepts both `'` and `"` quote styles; we mirror that.
        let html = r#"<input type='hidden' name='X' value='Y' />"#;
        assert_eq!(extract_hidden_inputs(html), vec![("X".into(), "Y".into())]);
    }

    #[test]
    fn mixed_case_type_submit_is_still_filtered() {
        // The WPF probe is `OrdinalIgnoreCase`, so a stray `Type="Submit"`
        // must also be dropped.
        let html = r#"<input Type="SUBMIT" name="shouldnotappear" value="X" />"#;
        assert!(extract_hidden_inputs(html).is_empty());
    }

    #[test]
    fn input_without_name_is_dropped() {
        // No `name` attribute ⇒ no way to build a POST field ⇒ skip.
        let html = r#"<input type="hidden" value="orphan" />"#;
        assert!(extract_hidden_inputs(html).is_empty());
    }

    #[test]
    fn input_without_value_is_dropped() {
        // Mirrors the WPF `valMatch.Success` guard. This is distinct from
        // `value=""` which is scraped as empty-string; here the attribute
        // is entirely absent.
        let html = r#"<input type="hidden" name="lonely" />"#;
        assert!(extract_hidden_inputs(html).is_empty());
    }

    #[test]
    fn multiple_inputs_on_one_line_are_each_picked_up() {
        // ASP.NET can render inputs without newlines between them; the
        // `[^>]+` terminator guarantees we don't merge two tags.
        let html = r#"<input type="hidden" name="A" value="1" /><input type="hidden" name="B" value="2" />"#;
        assert_eq!(
            extract_hidden_inputs(html),
            vec![("A".into(), "1".into()), ("B".into(), "2".into()),]
        );
    }

    #[test]
    fn extra_attributes_between_name_and_value_are_tolerated() {
        // ASP.NET occasionally inserts data-* / id / autocomplete between
        // the attributes we care about.
        let html = r#"<input type="hidden" id="ctl00_X" autocomplete="off" name="X" class="q" value="1" />"#;
        assert_eq!(extract_hidden_inputs(html), vec![("X".into(), "1".into())]);
    }
}
