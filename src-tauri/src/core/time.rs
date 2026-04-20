//! Cache-buster timestamp formatters for Beanfun's classic ASP.NET URLs.
//!
//! Several Beanfun endpoints (`game_zone/*.aspx`, `get_result.ashx`, …)
//! take an opaque `dt=…` or `_=…` query parameter purely to defeat
//! intermediate HTTP caches. The legacy WPF `BeanfunClient` produces
//! these strings via `BeanfunClient.cs::GetCurrentTime(int method)`
//! (see WPF L175-191), and we mirror the **exact byte sequence** that
//! method emits so caches behave identically — different formatting
//! could in theory cause spurious cache hits or 404s on edge nodes
//! that key on the raw string.
//!
//! # Two formats in use
//!
//! | This module | WPF reference          | Wire shape                              | Used by                                                         |
//! |-------------|------------------------|-----------------------------------------|------------------------------------------------------------------|
//! | [`dt_compact`] | `GetCurrentTime(2)` | `Y(M-1)DDhhmmssfff` (concatenated)      | `?dt=…` on `game_zone/*.aspx` (account list, OTP step 1)        |
//! | [`dt_iso`]     | `GetCurrentTime(0)` | `yyyyMMddHHmmss.fff`                    | `?_=…` on `get_result.ashx` (OTP long-poll)                     |
//!
//! Both formats use **local time** (matching WPF `DateTime.Now`).
//! The `dt` parameter is a cache buster; the server does not validate
//! the value, so the timezone is functionally irrelevant — but
//! mirroring `DateTime.Now` keeps any hypothetical future server-side
//! sanity check (e.g. "rejects timestamps from the future") aligned
//! with WPF's.
//!
//! # Quirk: zero-indexed month in [`dt_compact`]
//!
//! `GetCurrentTime(2)` in WPF computes `(date.Month - 1).ToString()`
//! and concatenates it without zero-padding, mimicking JavaScript's
//! `Date.getMonth()` convention (which is 0-11). For January it emits
//! `"0"` (single digit), and for October it emits `"9"`. We reproduce
//! that exactly — including the absence of zero-padding — so the
//! emitted string is byte-for-byte identical to WPF for every minute
//! of every day.

use chrono::{DateTime, Datelike, Local, Timelike};

/// Format `now` as `Y(M-1)DDhhmmssfff` — the WPF `GetCurrentTime(2)`
/// shape used as the `?dt=…` cache buster on `game_zone/*.aspx` URLs.
///
/// Notable quirks (all 1:1 with WPF):
/// - Year is the full 4-digit year.
/// - Month is **0-indexed and not zero-padded** (Jan → `"0"`, Oct →
///   `"9"`, Dec → `"11"`), mirroring JavaScript `Date.getMonth()`.
/// - Day, hour, minute, second are all 2-digit zero-padded.
/// - Milliseconds are 3-digit zero-padded.
///
/// Examples:
/// - 2024-01-05 03:09:07.042 → `"2005030907042"` (Y=`2024`, M-1=`0`, …)
/// - 2024-12-31 23:59:59.999 → `"20241131235959999"` (M-1=`11`)
pub fn dt_compact(now: DateTime<Local>) -> String {
    let year = now.year();
    let month_zero_indexed = now.month0();
    let day = now.day();
    let hour = now.hour();
    let minute = now.minute();
    let second = now.second();
    let millis = now.nanosecond() / 1_000_000;

    format!("{year}{month_zero_indexed}{day:02}{hour:02}{minute:02}{second:02}{millis:03}")
}

/// Format `now` as `yyyyMMddHHmmss.fff` — the WPF `GetCurrentTime(0)`
/// shape used as the `?_=…` cache buster on `get_result.ashx` (OTP
/// long-poll).
///
/// Sortable, ISO-ish with millisecond suffix; no separators between
/// date components. Mirrors C# `date.ToString("yyyyMMddHHmmss.fff")`
/// byte-for-byte.
pub fn dt_iso(now: DateTime<Local>) -> String {
    now.format("%Y%m%d%H%M%S%.3f").to_string()
}

/// Convenience wrapper: [`dt_compact`] with `now = Local::now()`.
///
/// Production callers use this; tests should call [`dt_compact`]
/// directly with a pinned `DateTime` so the assertion is reproducible.
pub fn dt_compact_now() -> String {
    dt_compact(Local::now())
}

/// Convenience wrapper: [`dt_iso`] with `now = Local::now()`.
///
/// Production callers use this; tests should call [`dt_iso`] directly
/// with a pinned `DateTime` so the assertion is reproducible.
pub fn dt_iso_now() -> String {
    dt_iso(Local::now())
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn pin(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        min: u32,
        sec: u32,
        ms: u32,
    ) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .unwrap()
            + chrono::Duration::milliseconds(ms as i64)
    }

    // -------------------------------------------------------------------------
    // dt_compact (= WPF GetCurrentTime(2))
    // -------------------------------------------------------------------------

    /// January 5th at 03:09:07.042 — exercises the zero-indexed month
    /// edge case (Jan → `"0"`, single digit, no zero-padding) and the
    /// zero-padding of every other component.
    ///
    /// Decomposition of the expected string:
    /// `2024` + `0` (M-1) + `05` (DD) + `03` (HH) + `09` (mm) +
    /// `07` (ss) + `042` (fff) = `"2024005030907042"` (16 chars).
    #[test]
    fn dt_compact_january_emits_single_digit_zero_for_month() {
        let now = pin(2024, 1, 5, 3, 9, 7, 42);
        assert_eq!(dt_compact(now), "2024005030907042".to_string());
    }

    /// December 31st at 23:59:59.999 — exercises the upper bound of
    /// every component, in particular `month0() == 11` becoming the
    /// 2-digit string `"11"` (not zero-padded to `"011"`).
    ///
    /// `2024` + `11` (M-1) + `31` + `23` + `59` + `59` + `999` =
    /// `"20241131235959999"` (17 chars — one more than January because
    /// the month component is 2 digits instead of 1).
    #[test]
    fn dt_compact_december_emits_two_digit_eleven_for_month() {
        let now = pin(2024, 12, 31, 23, 59, 59, 999);
        assert_eq!(dt_compact(now), "20241131235959999".to_string());
    }

    /// October — `month0() == 9` → emits `"9"` (single digit),
    /// confirming there is no zero-padding for months 1-10.
    ///
    /// `2024` + `9` (M-1) + `01` + `00` + `00` + `00` + `000` =
    /// `"2024901000000000"` (16 chars).
    #[test]
    fn dt_compact_october_emits_single_digit_nine_for_month() {
        let now = pin(2024, 10, 1, 0, 0, 0, 0);
        assert_eq!(dt_compact(now), "2024901000000000".to_string());
    }

    /// Sanity: midnight on the first of February — every padded field
    /// at its lowest value, confirming the `02`/`03` width specifiers
    /// emit leading zeroes correctly.
    ///
    /// `2024` + `1` (M-1, single digit) + `01` + `00` + `00` + `00` +
    /// `000` = `"2024101000000000"` (16 chars).
    #[test]
    fn dt_compact_lowest_values_padded() {
        let now = pin(2024, 2, 1, 0, 0, 0, 0);
        assert_eq!(dt_compact(now), "2024101000000000".to_string());
    }

    // -------------------------------------------------------------------------
    // dt_iso (= WPF GetCurrentTime(0))
    // -------------------------------------------------------------------------

    #[test]
    fn dt_iso_emits_yyyy_mm_dd_hh_mm_ss_dot_fff() {
        let now = pin(2024, 1, 5, 3, 9, 7, 42);
        assert_eq!(dt_iso(now), "20240105030907.042".to_string());
    }

    #[test]
    fn dt_iso_zero_pads_every_component() {
        let now = pin(2024, 12, 31, 23, 59, 59, 999);
        assert_eq!(dt_iso(now), "20241231235959.999".to_string());
    }

    #[test]
    fn dt_iso_emits_three_digit_millis() {
        let now = pin(2024, 6, 15, 12, 0, 0, 5);
        // .005 (not .5 or .050)
        assert!(dt_iso(now).ends_with(".005"), "got: {}", dt_iso(now));
    }

    // -------------------------------------------------------------------------
    // _now wrappers — smoke only (cannot pin Local::now())
    // -------------------------------------------------------------------------

    #[test]
    fn dt_compact_now_returns_non_empty_digits_only() {
        let s = dt_compact_now();
        assert!(!s.is_empty());
        assert!(
            s.chars().all(|c| c.is_ascii_digit()),
            "dt_compact must be all digits, got: {s}"
        );
    }

    #[test]
    fn dt_iso_now_contains_dot_for_subsecond() {
        let s = dt_iso_now();
        assert!(
            s.contains('.'),
            "dt_iso must include the .fff separator, got: {s}"
        );
    }
}
