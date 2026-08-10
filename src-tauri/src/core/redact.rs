//! Keeping secrets out of the log file.
//!
//! Logs used to be a developer-only convenience that vanished into a
//! suppressed console. Now they are written to disk and we actively ask
//! users to send them to us, which usually means pasting them into a
//! public GitHub issue. Anything a log line carries is therefore
//! effectively published, and a session token or an account id in there
//! is not a diagnostic — it is a credential leak with a helpful
//! timestamp attached.
//!
//! The rule this module encodes: log **enough to correlate, never
//! enough to reuse**. A masked value still lets you see that two lines
//! refer to the same token, or that a token changed between two
//! requests — which is what the diagnostics were actually for.

use url::Url;

/// Below this length a value has too little entropy to reveal any of
/// it: showing two characters of a 6-character code gives away a third
/// of it.
const MIN_LEN_TO_SHOW_EDGES: usize = 12;

/// How many leading / trailing characters survive masking.
const EDGE: usize = 2;

/// Mask a secret — a session token, an account id, a cookie value.
///
/// Keeps the first and last couple of characters plus the length, so
/// two log lines can still be compared ("same token as before?",
/// "did it change across the request?") without the value being usable.
/// Short values are replaced wholesale.
///
/// Counts and slices by `char`, so a multi-byte value (an account name
/// in Chinese, say) can never be split mid-character.
pub fn mask(value: &str) -> String {
    let count = value.chars().count();
    if count == 0 {
        return "<empty>".to_string();
    }
    if count < MIN_LEN_TO_SHOW_EDGES {
        return format!("***({count})");
    }
    let head: String = value.chars().take(EDGE).collect();
    let tail: String = value.chars().skip(count - EDGE).collect();
    format!("{head}***{tail}({count})")
}

/// Mask an optional secret, so call sites do not have to branch.
pub fn mask_opt(value: Option<&str>) -> String {
    value.map_or_else(|| "<none>".to_string(), mask)
}

/// Redact a URI for logging.
///
/// - **http / https** — scheme, host and path are kept (they say *which
///   page*, which is the diagnostic), and the query is dropped, because
///   that is where beanfun puts `web_token`, `pSKey` and friends.
/// - **any other scheme** — everything after the scheme is masked. A
///   custom-scheme URI like `ngm://launch/…-passarg:'… sess<hex> …'`
///   carries its session token in the *path*, so there is no safe part
///   to keep beyond the scheme itself.
/// - **unparseable** — masked entirely.
pub fn redact_uri(raw: &str) -> String {
    let Ok(url) = Url::parse(raw) else {
        return format!("[REDACTED]({})", raw.chars().count());
    };

    if !matches!(url.scheme(), "http" | "https") {
        return format!("{}://[REDACTED]({})", url.scheme(), raw.chars().count());
    }

    let mut trimmed = url.clone();
    trimmed.set_query(None);
    trimmed.set_fragment(None);
    let base = trimmed.to_string();
    if url.query().is_some() || url.fragment().is_some() {
        format!("{base}?[REDACTED]")
    } else {
        base
    }
}

/// Scrub a response-body preview before logging it.
///
/// The login flow logs a few hundred characters of the server's reply
/// when a step fails. That preview is the single most useful field we
/// have for diagnosing a login breaking in the wild — it is what tells
/// "beanfun returned a login page" apart from "beanfun returned an
/// error page" apart from "we got HTML where JSON was expected" — so
/// deleting it would gut the reason the log file exists.
///
/// What it must not do is carry the secrets those bodies contain. This
/// removes them while leaving the *shape* of the document intact:
///
/// 1. values of known credential parameters (`web_token=…`, `pSKey=…`,
///    `mltoken=…`, …), including short ones;
/// 2. every HTML `value="…"` attribute — beanfun's login pages carry
///    `pSKey`, `__VIEWSTATE` and friends as hidden inputs, so the
///    attribute is masked wholesale rather than by guessing which
///    `name=` next to it counts as a secret;
/// 3. anything that looks like an email address;
/// 4. any run of 20+ token-alphabet characters, which is what session
///    keys, base64 blobs and GUIDs look like.
///
/// Rules 2 and 4 are deliberately over-eager: masking an innocuous
/// `<option value="TW">` costs a little readability, missing a token
/// costs a credential.
pub fn scrub(preview: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    // Credential-ish parameters, in `k=v`, `k: v` and `"k":"v"` forms.
    static PARAM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)\b(web_?token|p?s{1,2}key|session_?key|mltoken|auth_?token|access_?token|refresh_?token|password|passwd|pwd|otp|secret)\b\s*["']?\s*[=:]\s*["']?([^"'&\s<>,;}\]]+)"#,
        )
        .expect("static regex")
    });
    // Hidden form fields: `<input name="pSKey" value="…">`.
    static VALUE_ATTR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)\bvalue\s*=\s*"([^"]+)""#).expect("static regex"));
    static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("static regex")
    });
    static LONG_RUN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/_-]{20,}={0,2}").expect("static regex"));

    let stage1 = PARAM.replace_all(preview, |caps: &regex::Captures<'_>| {
        format!("{}=[{}]", &caps[1], mask(&caps[2]))
    });
    let stage2 = VALUE_ATTR.replace_all(&stage1, |caps: &regex::Captures<'_>| {
        format!(r#"value="{}""#, mask(&caps[1]))
    });
    let stage3 = EMAIL.replace_all(&stage2, |caps: &regex::Captures<'_>| mask(&caps[0]));
    LONG_RUN
        .replace_all(&stage3, |caps: &regex::Captures<'_>| mask(&caps[0]))
        .into_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_secret_keeps_only_its_edges() {
        let masked = mask("0f1e2d3c4b5a69788796a5b4c3d2e1f0");
        assert_eq!(masked, "0f***f0(32)");
        assert!(!masked.contains("1e2d3c4b5a69788796a5b4c3d2e"));
    }

    #[test]
    fn two_log_lines_can_still_be_compared() {
        // The whole point of keeping the edges: telling "the token
        // changed" apart from "the token is the same" without exposing
        // either of them.
        assert_eq!(mask("AAAA1111222233334444"), mask("AAAA1111222233334444"));
        assert_ne!(mask("AAAA1111222233334444"), mask("BBBB1111222233335555"));
    }

    #[test]
    fn a_short_value_is_replaced_wholesale() {
        // Showing edges of something this short would give most of it away.
        assert_eq!(mask("123456"), "***(6)");
        assert_eq!(mask("a"), "***(1)");
    }

    #[test]
    fn an_empty_value_is_named_rather_than_starred() {
        assert_eq!(mask(""), "<empty>");
        assert_eq!(mask_opt(None), "<none>");
        assert_eq!(mask_opt(Some("")), "<empty>");
    }

    #[test]
    fn a_multibyte_value_is_not_split_mid_character() {
        // Byte slicing would panic or emit replacement characters here.
        let masked = mask("楓之谷經典版帳號名稱測試");
        assert!(masked.starts_with("楓之"));
        assert!(masked.contains("(12)"));
    }

    #[test]
    fn an_account_id_does_not_survive_masking() {
        let masked = mask("player12345@example.com");
        assert!(!masked.contains("player12345"));
        assert!(!masked.contains("example"));
    }

    #[test]
    fn http_urls_keep_the_page_but_lose_the_query() {
        let redacted =
            redact_uri("https://tw.beanfun.com/TW/auth.aspx?channel=member&web_token=SECRET123");
        assert!(!redacted.contains("SECRET123"));
        assert!(redacted.contains("tw.beanfun.com/TW/auth.aspx"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn a_query_less_http_url_is_left_intact() {
        assert_eq!(
            redact_uri("https://tw.beanfun.com/index.aspx"),
            "https://tw.beanfun.com/index.aspx"
        );
    }

    #[test]
    fn a_fragment_is_treated_as_secret_too() {
        // `mltoken=login~<token>` arrives in the fragment.
        let redacted = redact_uri("https://tw.beanfun.com/x#mltoken=login~SECRET");
        assert!(!redacted.contains("SECRET"));
    }

    #[test]
    fn a_custom_scheme_keeps_nothing_but_the_scheme() {
        // The real shape: NGM carries the session token in the path.
        let raw = "ngm://launch/%20-mode%3Alaunch%20-passarg%3A'1234567%20sess0f1e2d3c4b5a69788796a5b4c3d2e1f0%202373'";
        let redacted = redact_uri(raw);
        assert!(!redacted.contains("sess0f1e2d3c4b5a69788796a5b4c3d2e1f0"));
        assert!(!redacted.contains("1234567"));
        assert!(redacted.starts_with("ngm://[REDACTED]"));
    }

    #[test]
    fn an_unparseable_uri_is_masked_entirely() {
        let redacted = redact_uri("not a uri at all ?token=SECRET");
        assert!(!redacted.contains("SECRET"));
        assert!(redacted.starts_with("[REDACTED]"));
    }

    // ── scrub ─────────────────────────────────────────────────────

    #[test]
    fn scrub_keeps_the_shape_of_the_document() {
        // The whole point: you can still tell what kind of page this is.
        let body = r#"<html><head><title>登入</title></head><body class="login">"#;
        assert_eq!(scrub(body), body);
    }

    #[test]
    fn scrub_removes_credential_parameters_even_when_short() {
        let body = r#"<input name="pSKey" value="abc123"/>&web_token=XY9&password=hunter2"#;
        let out = scrub(body);
        assert!(!out.contains("abc123"), "{out}");
        assert!(!out.contains("XY9"), "{out}");
        assert!(!out.contains("hunter2"), "{out}");
        // The parameter names survive — that is the diagnostic.
        assert!(out.contains("pSKey"));
        assert!(out.contains("web_token"));
    }

    #[test]
    fn scrub_masks_hidden_form_values_whatever_they_are_called() {
        // ASP.NET login pages carry the interesting secrets as hidden
        // inputs, and the field name is not always one we predicted.
        let body = r#"<input type="hidden" name="__VIEWSTATE" value="/wEPDwUKMTQ3OTU2" /><input name="ctl00$unknown" value="s3cret" />"#;
        let out = scrub(body);
        assert!(!out.contains("/wEPDwUKMTQ3OTU2"), "{out}");
        assert!(!out.contains("s3cret"), "{out}");
        assert!(out.contains("__VIEWSTATE"), "{out}");
    }

    #[test]
    fn scrub_removes_emails_and_long_token_runs() {
        let body = "user player12345@example.com sess0f1e2d3c4b5a69788796a5b4c3d2e1f0 end";
        let out = scrub(body);
        assert!(!out.contains("player12345"), "{out}");
        assert!(!out.contains("example"), "{out}");
        assert!(!out.contains("0f1e2d3c4b5a69788796a5b4c3d2e1f0"), "{out}");
        assert!(out.starts_with("user "), "{out}");
        assert!(out.ends_with(" end"), "{out}");
    }

    #[test]
    fn scrub_leaves_ordinary_words_and_short_numbers_alone() {
        let body = "service_code 610074 region T9 accounts 1 status ok";
        assert_eq!(scrub(body), body);
    }

    #[test]
    fn scrub_masks_a_json_shaped_credential() {
        let out = scrub(r#"{"MLToken":"03AFcW.short","Result":0}"#);
        assert!(!out.contains("03AFcW.short"), "{out}");
        assert!(out.contains("Result"), "{out}");
    }

    // ── source-tree guard ─────────────────────────────────────────

    /// Fail the build when a `tracing!` field that carries user data is
    /// bound to something unredacted.
    ///
    /// This exists because the first pass at redaction was done by
    /// grepping for likely names and it **missed a live account id** in
    /// `login::completed` — the sweep looked at the sites a human
    /// thought of, and the log only proved it during a run that
    /// happened not to log in. A mechanical check does not get tired
    /// and does not depend on which code path a test run exercised.
    ///
    /// Escape hatch: put `redact-ok` in a comment on the line or the
    /// line above, which forces the exception to be written down and
    /// reviewed rather than merely assumed.
    #[test]
    fn no_tracing_field_leaks_user_data() {
        use std::path::Path;

        /// Field names whose value is user data, not a diagnostic.
        const SENSITIVE: &[&str] = &[
            "account",
            "account_id",
            "accounts_raw",
            "auth_token",
            "body_preview",
            "cookie_value",
            "final_url",
            "href",
            "jar_web_token",
            "mltoken",
            "otp",
            "password",
            "preview",
            "pwd",
            "raw",
            "session_key",
            "session_web_token",
            "skey",
            "uri",
            "url",
            "used_web_token",
            "value",
            "web_token",
        ];
        const SAFE_MARKERS: &[&str] = &["redact::", "redact_u", "mask(", "mask_opt(", "scrub("];

        /// Byte ranges covered by `tracing::…!( … )` invocations.
        ///
        /// Needed because the `name = expr,` shape also describes
        /// `format!` named arguments — an early version of this test
        /// flagged a URL builder in `otp.rs` that logs nothing.
        /// Parenthesis counting skips string literals so a message like
        /// `"portal running (visible=true)"` cannot unbalance it.
        fn tracing_spans(text: &str) -> Vec<(usize, usize)> {
            let bytes = text.as_bytes();
            let mut spans = Vec::new();
            let mut search = 0;
            while let Some(hit) = text[search..].find("tracing::") {
                let start = search + hit;
                search = start + "tracing::".len();
                let Some(bang) = text[start..].find("!(") else {
                    break;
                };
                // `!(` must be on the same statement, not paragraphs away.
                if bang > 40 {
                    continue;
                }
                let open = start + bang + 1;
                let (mut depth, mut i, mut in_str) = (0usize, open, false);
                while i < bytes.len() {
                    let c = bytes[i];
                    if in_str {
                        if c == b'\\' {
                            i += 2;
                            continue;
                        }
                        if c == b'"' {
                            in_str = false;
                        }
                    } else if c == b'"' {
                        in_str = true;
                    } else if c == b'(' {
                        depth += 1;
                    } else if c == b')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    i += 1;
                }
                spans.push((open, i.min(bytes.len())));
                search = i;
            }
            spans
        }

        fn visit(dir: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read source");
                let spans = tracing_spans(&text);
                let lines: Vec<&str> = text.lines().collect();
                let mut offset = 0usize;
                for (i, line) in lines.iter().enumerate() {
                    let line_start = offset;
                    offset += line.len() + 1;
                    if !spans
                        .iter()
                        .any(|(a, b)| line_start > *a && line_start < *b)
                    {
                        continue;
                    }
                    let trimmed = line.trim();
                    // A tracing field binding looks like `name = expr,`.
                    let Some((name, expr)) = trimmed.split_once('=') else {
                        continue;
                    };
                    let name = name.trim();
                    if !trimmed.ends_with(',') || name.starts_with("let ") || name.contains(' ') {
                        continue;
                    }
                    if !SENSITIVE.contains(&name) {
                        continue;
                    }
                    let expr = expr.trim().trim_end_matches(',').trim();
                    // A literal carries no user data by construction.
                    if expr.starts_with('"') {
                        continue;
                    }
                    if SAFE_MARKERS.iter().any(|m| expr.contains(m)) {
                        continue;
                    }
                    let prev = i.checked_sub(1).map(|p| lines[p]).unwrap_or("");
                    if line.contains("redact-ok") || prev.contains("redact-ok") {
                        continue;
                    }
                    out.push(format!(
                        "{}:{}  {name} = {expr}",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                        i + 1
                    ));
                }
            }
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        visit(&src, &mut offenders);
        offenders.sort();

        assert!(
            offenders.is_empty(),
            "these log fields would put user data in a file we ask users to send us.\n\
             Wrap the value in core::redact (mask / mask_opt / redact_uri / scrub),\n\
             or add a `redact-ok: <reason>` comment if it genuinely carries none:\n  {}",
            offenders.join("\n  ")
        );
    }
}
