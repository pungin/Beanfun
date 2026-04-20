//! Extract the ASP.NET MVC / Razor anti-forgery token
//! (`__RequestVerificationToken`) from an HTML response.
//!
//! # WPF reference
//!
//! Two call sites, both with the **same** loose regex:
//!
//! - `Beanfun/Tools/BeanfunClient.Login.cs` L45-48
//!
//!   ```csharp
//!   string formToken = Regex
//!       .Match(indexHtml, "__RequestVerificationToken[^>]+value=\"([^\"]+)\"")
//!       .Groups[1]
//!       .Value;
//!   ```
//!
//! - Same file L416-421 (verify captcha flow).
//!
//! The pattern matches both `<input name="__RequestVerificationToken" …>`
//! and `<input id="__RequestVerificationToken" …>`, which is important
//! because MVC emits the former while hand-rolled forms sometimes use the
//! latter.
//!
//! # Why not just reuse the viewstate extractor?
//!
//! The viewstate extractor anchors on `id="<name>"`, which would miss the
//! common MVC form where only `name="__RequestVerificationToken"` is set. The
//! token field therefore gets its own tailored regex that mirrors the WPF
//! call site exactly.

use regex::Regex;
use std::sync::OnceLock;

use super::{capture_first, ParserError, Result};

/// Extract the `__RequestVerificationToken` value from the first matching
/// `<input>` tag in `html`.
///
/// Returns [`ParserError::MissingRequestVerificationToken`] when no such tag
/// exists. In WPF, the Login flow tolerates a missing token for one code path
/// (by defaulting to an empty string) but errors out in another; surfacing a
/// typed error lets each call site decide.
pub fn extract_verification_token(html: &str) -> Result<String> {
    capture_first(token_regex(), html).ok_or(ParserError::MissingRequestVerificationToken)
}

fn token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Identical to the WPF loose regex
    // `__RequestVerificationToken[^>]+value="([^"]+)"`.
    RE.get_or_init(|| {
        Regex::new(r#"__RequestVerificationToken[^>]+value="([^"]+)""#)
            .expect("token regex must compile")
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_token_from_mvc_style_name_input() {
        // Razor `@Html.AntiForgeryToken()` shape: `name="…"` with no `id`.
        let html =
            r#"<input name="__RequestVerificationToken" type="hidden" value="MVC_TOKEN_abc+/=" />"#;
        assert_eq!(
            extract_verification_token(html).unwrap(),
            "MVC_TOKEN_abc+/="
        );
    }

    #[test]
    fn extracts_token_from_id_attribute_style() {
        // Hand-rolled form using `id="…"`.
        let html = r#"<input id="__RequestVerificationToken" value="ID_STYLE_TOKEN" />"#;
        assert_eq!(extract_verification_token(html).unwrap(), "ID_STYLE_TOKEN");
    }

    #[test]
    fn extracts_token_with_extra_attributes_in_between() {
        // `[^>]+` tolerates arbitrary attributes between the field name and
        // `value="…"`, which is exactly how the real login page renders it.
        let html = r#"<input name="__RequestVerificationToken" type="hidden" class="anti-forgery" autocomplete="off" value="LONG_TOKEN==" />"#;
        assert_eq!(extract_verification_token(html).unwrap(), "LONG_TOKEN==");
    }

    #[test]
    fn missing_token_returns_error() {
        let html = r#"<form><input type="text" name="Account" /></form>"#;
        assert_eq!(
            extract_verification_token(html),
            Err(ParserError::MissingRequestVerificationToken)
        );
    }

    #[test]
    fn first_token_wins_when_multiple_forms_present() {
        // The WPF code uses `Regex.Match(...)` (first match), not Matches(...),
        // so the earliest anti-forgery token wins when a page renders multiple
        // forms (e.g. login + register).
        let html = r#"
<form id="login"><input name="__RequestVerificationToken" value="LOGIN_TOK" /></form>
<form id="register"><input name="__RequestVerificationToken" value="REGISTER_TOK" /></form>
"#;
        assert_eq!(extract_verification_token(html).unwrap(), "LOGIN_TOK");
    }

    /// The value capture group is `[^"]+`, so an empty `value=""` does NOT
    /// match and the token is reported missing — matching the WPF guard that
    /// treats `Groups[1].Value == ""` as "no token".
    #[test]
    fn empty_value_is_treated_as_missing() {
        let html = r#"<input name="__RequestVerificationToken" value="" />"#;
        assert_eq!(
            extract_verification_token(html),
            Err(ParserError::MissingRequestVerificationToken)
        );
    }
}
