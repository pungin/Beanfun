//! Extract the `akey=…` value from a beanfun redirect URL.
//!
//! # WPF reference
//!
//! `Beanfun/Tools/BeanfunClient.Login.cs` at L261, L365, and L688 all use the
//! same pattern:
//!
//! ```csharp
//! Regex regex = new Regex("akey=(.*)");
//! ```
//!
//! Applied against either:
//!
//! - `this.ResponseUri.ToString()` — a `UrlDecoded` redirect URL, where
//!   `akey=` is always the last query parameter in the real server responses.
//! - `(string)json["StrReslut"]` — a JSON string that also happens to end
//!   with `akey=…`.
//!
//! Because the C# regex is **greedy** with `(.*)` and the caller feeds it a
//! single line, any residual query parameters after `akey=` get swallowed
//! into the capture. We intentionally preserve that behaviour (see user rule
//! "不要修改沒有叫你修改的部分" for the migration spec) so that the
//! downstream `services/beanfun` code — which already strips any trailing
//! noise — stays byte-compatible with WPF.

use regex::Regex;
use std::sync::OnceLock;

use super::{ParserError, Result};

/// Extract the value following the first `akey=` occurrence in `input`.
///
/// Greedy by design: anything from `akey=` up to the end of the **line** is
/// returned as the value, matching the WPF regex. Callers whose input may
/// contain trailing parameters should split on `&` themselves.
pub fn extract_akey(input: &str) -> Result<String> {
    akey_regex()
        .captures(input)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_owned())
        .ok_or(ParserError::MissingAkey)
}

fn akey_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // `(.*)` — greedy to end-of-line, 1:1 port of the WPF pattern.
    RE.get_or_init(|| Regex::new(r"akey=(.*)").expect("akey regex must compile"))
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_akey_from_redirect_url_trailing() {
        let url = "https://tw.newlogin.beanfun.com/login/id-pass_done.aspx?akey=ABCDEF1234567890";
        assert_eq!(extract_akey(url).unwrap(), "ABCDEF1234567890");
    }

    /// Greedy-by-design behaviour: trailing query params get swallowed. We
    /// lock this in because the WPF downstream code assumes it and strips
    /// any `&foo=bar` segment on its own.
    #[test]
    fn greedy_captures_trailing_query_params() {
        let url = "https://host/path?akey=XYZ&ts=123&sig=abc";
        assert_eq!(extract_akey(url).unwrap(), "XYZ&ts=123&sig=abc");
    }

    /// A cookie value or JSON fragment containing `akey=…` in the middle
    /// still matches, again for compatibility with WPF's
    /// `(string)json["StrReslut"]` path.
    #[test]
    fn matches_when_akey_appears_mid_string() {
        let fragment = "some prefix text akey=TOKEN_IN_THE_MIDDLE";
        assert_eq!(extract_akey(fragment).unwrap(), "TOKEN_IN_THE_MIDDLE");
    }

    #[test]
    fn empty_value_is_captured_as_empty_string() {
        // `akey=` with nothing after it on the same line is considered a
        // successful (but empty) capture — same as the WPF regex.
        let url = "https://host/done.aspx?akey=";
        assert_eq!(extract_akey(url).unwrap(), "");
    }

    #[test]
    fn missing_akey_returns_error() {
        assert_eq!(
            extract_akey("https://host/no-key-here.aspx"),
            Err(ParserError::MissingAkey)
        );
        assert_eq!(extract_akey(""), Err(ParserError::MissingAkey));
    }

    /// `.` in the default regex flavour does **not** match newlines, so a
    /// multi-line input containing `akey=…` followed by another line only
    /// captures up to the first `\n`. Preserving this guarantees we never
    /// accidentally inhale another header or body segment into the akey.
    #[test]
    fn newline_terminates_the_greedy_match() {
        let input = "Location: https://host/?akey=LINE_ONE\nContent-Type: text/html";
        assert_eq!(extract_akey(input).unwrap(), "LINE_ONE");
    }

    /// Only the first occurrence wins — WPF uses `regex.Match(...)` (first
    /// match), not `Matches(...)`.
    #[test]
    fn first_occurrence_wins() {
        let input = "akey=FIRST and later akey=SECOND";
        // Greedy regex actually captures up to end-of-line, so "FIRST and later akey=SECOND"
        // is returned as a single capture. This is the documented WPF behaviour and
        // serves as a regression guard against anyone "fixing" it to be non-greedy.
        assert_eq!(extract_akey(input).unwrap(), "FIRST and later akey=SECOND");
    }
}
