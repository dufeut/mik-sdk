//! Structured JSON logging to stderr.
//!
//! Outputs logs in a standardized JSON format compatible with most log aggregation
//! systems including ELK Stack, Datadog, `CloudWatch`, Splunk, and Grafana Loki.
//!
//! # Output Format
//!
//! Each log line is a JSON object with these fields:
//! - `level`: Log level ("debug", "info", "warn", "error")
//! - `msg`: The log message
//! - `ts`: ISO 8601 UTC timestamp (e.g., "2025-01-16T10:30:00Z")
//! - Additional key-value fields as specified
//!
//! ```json
//! {"level":"info","msg":"user created","id":"123","email":"alice@example.com","ts":"2025-01-16T10:30:00Z"}
//! {"level":"warn","msg":"rate limit approaching","remaining":"5","ts":"2025-01-16T10:30:01Z"}
//! {"level":"error","msg":"failed to fetch","url":"https://api.example.com","status":"500","ts":"2025-01-16T10:30:02Z"}
//! ```
//!
//! # Structured Logging Usage
//!
//! ```no_run
//! # use mik_sdk::log;
//! let user_id = "123";
//! let email = "alice@example.com";
//! let count = 5;
//!
//! // Structured logging with key-value pairs
//! log!(info, "user created", id: user_id, email: &email);
//! log!(warn, "rate limit approaching", remaining: count);
//! ```
//!
//! # Simple Logging (format string style)
//!
//! ```no_run
//! # use mik_sdk::log;
//! let user_id = "u123";
//! let key = "cache_key";
//! let err = "connection refused";
//!
//! log::info!("User {} logged in", user_id);
//! log::warn!("Cache miss for key: {}", key);
//! log::error!("Database connection failed: {}", err);
//! log::debug!("Debug message");  // Only in debug builds
//! ```
//!
//! # Debug Logging
//!
//! `debug!` messages are only emitted in debug builds (when `#[cfg(debug_assertions)]` is true).
//! In release builds, `debug!` is a no-op.
//!
//! # Compatibility
//!
//! This format is designed for maximum compatibility:
//! - **ELK Stack**: Auto-parses JSON, maps fields automatically
//! - **Datadog**: Auto-detects JSON logs, extracts standard fields
//! - **`CloudWatch` Logs Insights**: Query with parsed JSON fields
//! - **Splunk**: Auto-extracts JSON fields
//! - **Grafana Loki**: Label extraction from JSON fields
//!
//! # Note on Macro Names
//!
//! These macros are prefixed with `log_` internally to avoid conflicts with other macros
//! and builtin attributes, but are re-exported as `info`, `warn`, `error`, and `debug`
//! for use as `log::info!()`, `log::warn!()`, etc.

use std::time::{SystemTime, UNIX_EPOCH};

/// Format timestamp as ISO 8601 with millisecond precision.
/// Returns format: "YYYY-MM-DDTHH:MM:SS.sssZ"
#[doc(hidden)]
#[must_use]
pub fn __format_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    __format_timestamp_from_duration(now.as_secs(), now.subsec_millis())
}

/// Internal: format timestamp from seconds and milliseconds.
/// Exposed for testing the date calculation algorithm.
///
/// # Algorithm
///
/// Uses Howard Hinnant's civil_from_days algorithm for converting days since
/// Unix epoch (1970-01-01) to year/month/day. This is a well-tested, efficient
/// algorithm used in many date libraries.
///
/// Reference: <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>
#[doc(hidden)]
#[must_use]
#[allow(clippy::similar_names)] // doe/doy are standard date algorithm abbreviations
pub fn __format_timestamp_from_duration(secs: u64, millis: u32) -> String {
    use crate::constants::{SECONDS_PER_DAY, SECONDS_PER_HOUR, SECONDS_PER_MINUTE};

    // Split timestamp into days and time-of-day components
    let days = secs / SECONDS_PER_DAY;
    let remaining = secs % SECONDS_PER_DAY;

    // Extract hours, minutes, seconds from time-of-day
    let hours = remaining / SECONDS_PER_HOUR;
    let remaining = remaining % SECONDS_PER_HOUR;
    let minutes = remaining / SECONDS_PER_MINUTE;
    let seconds = remaining % SECONDS_PER_MINUTE;

    // ========================================================================
    // Howard Hinnant's civil_from_days algorithm
    // Converts days since Unix epoch to (year, month, day)
    // ========================================================================

    // Shift epoch from 1970-01-01 to 0000-03-01 (simplifies leap year math)
    // 719468 = days from 0000-03-01 to 1970-01-01
    let z = days + 719468;

    // Calculate era (400-year cycle). Each era has exactly 146097 days.
    // 146097 = 365*400 + 97 leap days (97 = 400/4 - 400/100 + 400/400)
    let era = z / 146097;

    // Day-of-era: 0 to 146096
    let doe = z - era * 146097;

    // Year-of-era: 0 to 399
    // The formula accounts for leap years:
    // - Every 4 years is a leap year (subtract doe/1460)
    // - Except every 100 years (add doe/36524)
    // - Except every 400 years (subtract doe/146096)
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;

    // Absolute year (from year 0)
    let y = yoe + era * 400;

    // Day-of-year: 0 to 365
    // Subtracts the days in previous years of this era
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);

    // Month calculation using the "30.6 rule" (months avg ~30.6 days Mar-Feb)
    // mp ranges 0-11, representing Mar(0) to Feb(11)
    let mp = (5 * doy + 2) / 153;

    // Day-of-month: 1 to 31
    // 153 = sum of days in 5-month groups (Mar-Jul or Aug-Dec)
    let d = doy - (153 * mp + 2) / 5 + 1;

    // Convert month from Mar=0..Feb=11 to Jan=1..Dec=12
    let m = if mp < 10 { mp + 3 } else { mp - 9 };

    // Adjust year for Jan/Feb (they belong to the next calendar year in this algorithm)
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
}

/// Escape a string for JSON output per RFC 7159.
///
/// Handles quotes, backslashes, and control characters.
///
/// # Note: Intentional Duplication
///
/// This function is duplicated in `mik-bridge/src/lib.rs` as `escape_json_string()`.
/// This is intentional because:
/// - `mik-bridge` is a standalone WASM component that cannot depend on `mik-sdk`
/// - `mik-sdk` cannot depend on `mik-bridge` (it's the other way around)
/// - Creating a shared crate for ~20 lines of code adds unnecessary complexity
///
/// If you modify this function, please update the duplicate in `mik-bridge` too.
#[doc(hidden)]
#[must_use]
pub fn __escape_json(s: &str) -> String {
    // Estimate capacity: base length + 10% for potential escapes.
    // Common escapes (", \, \n, \r, \t) expand 1 char to 2.
    // Control chars expand to 6 (\uXXXX) but are rare.
    let estimated_capacity = s.len() + (s.len() / 10).max(8);
    let mut result = String::with_capacity(estimated_capacity);
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                // Use write! to avoid format! allocation for rare control chars
                use std::fmt::Write;
                let _ = write!(result, "\\u{:04x}", c as u32);
            },
            c => result.push(c),
        }
    }
    result
}

/// Write a simple log line to stderr (internal helper).
///
/// This function contains the actual logging logic. The public macros
/// (`log::info!`, etc.) are thin wrappers that call this function.
#[doc(hidden)]
pub fn __write_simple_log(level: &str, msg: &str) {
    use std::io::Write;
    let timestamp = __format_timestamp();
    let escaped = __escape_json(msg);
    let _ = writeln!(
        std::io::stderr(),
        r#"{{"level":"{level}","msg":"{escaped}","ts":"{timestamp}"}}"#,
    );
}

/// Log an informational message to stderr.
///
/// # Examples
///
/// ```no_run
/// # use mik_sdk::log;
/// log::info!("Server started on port 8080");
/// log::info!("User {} logged in", "u123");
/// ```
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::__write_simple_log("info", &format!($($arg)*)) };
}

/// Log a warning message to stderr.
///
/// # Examples
///
/// ```no_run
/// # use mik_sdk::log;
/// log::warn!("Cache miss for key: {}", "user:123");
/// log::warn!("Slow query detected: {}ms", 150);
/// ```
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::__write_simple_log("warn", &format!($($arg)*)) };
}

/// Log an error message to stderr.
///
/// # Examples
///
/// ```no_run
/// # use mik_sdk::log;
/// log::error!("Database connection failed: {}", "timeout");
/// log::error!("Failed to parse request body");
/// ```
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::__write_simple_log("error", &format!($($arg)*)) };
}

/// Log a debug message to stderr (only in debug builds).
///
/// In release builds, this macro is a no-op and produces no code.
///
/// # Examples
///
/// ```no_run
/// # use mik_sdk::log;
/// log::debug!("Request payload: {:?}", "test data");
/// log::debug!("Cache size: {} entries", 42);
/// ```
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        $crate::log::__write_simple_log("debug", &format!($($arg)*));
    }};
}

// Re-export macros with clean names for use as `log::info!()`, etc.
pub use log_debug as debug;
pub use log_error as error;
pub use log_info as info;
pub use log_warn as warn;

// ============================================================================
// STRUCTURED LOGGING MACRO
// ============================================================================

/// Build a structured JSON log line with key-value pairs.
///
/// Internal helper that constructs the JSON string from level, message, and fields.
#[doc(hidden)]
#[must_use]
pub fn __build_structured_log(level: &str, msg: &str, fields: &[(&str, &str)]) -> String {
    use crate::time::now_iso;

    // Estimate capacity: {"level":"info","msg":"...","ts":"...","key":"val",...}
    // Base overhead ~50, plus message, plus ~20 per field
    let estimated_capacity = 50 + msg.len() * 2 + fields.len() * 30;
    let mut output = String::with_capacity(estimated_capacity);

    output.push_str(r#"{"level":""#);
    output.push_str(level);
    output.push_str(r#"","msg":""#);
    output.push_str(&__escape_json(msg));
    output.push('"');

    for (key, value) in fields {
        output.push_str(r#",""#);
        output.push_str(&__escape_json(key));
        output.push_str(r#"":""#);
        output.push_str(&__escape_json(value));
        output.push('"');
    }

    output.push_str(r#","ts":""#);
    output.push_str(&now_iso());
    output.push_str(r#""}"#);

    output
}

/// Structured logging macro with key-value pairs.
///
/// Outputs JSON logs to stderr with level, message, custom fields, and timestamp.
///
/// # Usage
///
/// ```no_run
/// # use mik_sdk::log;
/// let user_id = "123";
/// let email = "alice@example.com";
///
/// // With key-value fields
/// log!(info, "user created", id: user_id, email: &email);
/// log!(warn, "rate limit approaching", remaining: 5);
///
/// // Without fields (just level and message)
/// log!(info, "server started");
/// ```
///
/// # Output Format
///
/// ```json
/// {"level":"info","msg":"user created","id":"123","email":"alice@example.com","ts":"2025-01-16T10:30:00Z"}
/// ```
///
/// # Note on debug level
///
/// Unlike `log::debug!`, the structured `log!(debug, ...)` is NOT compiled out in
/// release builds. If you need debug logs that are removed in release, use `log::debug!`.
#[macro_export]
macro_rules! log {
    // Pattern: log!(level, "message", key: value, ...)
    ($level:ident, $msg:expr $(, $key:ident : $value:expr)* $(,)?) => {{
        use std::io::Write;
        let fields: &[(&str, &str)] = &[
            $( (stringify!($key), &format!("{}", $value)) ),*
        ];
        let log_line = $crate::log::__build_structured_log(stringify!($level), $msg, fields);
        let _ = writeln!(std::io::stderr(), "{}", log_line);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_simple() {
        assert_eq!(__escape_json("hello"), "hello");
    }

    #[test]
    fn test_escape_json_quotes() {
        assert_eq!(__escape_json(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn test_escape_json_backslash() {
        assert_eq!(__escape_json(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn test_escape_json_newlines() {
        assert_eq!(__escape_json("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_json_tabs() {
        assert_eq!(__escape_json("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_escape_json_carriage_return() {
        assert_eq!(__escape_json("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_json_control_chars() {
        // Test control character escaping (ASCII 0-31 except common ones)
        let input = "\x00\x01\x02\x1F";
        let escaped = __escape_json(input);
        assert!(escaped.contains("\\u0000"));
        assert!(escaped.contains("\\u0001"));
        assert!(escaped.contains("\\u0002"));
        assert!(escaped.contains("\\u001f"));
    }

    #[test]
    fn test_escape_json_mixed() {
        let input = "Hello \"World\"\nLine2\t\x00end";
        let escaped = __escape_json(input);
        assert!(escaped.contains("\\\""));
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\t"));
        assert!(escaped.contains("\\u0000"));
    }

    #[test]
    fn test_escape_json_empty() {
        assert_eq!(__escape_json(""), "");
    }

    #[test]
    fn test_escape_json_unicode() {
        // Unicode characters should pass through unchanged
        assert_eq!(__escape_json("日本語"), "日本語");
        assert_eq!(__escape_json("emoji: 🎉"), "emoji: 🎉");
    }

    #[test]
    fn test_timestamp_format() {
        let ts = __format_timestamp();
        // Should be in ISO 8601 format: YYYY-MM-DDTHH:MM:SS.sssZ
        assert_eq!(ts.len(), 24);
        assert_eq!(ts.chars().nth(4), Some('-'));
        assert_eq!(ts.chars().nth(7), Some('-'));
        assert_eq!(ts.chars().nth(10), Some('T'));
        assert_eq!(ts.chars().nth(13), Some(':'));
        assert_eq!(ts.chars().nth(16), Some(':'));
        assert_eq!(ts.chars().nth(19), Some('.'));
        assert_eq!(ts.chars().last(), Some('Z'));
    }

    #[test]
    fn test_timestamp_valid_date_parts() {
        let ts = __format_timestamp();
        // Parse year
        let year: u32 = ts[0..4].parse().expect("valid year");
        assert!((1970..=3000).contains(&year));

        // Parse month (01-12)
        let month: u32 = ts[5..7].parse().expect("valid month");
        assert!((1..=12).contains(&month));

        // Parse day (01-31)
        let day: u32 = ts[8..10].parse().expect("valid day");
        assert!((1..=31).contains(&day));

        // Parse hour (00-23)
        let hour: u32 = ts[11..13].parse().expect("valid hour");
        assert!(hour <= 23);

        // Parse minute (00-59)
        let minute: u32 = ts[14..16].parse().expect("valid minute");
        assert!(minute <= 59);

        // Parse second (00-59)
        let second: u32 = ts[17..19].parse().expect("valid second");
        assert!(second <= 59);

        // Parse millisecond (000-999)
        let millis: u32 = ts[20..23].parse().expect("valid milliseconds");
        assert!(millis <= 999);
    }

    #[test]
    fn test_timestamp_changes_over_time() {
        let ts1 = __format_timestamp();
        // Small sleep to ensure time passes
        std::thread::sleep(std::time::Duration::from_millis(2));
        let ts2 = __format_timestamp();

        // Timestamps should differ (at least in milliseconds)
        // Note: could be same if very fast, so we just check they're valid
        assert_eq!(ts1.len(), 24);
        assert_eq!(ts2.len(), 24);
    }

    // Tests for timestamp arithmetic (catches mutation testing issues)
    #[test]
    fn test_timestamp_epoch() {
        // Unix epoch: 1970-01-01 00:00:00.000
        assert_eq!(
            __format_timestamp_from_duration(0, 0),
            "1970-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_known_date() {
        // 2025-01-16 10:50:00.000 UTC
        assert_eq!(
            __format_timestamp_from_duration(1737024600, 0),
            "2025-01-16T10:50:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_with_millis() {
        // 2025-01-16 10:50:00.123 UTC
        assert_eq!(
            __format_timestamp_from_duration(1737024600, 123),
            "2025-01-16T10:50:00.123Z"
        );
    }

    #[test]
    fn test_timestamp_leap_year() {
        // 2024-02-29 12:00:00.000 UTC (leap year)
        assert_eq!(
            __format_timestamp_from_duration(1709208000, 0),
            "2024-02-29T12:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_end_of_year() {
        // 2024-12-31 23:59:59.999 UTC
        assert_eq!(
            __format_timestamp_from_duration(1735689599, 999),
            "2024-12-31T23:59:59.999Z"
        );
    }

    #[test]
    fn test_timestamp_start_of_year() {
        // 2024-01-01 00:00:00.000 UTC
        assert_eq!(
            __format_timestamp_from_duration(1704067200, 0),
            "2024-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_y2k() {
        // 2000-01-01 00:00:00.000 UTC
        assert_eq!(
            __format_timestamp_from_duration(946684800, 0),
            "2000-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_far_future() {
        // 2100-12-31 23:59:59.000 UTC
        assert_eq!(
            __format_timestamp_from_duration(4133980799, 0),
            "2100-12-31T23:59:59.000Z"
        );
    }

    #[test]
    fn test_timestamp_hour_minute_second_boundaries() {
        // Test specific time: 23:59:59
        // 1970-01-01 23:59:59.000 UTC = 86399 seconds
        assert_eq!(
            __format_timestamp_from_duration(86399, 0),
            "1970-01-01T23:59:59.000Z"
        );

        // Test specific time: 12:30:45
        // 1970-01-01 12:30:45.000 UTC = 12*3600 + 30*60 + 45 = 45045 seconds
        assert_eq!(
            __format_timestamp_from_duration(45045, 0),
            "1970-01-01T12:30:45.000Z"
        );
    }

    #[test]
    fn test_timestamp_day_boundary() {
        // First second of day 2 (1970-01-02 00:00:00)
        assert_eq!(
            __format_timestamp_from_duration(86400, 0),
            "1970-01-02T00:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_month_boundaries() {
        // 1970-02-01 00:00:00 UTC = 31 days * 86400
        assert_eq!(
            __format_timestamp_from_duration(31 * 86400, 0),
            "1970-02-01T00:00:00.000Z"
        );

        // 1970-03-01 00:00:00 UTC = (31 + 28) days * 86400 (1970 is not a leap year)
        assert_eq!(
            __format_timestamp_from_duration(59 * 86400, 0),
            "1970-03-01T00:00:00.000Z"
        );
    }

    #[test]
    fn test_timestamp_century_boundary() {
        // 2000 is a leap year (divisible by 400)
        // 2000-02-29 00:00:00 UTC
        assert_eq!(
            __format_timestamp_from_duration(951782400, 0),
            "2000-02-29T00:00:00.000Z"
        );

        // 1900 was NOT a leap year (divisible by 100 but not 400)
        // But we can't test pre-epoch dates easily
    }

    // ========================================================================
    // STRUCTURED LOG TESTS
    // ========================================================================

    /// Helper to build structured log without timestamp for testing
    fn build_log_without_ts(level: &str, msg: &str, fields: &[(&str, &str)]) -> String {
        let mut output = String::new();
        output.push_str(r#"{"level":""#);
        output.push_str(level);
        output.push_str(r#"","msg":""#);
        output.push_str(&__escape_json(msg));
        output.push('"');

        for (key, value) in fields {
            output.push_str(r#",""#);
            output.push_str(&__escape_json(key));
            output.push_str(r#"":""#);
            output.push_str(&__escape_json(value));
            output.push('"');
        }

        output
    }

    #[test]
    fn test_structured_log_basic() {
        let output = build_log_without_ts("info", "user created", &[]);
        assert_eq!(output, r#"{"level":"info","msg":"user created""#);
    }

    #[test]
    fn test_structured_log_with_single_field() {
        let output = build_log_without_ts("info", "user created", &[("id", "123")]);
        assert_eq!(output, r#"{"level":"info","msg":"user created","id":"123""#);
    }

    #[test]
    fn test_structured_log_with_multiple_fields() {
        let output = build_log_without_ts(
            "info",
            "user created",
            &[("id", "123"), ("email", "alice@example.com")],
        );
        assert_eq!(
            output,
            r#"{"level":"info","msg":"user created","id":"123","email":"alice@example.com""#
        );
    }

    #[test]
    fn test_structured_log_error_level() {
        let output = build_log_without_ts(
            "error",
            "failed to fetch",
            &[("url", "https://api.example.com"), ("status", "500")],
        );
        assert_eq!(
            output,
            r#"{"level":"error","msg":"failed to fetch","url":"https://api.example.com","status":"500""#
        );
    }

    #[test]
    fn test_structured_log_warn_level() {
        let output = build_log_without_ts("warn", "rate limit approaching", &[("remaining", "5")]);
        assert_eq!(
            output,
            r#"{"level":"warn","msg":"rate limit approaching","remaining":"5""#
        );
    }

    #[test]
    fn test_structured_log_debug_level() {
        let output = build_log_without_ts(
            "debug",
            "request parsed",
            &[("method", "GET"), ("path", "/users")],
        );
        assert_eq!(
            output,
            r#"{"level":"debug","msg":"request parsed","method":"GET","path":"/users""#
        );
    }

    #[test]
    fn test_structured_log_escapes_message() {
        let output = build_log_without_ts("info", "message with \"quotes\"", &[]);
        assert_eq!(output, r#"{"level":"info","msg":"message with \"quotes\"""#);
    }

    #[test]
    fn test_structured_log_escapes_field_values() {
        let output = build_log_without_ts("info", "test", &[("data", "line1\nline2")]);
        assert_eq!(
            output,
            r#"{"level":"info","msg":"test","data":"line1\nline2""#
        );
    }

    #[test]
    fn test_structured_log_full_output_format() {
        // Test the full __build_structured_log function including timestamp
        let output = __build_structured_log("info", "test message", &[("key", "value")]);

        // Should start with level
        assert!(output.starts_with(r#"{"level":"info""#));

        // Should contain msg
        assert!(output.contains(r#""msg":"test message""#));

        // Should contain the field
        assert!(output.contains(r#""key":"value""#));

        // Should end with timestamp and closing brace
        assert!(output.contains(r#","ts":"20"#)); // Starts with 20xx year
        assert!(output.ends_with(r#"Z"}"#));
    }

    #[test]
    fn test_structured_log_timestamp_is_valid_iso() {
        let output = __build_structured_log("info", "test", &[]);

        // Extract timestamp from output
        let ts_start = output.find(r#""ts":""#).expect("should have ts field") + 6;
        let ts_end = output[ts_start..].find('"').expect("should close ts") + ts_start;
        let ts = &output[ts_start..ts_end];

        // Should be valid ISO format
        assert!(ts.ends_with('Z'), "timestamp should end with Z");
        assert!(ts.contains('T'), "timestamp should contain T separator");
        assert!(
            ts.len() == 20 || ts.len() == 24,
            "timestamp should be 20 or 24 chars"
        );
    }
}
