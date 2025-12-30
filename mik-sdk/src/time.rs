//! Time utilities for WASI HTTP handlers.
//!
//! Provides convenient functions for getting the current time and formatting timestamps.
//!
//! - **WASI (wasi-http feature):** Uses `wasi:clocks/wall-clock` directly
//! - **Native:** Uses `std::time::SystemTime`
//!
//! # Usage
//!
//! ```ignore
//! use mik_sdk::time;
//!
//! fn my_handler(req: &Request) -> Response {
//!     let timestamp = time::now();       // Unix timestamp (seconds)
//!     let iso = time::now_iso();         // ISO 8601 string
//!
//!     ok!({ "timestamp": int(timestamp as i64), "iso": str(&iso) })
//! }
//! ```
//!
//! # Examples
//!
//! ```ignore
//! use mik_sdk::time;
//!
//! // Unix timestamp (seconds)
//! let secs = time::now();
//!
//! // Milliseconds (for JavaScript interop)
//! let ms = time::now_millis();
//!
//! // ISO 8601 string
//! let iso = time::now_iso();
//! // => "2025-01-16T10:30:00Z"
//! ```

// Native target: use std::time for testing
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds.
///
/// Returns seconds since January 1, 1970 (Unix epoch).
///
/// # Examples
///
/// ```
/// let timestamp = mik_sdk::time::now();
/// assert!(timestamp > 0);
/// ```
// WASM target: use native wasi:clocks/wall-clock
#[inline]
#[must_use]
#[cfg(target_arch = "wasm32")]
pub fn now() -> u64 {
    let datetime = crate::wasi_http::wasi::clocks::wall_clock::now();
    datetime.seconds
}

/// Get current Unix timestamp in seconds (native implementation).
#[inline]
#[must_use]
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Get current Unix timestamp in milliseconds.
///
/// Returns milliseconds since January 1, 1970 (Unix epoch).
/// Useful for JavaScript interop where `Date.now()` returns milliseconds.
///
/// # Examples
///
/// ```
/// let timestamp_ms = mik_sdk::time::now_millis();
/// assert!(timestamp_ms > 0);
/// ```
// WASM target: use native wasi:clocks/wall-clock
#[inline]
#[must_use]
#[cfg(target_arch = "wasm32")]
pub fn now_millis() -> u64 {
    let datetime = crate::wasi_http::wasi::clocks::wall_clock::now();
    to_millis(datetime.seconds, datetime.nanoseconds)
}

/// Get current Unix timestamp in milliseconds (native implementation).
#[inline]
#[must_use]
#[cfg(not(target_arch = "wasm32"))]
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Get current time as ISO 8601 string.
///
/// Returns a UTC timestamp in format: `YYYY-MM-DDTHH:MM:SS.sssZ`
/// Milliseconds are included only if non-zero.
///
/// # Examples
///
/// ```
/// let iso = mik_sdk::time::now_iso();
/// assert!(iso.ends_with('Z'));
/// assert!(iso.contains('T'));
/// ```
// WASM target: use native wasi:clocks/wall-clock
#[inline]
#[must_use]
#[cfg(target_arch = "wasm32")]
pub fn now_iso() -> String {
    let datetime = crate::wasi_http::wasi::clocks::wall_clock::now();
    to_iso(datetime.seconds, datetime.nanoseconds)
}

/// Get current time as ISO 8601 string (native implementation).
#[inline]
#[must_use]
#[cfg(not(target_arch = "wasm32"))]
pub fn now_iso() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    to_iso(duration.as_secs(), duration.subsec_nanos())
}

/// Convert to milliseconds since epoch.
///
/// Uses saturating arithmetic to prevent overflow on extreme values.
/// For timestamps beyond ~584 million years, returns `u64::MAX`.
///
/// # Examples
///
/// ```
/// let ms = mik_sdk::time::to_millis(1737024600, 500_000_000);
/// assert_eq!(ms, 1737024600500);
/// ```
#[inline]
#[must_use]
pub fn to_millis(seconds: u64, nanoseconds: u32) -> u64 {
    seconds
        .saturating_mul(1000)
        .saturating_add(u64::from(nanoseconds / 1_000_000))
}

/// Convert to ISO 8601 string with optional millisecond precision.
///
/// - If nanoseconds is 0: returns `YYYY-MM-DDTHH:MM:SSZ`
/// - If nanoseconds > 0: returns `YYYY-MM-DDTHH:MM:SS.sssZ` (millisecond precision)
///
/// Uses Howard Hinnant's date algorithm for efficient calculation.
///
/// # Examples
///
/// ```
/// // Without sub-second precision
/// let iso = mik_sdk::time::to_iso(1737024600, 0);
/// assert_eq!(iso, "2025-01-16T10:50:00Z");
///
/// // With millisecond precision
/// let iso = mik_sdk::time::to_iso(1737024600, 500_000_000);
/// assert_eq!(iso, "2025-01-16T10:50:00.500Z");
/// ```
#[must_use]
pub fn to_iso(seconds: u64, nanoseconds: u32) -> String {
    use crate::constants::{SECONDS_PER_DAY, SECONDS_PER_HOUR, SECONDS_PER_MINUTE};

    let days = seconds / SECONDS_PER_DAY;
    let remaining = seconds % SECONDS_PER_DAY;

    let hours = remaining / SECONDS_PER_HOUR;
    let remaining = remaining % SECONDS_PER_HOUR;
    let minutes = remaining / SECONDS_PER_MINUTE;
    let secs = remaining % SECONDS_PER_MINUTE;

    // Howard Hinnant's algorithm: https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    if nanoseconds == 0 {
        format!("{year:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{secs:02}Z")
    } else {
        let millis = nanoseconds / 1_000_000;
        format!("{year:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{secs:02}.{millis:03}Z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_millis() {
        assert_eq!(to_millis(1737024600, 500_000_000), 1737024600500);
    }

    #[test]
    fn test_to_millis_truncates() {
        assert_eq!(to_millis(1000, 123_456_789), 1000123);
    }

    #[test]
    fn test_to_iso_epoch() {
        assert_eq!(to_iso(0, 0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_known_date() {
        assert_eq!(to_iso(1737024600, 0), "2025-01-16T10:50:00Z");
    }

    #[test]
    fn test_to_iso_leap_year() {
        assert_eq!(to_iso(1709208000, 0), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn test_to_iso_end_of_year() {
        assert_eq!(to_iso(1735689599, 0), "2024-12-31T23:59:59Z");
    }

    #[test]
    fn test_to_iso_start_of_year() {
        assert_eq!(to_iso(1735689600, 0), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_y2k() {
        assert_eq!(to_iso(946684800, 0), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_far_future() {
        assert_eq!(to_iso(4133980799, 0), "2100-12-31T23:59:59Z");
    }

    #[test]
    fn test_to_iso_month_boundaries() {
        assert_eq!(to_iso(1677628800, 0), "2023-03-01T00:00:00Z");
        assert_eq!(to_iso(1719792000, 0), "2024-07-01T00:00:00Z");
        assert_eq!(to_iso(1730419200, 0), "2024-11-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_day_boundary() {
        assert_eq!(to_iso(1719791999, 0), "2024-06-30T23:59:59Z");
        assert_eq!(to_iso(1719792000, 0), "2024-07-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_century_boundary() {
        assert_eq!(to_iso(946684799, 0), "1999-12-31T23:59:59Z");
        assert_eq!(to_iso(946684800, 0), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn test_to_iso_multi_decade() {
        let cases = [
            (0, "1970-01-01T00:00:00Z"),
            (315532800, "1980-01-01T00:00:00Z"),
            (631152000, "1990-01-01T00:00:00Z"),
            (946684800, "2000-01-01T00:00:00Z"),
            (1262304000, "2010-01-01T00:00:00Z"),
            (1577836800, "2020-01-01T00:00:00Z"),
            (1893456000, "2030-01-01T00:00:00Z"),
        ];
        for (secs, expected) in cases {
            assert_eq!(to_iso(secs, 0), expected);
        }
    }

    #[test]
    fn test_to_iso_with_milliseconds() {
        // 500ms
        assert_eq!(to_iso(1737024600, 500_000_000), "2025-01-16T10:50:00.500Z");
        // 1ms
        assert_eq!(to_iso(1737024600, 1_000_000), "2025-01-16T10:50:00.001Z");
        // 999ms
        assert_eq!(to_iso(1737024600, 999_000_000), "2025-01-16T10:50:00.999Z");
        // 123ms
        assert_eq!(to_iso(1737024600, 123_000_000), "2025-01-16T10:50:00.123Z");
    }

    #[test]
    fn test_to_iso_subsecond_truncation() {
        // Nanoseconds beyond milliseconds are truncated (not rounded)
        assert_eq!(to_iso(1737024600, 123_456_789), "2025-01-16T10:50:00.123Z");
        assert_eq!(to_iso(1737024600, 999_999_999), "2025-01-16T10:50:00.999Z");
    }

    #[test]
    fn test_to_iso_zero_nanoseconds_no_decimal() {
        // Zero nanoseconds should not include decimal point
        assert_eq!(to_iso(1737024600, 0), "2025-01-16T10:50:00Z");
        assert!(!to_iso(1737024600, 0).contains('.'));
    }

    #[test]
    fn test_now_returns_reasonable_value() {
        let timestamp = now();
        // Should be after 2020-01-01 (1577836800)
        assert!(timestamp > 1577836800, "Timestamp should be after 2020");
        // Should be before 2100-01-01 (4102444800)
        assert!(timestamp < 4102444800, "Timestamp should be before 2100");
    }

    #[test]
    fn test_now_millis_returns_reasonable_value() {
        let timestamp_ms = now_millis();
        // Should be after 2020-01-01 in milliseconds
        assert!(
            timestamp_ms > 1577836800000,
            "Timestamp should be after 2020"
        );
        // Should be before 2100-01-01 in milliseconds
        assert!(
            timestamp_ms < 4102444800000,
            "Timestamp should be before 2100"
        );
    }

    #[test]
    fn test_now_millis_is_greater_than_now() {
        let secs = now();
        let ms = now_millis();
        // Milliseconds should be roughly 1000x seconds (within a second of each other)
        assert!(ms >= secs * 1000);
        assert!(ms < (secs + 2) * 1000);
    }

    #[test]
    fn test_now_iso_format() {
        let iso = now_iso();
        // Should end with Z (UTC)
        assert!(iso.ends_with('Z'), "Should end with Z");
        // Should contain T separator
        assert!(iso.contains('T'), "Should contain T separator");
        // Should be 20 or 24 characters (without or with milliseconds)
        assert!(
            iso.len() == 20 || iso.len() == 24,
            "Should be 20 or 24 chars, got {}",
            iso.len()
        );
        // Should match ISO 8601 pattern
        assert!(iso.chars().nth(4) == Some('-'), "Year separator");
        assert!(iso.chars().nth(7) == Some('-'), "Month separator");
        assert!(iso.chars().nth(13) == Some(':'), "Hour separator");
        assert!(iso.chars().nth(16) == Some(':'), "Minute separator");
    }

    #[test]
    fn test_now_consistency() {
        // now_iso should be consistent with now()
        let secs = now();
        let iso = now_iso();
        // Extract year from ISO string
        let year: u64 = iso[0..4].parse().unwrap();
        // Year should be reasonable (2020-2100)
        assert!((2020..=2100).contains(&year));
        // Check that now() and now_iso() are close in time
        let secs2 = now();
        assert!(secs2 - secs <= 1, "now() calls should be within 1 second");
    }

    #[test]
    fn test_now_iso_accuracy() {
        // Verify that now_iso() accurately represents the current time
        // by parsing the ISO string and comparing to now()
        let before = now();
        let iso = now_iso();
        let after = now();

        // Parse ISO string: "2025-01-16T10:50:00Z" or "2025-01-16T10:50:00.500Z"
        let year: u64 = iso[0..4].parse().unwrap();
        let month: u64 = iso[5..7].parse().unwrap();
        let day: u64 = iso[8..10].parse().unwrap();
        let hour: u64 = iso[11..13].parse().unwrap();
        let minute: u64 = iso[14..16].parse().unwrap();
        let second: u64 = iso[17..19].parse().unwrap();

        // Reconstruct timestamp using inverse of Hinnant's algorithm
        // This verifies our to_iso output is accurate
        let y = if month <= 2 { year - 1 } else { year };
        let m = if month <= 2 { month + 12 } else { month };
        let era = y / 400;
        let yoe = y - era * 400;
        let doy = (153 * (m - 3) + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146097 + doe - 719468;
        let reconstructed_secs = days * 86400 + hour * 3600 + minute * 60 + second;

        // The reconstructed timestamp should be within 1 second of the actual time
        assert!(
            reconstructed_secs >= before && reconstructed_secs <= after + 1,
            "ISO '{iso}' -> {reconstructed_secs} should be between {before} and {after}"
        );
    }

    #[test]
    fn test_timestamp_changes_over_time() {
        // Verify that timestamps actually advance
        let t1 = now_millis();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = now_millis();
        assert!(t2 > t1, "Time should advance: {t2} should be > {t1}");
    }
}
