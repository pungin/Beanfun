//! Version comparison — 1:1 port of the legacy C# `ApplicationUpdater.IsNewerVersion`
//! (`Beanfun/Update/ApplicationUpdater.cs`).
//!
//! # Algorithm
//!
//! 1. Attempt to parse the local version string with the regex
//!    `(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)`. If matched:
//!    * If the local timestamp equals `remote.timestamp`, return **not newer**.
//!    * Otherwise pack `{major:03}{minor:03}{patch:03}{timestamp}` on both sides
//!      and compare as `i64`. Missing local patch defaults to `0`, matching
//!      C# `string.IsNullOrEmpty(match.Groups[3].Value) ? 0 : …`.
//! 2. If the regex does **not** match, fall back to the legacy behaviour:
//!    strip every non-digit from the local string, left-pad to 19 characters,
//!    parse as `i64`, and compare against the packed remote number.
//! 3. Any parse / overflow error is swallowed and reported as "not newer",
//!    mirroring the C# `try { … } catch { return false; }`.
//!
//! # Why `VersionInfo` fields are `String`
//!
//! WPF captures these fields straight from a GitHub release-tag regex and never
//! parses them as integers at rest. Keeping them as raw lexemes preserves
//! leading zeros (if any) and sidesteps premature overflow checks on the
//! arbitrary-length `timestamp`.

use regex::Regex;
use std::sync::OnceLock;

/// A remote version captured from e.g. a GitHub release tag such as
/// `v5.8.3(2604011114)`.
///
/// Fields are stored verbatim (no numeric parsing) to stay byte-compatible
/// with the C# implementation, which also treats them as raw strings until the
/// comparison step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub major: String,
    pub minor: String,
    pub patch: String,
    pub timestamp: String,
}

impl VersionInfo {
    /// Convenience ctor accepting string slices.
    pub fn new(
        major: impl Into<String>,
        minor: impl Into<String>,
        patch: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            major: major.into(),
            minor: minor.into(),
            patch: patch.into(),
            timestamp: timestamp.into(),
        }
    }
}

/// Return `true` iff `remote` is strictly newer than `local`.
///
/// Any unparseable input (malformed remote fields, numeric overflow, etc.)
/// yields `false`, matching the `try { … } catch { return false; }` pattern in
/// the C# reference.
pub fn is_newer(local: &str, remote: &VersionInfo) -> bool {
    is_newer_checked(local, remote).unwrap_or(false)
}

// -----------------------------------------------------------------------------
// Implementation
// -----------------------------------------------------------------------------

/// Inner fallible variant used by [`is_newer`]. Returning `None` is treated as
/// "not newer" by the public API, emulating the C# catch-all behaviour.
fn is_newer_checked(local: &str, remote: &VersionInfo) -> Option<bool> {
    let remote_num = pack_numeric(
        &remote.major,
        &remote.minor,
        &remote.patch,
        &remote.timestamp,
    )?;

    if let Some(caps) = local_version_regex().captures(local) {
        let local_timestamp = caps.get(4)?.as_str();

        // Same timestamp → authoritative "not newer", even if major/minor differ.
        // This matches the C# short-circuit and also protects against accidental
        // downgrade notifications when two release tags share a build stamp.
        if local_timestamp == remote.timestamp {
            return Some(false);
        }

        let l_major = caps.get(1)?.as_str();
        let l_minor = caps.get(2)?.as_str();
        let l_patch_raw = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        let l_patch = if l_patch_raw.is_empty() {
            "0"
        } else {
            l_patch_raw
        };

        let local_num = pack_numeric(l_major, l_minor, l_patch, local_timestamp)?;
        Some(remote_num > local_num)
    } else {
        // Legacy fallback: strip every non-digit, left-pad to 19, parse as i64.
        // Matches `Regex.Replace(localVer, @"[^\d]", "").PadLeft(19, '0')`.
        let digits: String = local.chars().filter(|c| c.is_ascii_digit()).collect();
        let padded = format!("{:0>19}", digits);
        let local_num: i64 = padded.parse().ok()?;
        Some(remote_num > local_num)
    }
}

/// Format `{major:03}{minor:03}{patch:03}{timestamp}` and parse as `i64`.
///
/// Returns `None` if any field is non-numeric or the concatenated digits
/// overflow `i64`, which is exactly when the C# reference would throw and fall
/// into its `catch { return false; }`.
fn pack_numeric(major: &str, minor: &str, patch: &str, timestamp: &str) -> Option<i64> {
    let m: u32 = major.parse().ok()?;
    let n: u32 = minor.parse().ok()?;
    let p: u32 = patch.parse().ok()?;

    format!("{m:03}{n:03}{p:03}{timestamp}").parse::<i64>().ok()
}

/// Lazily-compiled regex matching `"<maj>.<min>[.<patch>](<timestamp>)"`.
///
/// Identical to the C# pattern `(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)`.
fn local_version_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)").expect("local version regex must compile")
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn v(maj: &str, min: &str, patch: &str, ts: &str) -> VersionInfo {
        VersionInfo::new(maj, min, patch, ts)
    }

    // -------------------------------------------------------------------------
    // Happy-path comparisons
    // -------------------------------------------------------------------------

    // Note: every happy-path test below uses **different** local and remote
    // timestamps on purpose. If they coincide, the C# short-circuit kicks in
    // (see `same_timestamp_short_circuits_even_when_major_differs`) and the
    // packed comparison we want to cover is bypassed.

    #[test]
    fn remote_newer_by_major() {
        assert!(is_newer(
            "5.8.3(2604011114)",
            &v("6", "0", "0", "2604011200")
        ));
    }

    #[test]
    fn remote_newer_by_minor() {
        assert!(is_newer(
            "5.8.3(2604011114)",
            &v("5", "9", "0", "2604011200")
        ));
    }

    /// THE motivating case for the whole packed-number comparison:
    /// naive lexicographic ordering would consider `"5.8.9"` > `"5.8.10"`.
    #[test]
    fn remote_newer_by_patch_9_vs_10() {
        assert!(is_newer(
            "5.8.9(2604011114)",
            &v("5", "8", "10", "2604011200")
        ));
    }

    #[test]
    fn remote_newer_by_timestamp_only() {
        assert!(is_newer(
            "5.8.3(2604011114)",
            &v("5", "8", "3", "2604011115")
        ));
    }

    #[test]
    fn remote_older_returns_false() {
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("5", "8", "2", "2604011113")
        ));
    }

    #[test]
    fn identical_version_returns_false() {
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("5", "8", "3", "2604011114")
        ));
    }

    // -------------------------------------------------------------------------
    // Timestamp-equality short-circuit
    // -------------------------------------------------------------------------

    /// Per C#: identical timestamp ⇒ treated as "not newer" even if major/minor
    /// appear higher. Defensive against release-tag typos.
    #[test]
    fn same_timestamp_short_circuits_even_when_major_differs() {
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("6", "0", "0", "2604011114")
        ));
    }

    // -------------------------------------------------------------------------
    // Legacy "no patch" local format
    // -------------------------------------------------------------------------

    /// Older WPF builds shipped as `5.8(...)` without a patch segment. The C#
    /// regex allows the patch group to be absent; it defaults to 0.
    #[test]
    fn old_local_format_without_patch_defaults_patch_to_zero() {
        // local packed = 005.008.000.{ts}; remote 5.8.1 should be strictly newer.
        assert!(is_newer("5.8(2604011114)", &v("5", "8", "1", "2604011115")));
        // local 5.8 (== 5.8.0) vs remote 5.8.0 same timestamp → not newer.
        assert!(!is_newer(
            "5.8(2604011114)",
            &v("5", "8", "0", "2604011114")
        ));
    }

    // -------------------------------------------------------------------------
    // Regex-fallback branch (local string is not in "maj.min[.patch](ts)" form)
    // -------------------------------------------------------------------------

    /// When the local version is unparseable, the C# code strips non-digits,
    /// left-pads to 19, and compares. Empty / non-numeric local strings give a
    /// local number of 0, so any valid remote version is "newer".
    #[test]
    fn unparseable_local_falls_back_to_digit_strip() {
        assert!(is_newer("bogus", &v("5", "8", "3", "2604011114")));
        assert!(is_newer("", &v("1", "0", "0", "1")));
    }

    /// Fallback is intentionally lossy — concatenating digits drops the
    /// major/minor/patch boundary info. A local string like
    /// `"ver 5.8.3 build 2604011114"` collapses to the 13-digit number
    /// `5832604011114`, well below the remote's 17-digit packed form
    /// `50080032604011114`, so the remote is reported as strictly newer
    /// even though both nominally "describe" `5.8.3`.
    ///
    /// We lock that behaviour in here so nobody silently "fixes" the
    /// fallback to be smarter than the C# reference.
    #[test]
    fn fallback_is_lossy_by_design() {
        assert!(is_newer(
            "ver 5.8.3 build 2604011114",
            &v("5", "8", "3", "2604011114")
        ));
    }

    // -------------------------------------------------------------------------
    // Error / defensive paths (any failure ⇒ `false`)
    // -------------------------------------------------------------------------

    #[test]
    fn non_numeric_remote_fields_return_false() {
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("abc", "8", "3", "2604011114")
        ));
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("5", "8", "3", "notanumber")
        ));
        assert!(!is_newer(
            "5.8.3(2604011114)",
            &v("", "8", "3", "2604011114")
        ));
    }

    #[test]
    fn overflow_on_huge_timestamp_returns_false() {
        // i64::MAX has 19 decimal digits. Packed remote = 9 prefix digits plus a
        // 20-digit timestamp = 29 digits total, which will not fit in i64. The
        // C# reference throws OverflowException here and returns false; we do
        // the same.
        let huge_ts = "1".repeat(20);
        assert!(!is_newer("5.8.3(2604011114)", &v("5", "8", "3", &huge_ts)));
    }

    #[test]
    fn overflow_on_fallback_path_returns_false() {
        // Local string contains enough digits that the padded-to-19 fallback
        // overflows i64. Public API must still return `false`, not panic.
        let digits_only = "9".repeat(25);
        assert!(!is_newer(&digits_only, &v("5", "8", "3", "2604011114")));
    }

    // -------------------------------------------------------------------------
    // Regex variants accepted by the WPF pattern
    // -------------------------------------------------------------------------

    /// The WPF regex `(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)` permits an optional
    /// trailing dot before the timestamp group (`5.8.3.(ts)`). Accepting it
    /// here keeps behaviour identical even though the form is unusual.
    #[test]
    fn trailing_dot_before_timestamp_still_matches() {
        assert!(is_newer(
            "5.8.3.(2604011114)",
            &v("5", "8", "4", "2604011115")
        ));
    }

    // -------------------------------------------------------------------------
    // VersionInfo ergonomics
    // -------------------------------------------------------------------------

    #[test]
    fn version_info_new_accepts_mixed_string_types() {
        let a = VersionInfo::new("5", "8", "3", "2604011114");
        let b = VersionInfo::new(
            String::from("5"),
            String::from("8"),
            String::from("3"),
            String::from("2604011114"),
        );
        assert_eq!(a, b);
    }
}
