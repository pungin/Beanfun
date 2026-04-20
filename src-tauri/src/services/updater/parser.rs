//! Pure, cross-platform version parsing and comparison — ports the WPF
//! `ApplicationUpdater.cs` `Regex.Match` / `IsNewerVersion` logic
//! (L135-292) to Rust.
//!
//! # Why pack into a single integer?
//!
//! WPF compares versions by concatenating zero-padded components into one
//! `long` and doing a numeric `>` — this naturally handles
//! `5.8.9 < 5.8.10` (which lexicographic string compare gets wrong) and
//! extends trivially to include the build timestamp. We use `u128`
//! instead of `i64` / `u64`:
//!
//! - `long` (i64) in WPF can overflow once the packed digit string grows
//!   past 18 digits — a future major/minor/patch bump + a longer
//!   timestamp would silently wrap. WPF ships without overflow checks so
//!   the bug would surface as "update banner goes away when you really
//!   do need to update".
//! - `u128::from_str` on a 19-digit string fits easily (`u128::MAX ≈
//!   3.4×10³⁸`), so we are safe for any plausible future release cadence
//!   without having to revisit the comparator.
//!
//! # Two local-version paths
//!
//! [`is_newer_version`] mirrors WPF's two-branch behaviour:
//!
//! - **Display form** — matches `(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)`, i.e.
//!   the string produced by the updater itself when it shows "Detect
//!   New Version `5.8.3(2604011114)`" and the user has cached that
//!   into `AssemblyVersion`. If the timestamps are equal we short-circuit
//!   to `false` (WPF L236-239 policy, preserved verbatim even when
//!   major/minor/patch would suggest an upgrade).
//! - **Fallback** — any other shape: strip non-digits, left-pad to 19
//!   chars, parse as `u128`. Covers the common
//!   `Assembly.GetExecutingAssembly().GetName().Version` form
//!   ("5.8.3.2604011114") that the `(timestamp)` regex never matches,
//!   plus truly garbled inputs.
//!
//! # WPF parity
//!
//! | WPF line      | Behaviour                                         | This module                            |
//! | ------------- | ------------------------------------------------- | -------------------------------------- |
//! | L135          | `^v(\d+)\.(\d+)\.(\d+)\.(\d+)$`                   | [`parse_tag`]                          |
//! | L136-137      | `!match.Success → return`                         | [`parse_tag`] → [`super::UpdaterError::UnsupportedTag`] |
//! | L231          | Local regex `(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)`   | [`is_newer_version`] Path A            |
//! | L236-239      | Identical timestamps short-circuit `false`        | [`is_newer_version`] Path A            |
//! | L241-265      | `{major:D3}{minor:D3}{patch:D3}{timestamp}`       | `pack_version` (private helper)        |
//! | L281-282      | Digits-only + `PadLeft(19, '0')`                  | [`is_newer_version`] Path B            |
//! | L287-291      | `catch → return false`                            | [`is_newer_version`] returns `false`    |

use std::sync::OnceLock;

use regex::Regex;

use super::error::UpdaterError;

/// A GitHub release tag successfully parsed as
/// `v<major>.<minor>.<patch>.<timestamp>`.
///
/// Fields carry the exact digit strings from the tag so the packing
/// helper can reproduce WPF's `String.Format("{0:D3}{1:D3}{2:D3}{3}", ...)`
/// byte-for-byte (in particular, the timestamp's exact digit count is
/// preserved — a 10-digit timestamp stays 10-digit, an 11-digit one
/// stays 11-digit, no silent left-pad).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersion {
    /// Major version, e.g. `5` in `v5.8.3.2604011114`.
    pub major: u32,
    /// Minor version, e.g. `8` in `v5.8.3.2604011114`.
    pub minor: u32,
    /// Patch version, e.g. `3` in `v5.8.3.2604011114`.
    pub patch: u32,
    /// Build timestamp as a digit-string (typically 10 digits
    /// `yyMMddHHmm`). Kept as a `String` so the packed form matches
    /// the WPF output exactly — see the [module docs](self) for the
    /// "no silent left-pad" rationale.
    pub timestamp: String,
}

/// Parse a GitHub release tag into [`ParsedVersion`].
///
/// Accepts **only** the exact shape WPF's `Regex.Match` expects
/// (`^v(\d+)\.(\d+)\.(\d+)\.(\d+)$`) — anything else (missing `v`
/// prefix, fewer than four components, alphabetic suffix, whitespace)
/// yields [`UpdaterError::UnsupportedTag`] so the caller can log-and-skip
/// the same way WPF does at L136-137.
pub fn parse_tag(tag: &str) -> Result<ParsedVersion, UpdaterError> {
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = TAG_RE.get_or_init(|| {
        Regex::new(r"^v(\d+)\.(\d+)\.(\d+)\.(\d+)$").expect("static regex compiles")
    });

    let caps = re
        .captures(tag)
        .ok_or_else(|| UpdaterError::UnsupportedTag(tag.to_owned()))?;

    let major = parse_u32(&caps[1], tag)?;
    let minor = parse_u32(&caps[2], tag)?;
    let patch = parse_u32(&caps[3], tag)?;
    let timestamp = caps[4].to_owned();

    Ok(ParsedVersion {
        major,
        minor,
        patch,
        timestamp,
    })
}

/// Compare the locally-running assembly version against a remote
/// [`ParsedVersion`]. Returns `true` iff the remote is strictly newer.
///
/// Mirrors WPF `ApplicationUpdater.IsNewerVersion` (L220-292) including
/// its two input-shape paths and its fail-safe `false` on any parse
/// error (L287-291 `catch → return false`). See the module docs for the
/// full parity table.
pub fn is_newer_version(local: &str, remote: &ParsedVersion) -> bool {
    static DISPLAY_RE: OnceLock<Regex> = OnceLock::new();
    let display_re = DISPLAY_RE.get_or_init(|| {
        Regex::new(r"(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)").expect("static regex compiles")
    });

    // Path A: WPF display form — e.g. "5.8.3(2604011114)"
    if let Some(caps) = display_re.captures(local) {
        let local_timestamp = &caps[4];
        if local_timestamp == remote.timestamp {
            return false;
        }

        let l_major = match caps[1].parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let l_minor = match caps[2].parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let l_patch = match caps.get(3).map(|m| m.as_str()).unwrap_or("") {
            "" => 0,
            s => match s.parse::<u32>() {
                Ok(v) => v,
                Err(_) => return false,
            },
        };

        let Some(remote_num) =
            pack_version(remote.major, remote.minor, remote.patch, &remote.timestamp)
        else {
            return false;
        };
        let Some(local_num) = pack_version(l_major, l_minor, l_patch, local_timestamp) else {
            return false;
        };

        return remote_num > local_num;
    }

    // Path C (new): pure semver without timestamp — e.g. "6.0.0"
    // Our Cargo-based version is X.Y.Z with no build timestamp.
    // Compare (major, minor, patch) tuples directly.
    static SEMVER_RE: OnceLock<Regex> = OnceLock::new();
    let semver_re = SEMVER_RE
        .get_or_init(|| Regex::new(r"^(\d+)\.(\d+)\.(\d+)$").expect("static regex compiles"));

    if let Some(caps) = semver_re.captures(local) {
        let l_major: u32 = match caps[1].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let l_minor: u32 = match caps[2].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let l_patch: u32 = match caps[3].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        return (remote.major, remote.minor, remote.patch) > (l_major, l_minor, l_patch);
    }

    // Path B (WPF fallback): strip non-digits, pad to 19 chars, numeric compare.
    let Some(remote_num) =
        pack_version(remote.major, remote.minor, remote.patch, &remote.timestamp)
    else {
        return false;
    };

    let digits: String = local.chars().filter(|c| c.is_ascii_digit()).collect();
    let padded = left_pad_to(&digits, 19, '0');
    let Ok(local_num) = padded.parse::<u128>() else {
        return false;
    };

    remote_num > local_num
}

/// Pack `major` / `minor` / `patch` (each zero-padded to 3 digits) and
/// `timestamp` (verbatim digit-string) into one `u128`, matching WPF's
/// `String.Format("{0:D3}{1:D3}{2:D3}{3}", ...)` (L241-265).
///
/// Returns `None` if the resulting concatenation fails to parse as
/// `u128` (only possible on implausibly large `major`/`minor`/`patch`
/// or a non-digit `timestamp` — WPF catches this at L287-291 and we
/// propagate the same failure mode).
fn pack_version(major: u32, minor: u32, patch: u32, timestamp: &str) -> Option<u128> {
    let packed = format!("{major:03}{minor:03}{patch:03}{timestamp}");
    packed.parse::<u128>().ok()
}

/// Left-pad `s` with `pad` up to `width` characters. If `s` is already
/// `>= width` chars wide it is returned unchanged (no truncation —
/// matches .NET `String.PadLeft` semantics).
fn left_pad_to(s: &str, width: usize, pad: char) -> String {
    if s.chars().count() >= width {
        return s.to_owned();
    }
    let missing = width - s.chars().count();
    let mut out = String::with_capacity(width);
    for _ in 0..missing {
        out.push(pad);
    }
    out.push_str(s);
    out
}

/// Parse one regex capture as `u32`, converting any
/// `ParseIntError` into an [`UpdaterError::UnsupportedTag`] that
/// includes the original tag (so the caller's log message points at
/// the offending input rather than a generic "invalid digit").
fn parse_u32(raw: &str, original: &str) -> Result<u32, UpdaterError> {
    raw.parse::<u32>()
        .map_err(|_| UpdaterError::UnsupportedTag(original.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_tag_accepts_canonical_release_tag() {
        let v = parse_tag("v5.8.3.2604011114").expect("valid tag");
        assert_eq!(
            v,
            ParsedVersion {
                major: 5,
                minor: 8,
                patch: 3,
                timestamp: "2604011114".to_owned(),
            }
        );
    }

    #[test]
    fn parse_tag_accepts_double_digit_components() {
        let v = parse_tag("v12.34.56.7890123456").expect("valid tag");
        assert_eq!(v.major, 12);
        assert_eq!(v.minor, 34);
        assert_eq!(v.patch, 56);
        assert_eq!(v.timestamp, "7890123456");
    }

    #[test]
    fn parse_tag_rejects_missing_v_prefix() {
        assert!(matches!(
            parse_tag("5.8.3.2604011114"),
            Err(UpdaterError::UnsupportedTag(t)) if t == "5.8.3.2604011114"
        ));
    }

    #[test]
    fn parse_tag_rejects_three_component_tag() {
        assert!(matches!(
            parse_tag("v5.8.3"),
            Err(UpdaterError::UnsupportedTag(_))
        ));
    }

    #[test]
    fn parse_tag_rejects_trailing_garbage() {
        assert!(matches!(
            parse_tag("v5.8.3.2604011114-beta"),
            Err(UpdaterError::UnsupportedTag(_))
        ));
    }

    #[test]
    fn is_newer_version_display_form_detects_upgrade() {
        // local 5.8.3(2604011114) → remote 5.8.4.2604020000
        let remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 4,
            timestamp: "2604020000".to_owned(),
        };
        assert!(is_newer_version("5.8.3(2604011114)", &remote));
    }

    #[test]
    fn is_newer_version_display_form_same_timestamp_returns_false_even_on_patch_bump() {
        // WPF L236-239 quirk: identical timestamps short-circuit to `false`
        // regardless of major/minor/patch delta.
        let remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 99,
            timestamp: "2604011114".to_owned(),
        };
        assert!(!is_newer_version("5.8.3(2604011114)", &remote));
    }

    #[test]
    fn is_newer_version_display_form_missing_patch_treated_as_zero() {
        // Local "5.8(...)" with no patch → l_patch = 0, so remote 5.8.0.later
        // must compare equal-on-components but newer-on-timestamp.
        let remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 0,
            timestamp: "2604020000".to_owned(),
        };
        assert!(is_newer_version("5.8(2604011114)", &remote));
    }

    #[test]
    fn is_newer_version_display_form_patch_bump_is_numeric_not_lexicographic() {
        // Path A stress test. The `(timestamp)` parens keep this on
        // Path A, which is the only path the app actually reaches in
        // production (see `App.xaml.cs::ConvertVersion` L80-102 — the
        // getter always wraps the build timestamp in parens before
        // returning it to the updater).
        //
        // 5.8.9 vs 5.8.10 — lexicographic string compare would say "9"
        // > "10", but the packed comparator zero-pads to 3 digits so
        // 010 > 009. Both directions must agree.
        let remote_newer = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 10,
            timestamp: "2604020000".to_owned(),
        };
        assert!(is_newer_version("5.8.9(2604011114)", &remote_newer));

        let remote_older = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 9,
            timestamp: "2604020000".to_owned(),
        };
        assert!(!is_newer_version("5.8.10(2604011115)", &remote_older));
    }

    #[test]
    fn is_newer_version_semver_local_newer_major_returns_false() {
        let remote = ParsedVersion {
            major: 5,
            minor: 9,
            patch: 1,
            timestamp: "2604180731".to_owned(),
        };
        assert!(!is_newer_version("6.0.0", &remote));
    }

    #[test]
    fn is_newer_version_semver_remote_newer_returns_true() {
        let remote = ParsedVersion {
            major: 7,
            minor: 0,
            patch: 0,
            timestamp: "2700010000".to_owned(),
        };
        assert!(is_newer_version("6.0.0", &remote));
    }

    #[test]
    fn is_newer_version_semver_equal_returns_false() {
        let remote = ParsedVersion {
            major: 6,
            minor: 0,
            patch: 0,
            timestamp: "2604180731".to_owned(),
        };
        assert!(!is_newer_version("6.0.0", &remote));
    }

    #[test]
    fn is_newer_version_fallback_path_b_locks_wpf_lossy_digit_concat() {
        // WPF `IsNewerVersion` Path B (L271-284) strips non-digits from
        // `localVer` and left-pads the result to 19 chars, then compares
        // numerically against the 19-char-packed remote. This is
        // lossy: a local shaped like `"5.8.3.2604011114"` (the raw
        // `Version.ToString()` form) collapses to the 13-digit string
        // `"5832604011114"` — losing the MAJOR/MINOR/PATCH boundary
        // entirely — and a padded 19-digit value that sits three orders
        // of magnitude below any well-formed packed remote.
        //
        // In other words, Path B declares **any** remote "newer" than
        // an assembly-shape local, even an older one. WPF ships with
        // this behaviour and in practice never triggers it because
        // `App.AssemblyVersion` always returns display form via
        // `App.xaml.cs::ConvertVersion` (L80-102) before the string
        // ever reaches the updater.
        //
        // We lock the buggy-but-WPF-faithful behaviour here so any
        // future "clean this up" has to come with an explicit
        // conversation — not a silent drive-by refactor. Two
        // assertions: an older remote and a newer remote, both of
        // which Path B should declare "newer" due to the lossy
        // concat.

        // Older remote (5.8.2) vs local 5.8.3.x — Path B says remote wins.
        let older_remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 2,
            timestamp: "2604011114".to_owned(),
        };
        assert!(
            is_newer_version("5.8.3.2604011114", &older_remote),
            "WPF Path B quirk: older remote reported as newer due to lossy concat"
        );

        // Genuinely newer remote (5.8.4) vs local 5.8.3.x — also wins
        // (coincidentally the "correct" answer, but via the same buggy
        // arithmetic path).
        let newer_remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 4,
            timestamp: "2604011114".to_owned(),
        };
        assert!(is_newer_version("5.8.3.2604011114", &newer_remote));
    }

    #[test]
    fn is_newer_version_garbage_local_falls_through_to_padded_zero() {
        // Non-numeric local → digits = "", padded = "0000000000000000000"
        // → u128 = 0, remote's packed value is > 0 → returns true.
        let remote = ParsedVersion {
            major: 5,
            minor: 8,
            patch: 3,
            timestamp: "2604011114".to_owned(),
        };
        assert!(is_newer_version("definitely-not-a-version", &remote));
    }

    #[test]
    fn pack_version_matches_wpf_zero_padding() {
        assert_eq!(pack_version(5, 8, 3, "2604011114"), Some(50080032604011114));
        assert_eq!(
            pack_version(5, 8, 10, "2604011114"),
            Some(50080102604011114)
        );
        assert_eq!(
            pack_version(12, 34, 56, "7890123456"),
            Some(120340567890123456)
        );
    }

    #[test]
    fn left_pad_to_does_not_truncate_oversized_input() {
        let big = "12345678901234567890";
        assert_eq!(left_pad_to(big, 19, '0'), big);
    }

    #[test]
    fn left_pad_to_pads_short_input() {
        assert_eq!(left_pad_to("123", 6, '0'), "000123");
    }
}
