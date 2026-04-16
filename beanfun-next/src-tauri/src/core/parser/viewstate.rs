//! Extract ASP.NET hidden form fields (`__VIEWSTATE`, `__VIEWSTATEGENERATOR`,
//! `__EVENTVALIDATION`) from an HTML response body.
//!
//! # WPF reference
//!
//! - `Beanfun/Tools/BeanfunClient.Account.cs` (used in 10+ places): the
//!   "strict" regex `id="__VIEWSTATE" value="(.*)" />`.
//! - `Beanfun/MainWindow.xaml.cs` around the verify-code flow: the "loose"
//!   regex `id="__VIEWSTATE"[^>]+value="([^"]+)"`.
//!
//! # Why the loose regex is safer
//!
//! Any HTML that the strict regex matches is also matched by the loose regex,
//! but not vice versa. ASP.NET is known to occasionally reorder attributes
//! (e.g. when the page contains additional server controls), so the loose
//! regex is strictly more resilient. We also capture the `value="…"` group
//! with `[^"]+` (non-greedy-ish) to avoid the catastrophic over-match
//! behaviour of `(.*)` when a single line contains multiple hidden fields.
//!
//! # Optionality matrix (from the WPF code paths)
//!
//! | Field                     | Required in | Notes                          |
//! |---------------------------|-------------|--------------------------------|
//! | `__VIEWSTATE`             | every page  | absence ⇒ `ParserError::MissingViewState` |
//! | `__VIEWSTATEGENERATOR`    | most pages  | absent on initial-GET verify page in WPF (stored only if present) |
//! | `__EVENTVALIDATION`       | POST pages  | absent on initial GET |
//!
//! To cover every WPF call site with a single function, we make the two
//! trailing fields `Option<String>` and let the caller assert presence.

use regex::Regex;
use std::sync::OnceLock;

use super::{ParserError, Result};

/// Extracted hidden-field payload from an ASP.NET WebForms HTML response.
///
/// See module docs for which fields are required vs optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewStateForm {
    /// `__VIEWSTATE` — always present on a well-formed ASP.NET WebForms page.
    pub viewstate: String,
    /// `__VIEWSTATEGENERATOR` — usually present; verify-code GET response in
    /// WPF treats it as optional (`bfClient.verifyViewStateGenerator` is only
    /// stored when the regex matches), so we do too.
    pub viewstate_generator: Option<String>,
    /// `__EVENTVALIDATION` — absent on the very first GET of a page; present
    /// after any POST round-trip.
    pub event_validation: Option<String>,
}

/// Extract the three ASP.NET hidden fields from `html`.
///
/// Returns [`ParserError::MissingViewState`] only if `__VIEWSTATE` itself is
/// absent. The other two fields are returned as `None` when missing, matching
/// the WPF behaviour in `MainWindow.xaml.cs` where `__VIEWSTATEGENERATOR` is
/// optional and `__EVENTVALIDATION` is only required on POST-targeted pages.
pub fn extract_viewstate(html: &str) -> Result<ViewStateForm> {
    // Each field gets its own function-local `OnceLock`: compiled once on
    // first call, reused for the lifetime of the process. Keeping the
    // `static` items adjacent to their sole use-site avoids an intermediate
    // dispatch helper that would otherwise need a catch-all `panic!` branch.
    static VIEWSTATE_RE: OnceLock<Regex> = OnceLock::new();
    static GENERATOR_RE: OnceLock<Regex> = OnceLock::new();
    static EVENT_VALIDATION_RE: OnceLock<Regex> = OnceLock::new();

    let vs_re = VIEWSTATE_RE.get_or_init(|| compile_field("__VIEWSTATE"));
    let gen_re = GENERATOR_RE.get_or_init(|| compile_field("__VIEWSTATEGENERATOR"));
    let ev_re = EVENT_VALIDATION_RE.get_or_init(|| compile_field("__EVENTVALIDATION"));

    let viewstate = capture_first(vs_re, html).ok_or(ParserError::MissingViewState)?;
    let viewstate_generator = capture_first(gen_re, html);
    let event_validation = capture_first(ev_re, html);

    Ok(ViewStateForm {
        viewstate,
        viewstate_generator,
        event_validation,
    })
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Return the first capture group (`value="…"`) of the first regex match, or
/// `None` when the pattern fails to find a match in `html`.
fn capture_first(re: &Regex, html: &str) -> Option<String> {
    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
}

/// Compile the WPF loose pattern `id="<field_name>"[^>]+value="([^"]+)"` for
/// an arbitrary hidden-field name. `regex::escape` guards against any
/// regex-special characters in `field_name` (none of the three canonical
/// ASP.NET names contain any, but defending here keeps the helper reusable).
fn compile_field(field_name: &str) -> Regex {
    let pattern = format!(r#"id="{}"[^>]+value="([^"]+)""#, regex::escape(field_name));
    Regex::new(&pattern).expect("viewstate regex must compile")
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal full page with all three hidden fields — the most common shape
    /// served by the beanfun login flow.
    const FULL_PAGE: &str = r#"
<html>
<body>
<form method="post" action="./login.aspx" id="form1">
<input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="VS_TOKEN_ABC+/=" />
<input type="hidden" name="__VIEWSTATEGENERATOR" id="__VIEWSTATEGENERATOR" value="C2EE9ABB" />
<input type="hidden" name="__EVENTVALIDATION" id="__EVENTVALIDATION" value="EV_TOKEN_XYZ==" />
</form>
</body>
</html>
"#;

    #[test]
    fn extracts_all_three_fields() {
        let form = extract_viewstate(FULL_PAGE).expect("should parse");
        assert_eq!(form.viewstate, "VS_TOKEN_ABC+/=");
        assert_eq!(form.viewstate_generator.as_deref(), Some("C2EE9ABB"));
        assert_eq!(form.event_validation.as_deref(), Some("EV_TOKEN_XYZ=="));
    }

    #[test]
    fn initial_get_page_without_event_validation() {
        // GET of the login page — WPF Account code path L189/L196 only requires
        // viewstate + viewstate_generator here, event_validation shows up on
        // the subsequent POST response.
        let html = r#"<input id="__VIEWSTATE" value="VS1" />
<input id="__VIEWSTATEGENERATOR" value="GEN1" />"#;
        let form = extract_viewstate(html).unwrap();
        assert_eq!(form.viewstate, "VS1");
        assert_eq!(form.viewstate_generator.as_deref(), Some("GEN1"));
        assert_eq!(form.event_validation, None);
    }

    #[test]
    fn verify_page_without_viewstategenerator() {
        // Verify flow — `MainWindow.xaml.cs` stores `verifyViewStateGenerator`
        // only when the regex matches (`if (regex.IsMatch(response))`), so the
        // absence of the generator field must be tolerated.
        let html = r#"<input id="__VIEWSTATE" value="VS_VERIFY" />
<input id="__EVENTVALIDATION" value="EV_VERIFY" />"#;
        let form = extract_viewstate(html).unwrap();
        assert_eq!(form.viewstate, "VS_VERIFY");
        assert_eq!(form.viewstate_generator, None);
        assert_eq!(form.event_validation.as_deref(), Some("EV_VERIFY"));
    }

    #[test]
    fn attribute_order_swapped_still_matches() {
        // The WPF **strict** regex `id="__VIEWSTATE" value="(.*)" />` would
        // fail here because `type=` appears between `id` and `value`. The
        // loose regex — which we use — still matches, matching the intent of
        // `MainWindow.xaml.cs`.
        let html = r#"<input id="__VIEWSTATE" type="hidden" name="__VIEWSTATE" value="SWAPPED" />"#;
        let form = extract_viewstate(html).unwrap();
        assert_eq!(form.viewstate, "SWAPPED");
    }

    #[test]
    fn missing_viewstate_returns_error() {
        let html = r#"<input id="__VIEWSTATEGENERATOR" value="GEN_ONLY" />"#;
        assert_eq!(extract_viewstate(html), Err(ParserError::MissingViewState));
    }

    #[test]
    fn viewstate_value_with_base64_padding_is_captured_whole() {
        // Real-world `__VIEWSTATE` values are base64 and often end in `=`
        // signs. The capture group `[^"]+` must keep them all.
        let html = r#"<input id="__VIEWSTATE" value="wEPDwULLTE5ODQ2MzM3OTYPFgIeFl9fQ==" />"#;
        let form = extract_viewstate(html).unwrap();
        assert_eq!(form.viewstate, "wEPDwULLTE5ODQ2MzM3OTYPFgIeFl9fQ==");
    }

    #[test]
    fn multiple_hidden_fields_on_one_line_does_not_over_match() {
        // The legacy strict regex uses greedy `.*` and would gobble the first
        // three hidden fields into the __VIEWSTATE value on a single-line
        // response. Our loose `[^"]+` stops at the first closing quote, which
        // is the safer behaviour.
        let html = r#"<input id="__VIEWSTATE" value="VS_ONLY" /><input id="__VIEWSTATEGENERATOR" value="GEN_ONLY" />"#;
        let form = extract_viewstate(html).unwrap();
        assert_eq!(form.viewstate, "VS_ONLY");
        assert_eq!(form.viewstate_generator.as_deref(), Some("GEN_ONLY"));
    }
}
