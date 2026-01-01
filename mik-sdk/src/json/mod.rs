//! JSON parsing and building using miniserde.
//!
//! This module provides a fluent API for building and parsing JSON values.
//! All JSON operations are pure Rust - no WIT component calls.
//!
//! # Lazy Parsing
//!
//! When you call `json::try_parse()`, the JSON is parsed lazily. The `path_*` methods
//! scan the raw bytes to find values without building a full tree. This is
//! **10-40x faster** when you only need a few fields:
//!
//! ```
//! # use mik_sdk::json;
//! let body = br#"{"user":{"name":"Alice","age":30}}"#;
//! let parsed = json::try_parse(body).unwrap();
//! let name = parsed.path_str(&["user", "name"]);  // Scans bytes, ~500ns
//! let age = parsed.path_int(&["user", "age"]);    // Scans bytes, ~500ns
//! assert_eq!(name, Some("Alice".to_string()));
//! assert_eq!(age, Some(30));
//! ```
//!
//! For operations that need the full tree (iteration, `get()`, `at()`), the tree
//! is built on first access and cached.
//!
//! # Examples
//!
//! ```
//! use mik_sdk::json;
//!
//! // Build JSON
//! let value = json::obj()
//!     .set("name", json::str("Alice"))
//!     .set("age", json::int(30))
//!     .set("tags", json::arr()
//!         .push(json::str("rust"))
//!         .push(json::str("wasm")));
//!
//! // Serialize to string
//! let s = value.to_string();
//! assert!(s.contains("Alice"));
//! assert!(s.contains("30"));
//!
//! // Parse JSON and extract values (lazy - fast path)
//! let parsed = json::try_parse(br#"{"user":{"name":"Bob"}}"#).unwrap();
//! let name = parsed.path_str(&["user", "name"]);  // Some("Bob")
//! let age = parsed.path_int_or(&["user", "age"], 0);  // 0 (default)
//! assert_eq!(name, Some("Bob".to_string()));
//! assert_eq!(age, 0);
//! ```

mod builder;
mod lazy;
#[cfg(test)]
mod tests;
mod to_json;
mod value;

use crate::constants::{MAX_JSON_DEPTH, get_max_json_size};
use miniserde::json::{Number, Value};

// Re-export public types and functions
pub use builder::{arr, bool, float, int, null, obj, str};
pub use to_json::ToJson;
pub use value::JsonValue;

// Re-export Value for use with map_array/try_map_array
pub use miniserde::json::Value as RawValue;

/// Check if JSON nesting depth exceeds limit.
pub(crate) fn json_depth_exceeds_limit(data: &[u8]) -> bool {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escape = false;

    for &byte in data {
        if escape {
            escape = false;
            continue;
        }

        match byte {
            b'\\' if in_string => escape = true,
            b'"' => in_string = !in_string,
            b'[' | b'{' if !in_string => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return true;
                }
            },
            b']' | b'}' if !in_string => {
                depth = depth.saturating_sub(1);
            },
            _ => {},
        }
    }

    false
}

/// Parse JSON from bytes (lazy mode).
///
/// The JSON is not fully parsed immediately. Instead, the raw bytes are stored
/// and values are extracted on-demand using `path_*` methods. This is **10-40x
/// faster** when you only need a few fields from the JSON.
///
/// For operations that need the full tree (`get()`, `at()`, `keys()`, iteration),
/// the full parse is triggered on first access and cached.
///
/// # Returns
///
/// Returns `None` if:
/// - Input exceeds max size (default 1MB, configurable via `MIK_MAX_JSON_SIZE`)
/// - Nesting depth exceeds 20 levels (`MAX_JSON_DEPTH`, heuristic check)
/// - Input is not valid UTF-8
/// - **Non-whitespace content exists after the JSON value** (security: prevents injection)
///
/// Note: Beyond trailing content validation, syntax validation is deferred until
/// values are accessed or full parse is triggered. Invalid JSON may return `None`
/// from `path_*` methods.
#[must_use]
pub fn try_parse(data: &[u8]) -> Option<JsonValue> {
    if data.len() > get_max_json_size() {
        return None;
    }
    if json_depth_exceeds_limit(data) {
        return None;
    }
    // Validate UTF-8 upfront
    std::str::from_utf8(data).ok()?;

    // Validate no trailing content (security: prevents JSON injection attacks).
    // This is checked upfront even for lazy parsing because accepting
    // `{"key":"value"}garbage` could lead to security issues.
    let value_end = find_json_value_end(data)?;
    if has_trailing_content(data, value_end) {
        return None;
    }

    // Return lazy JsonValue - parsing happens on demand
    Some(JsonValue::from_bytes(data))
}

/// Parse JSON from bytes eagerly (full tree parse).
///
/// Unlike `try_parse()`, this immediately parses the entire JSON into a tree.
/// Use this when you need to access many fields or iterate over arrays.
///
/// # Returns
///
/// Returns `None` if:
/// - Input exceeds max size (default 1MB, configurable via `MIK_MAX_JSON_SIZE`)
/// - Nesting depth exceeds 20 levels (`MAX_JSON_DEPTH`, heuristic check)
/// - Input is not valid UTF-8
/// - JSON syntax is invalid
/// - **Non-whitespace content exists after the JSON value** (security: prevents injection)
#[must_use]
pub fn try_parse_full(data: &[u8]) -> Option<JsonValue> {
    if data.len() > get_max_json_size() {
        return None;
    }
    if json_depth_exceeds_limit(data) {
        return None;
    }
    let s = std::str::from_utf8(data).ok()?;

    // Find where the JSON value ends, then verify only whitespace follows.
    // This prevents accepting input like `{"key":"value"}garbage` which could
    // be a security issue (e.g., JSON injection, log forging).
    let value_end = find_json_value_end(s.as_bytes())?;
    if has_trailing_content(s.as_bytes(), value_end) {
        return None;
    }

    let parsed: Value = miniserde::json::from_str(s).ok()?;
    Some(JsonValue::new(parsed))
}

/// Check if there's non-whitespace content after position `pos`.
#[inline]
fn has_trailing_content(bytes: &[u8], pos: usize) -> bool {
    bytes[pos..]
        .iter()
        .any(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

/// Find the end position of a JSON value starting at the beginning of bytes.
/// Returns the position after the complete JSON value.
fn find_json_value_end(bytes: &[u8]) -> Option<usize> {
    let pos = skip_ws(bytes, 0);
    let b = *bytes.get(pos)?;

    match b {
        b'"' => find_string_end_pos(bytes, pos + 1).map(|end| end + 1),
        b'{' => find_balanced_end_pos(bytes, pos, b'{', b'}'),
        b'[' => find_balanced_end_pos(bytes, pos, b'[', b']'),
        b't' => bytes
            .get(pos..pos + 4)
            .filter(|s| *s == b"true")
            .map(|_| pos + 4),
        b'f' => bytes
            .get(pos..pos + 5)
            .filter(|s| *s == b"false")
            .map(|_| pos + 5),
        b'n' => bytes
            .get(pos..pos + 4)
            .filter(|s| *s == b"null")
            .map(|_| pos + 4),
        b'-' | b'0'..=b'9' => {
            let mut end = pos;
            while end < bytes.len()
                && matches!(bytes[end], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
            {
                end += 1;
            }
            Some(end)
        },
        _ => None,
    }
}

#[inline]
fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

fn find_string_end_pos(bytes: &[u8], mut pos: usize) -> Option<usize> {
    while pos < bytes.len() {
        match bytes[pos] {
            b'"' => return Some(pos),
            b'\\' => pos += 2,
            _ => pos += 1,
        }
    }
    None
}

fn find_balanced_end_pos(bytes: &[u8], mut pos: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    while pos < bytes.len() {
        let b = bytes[pos];

        if escape {
            escape = false;
            pos += 1;
            continue;
        }

        match b {
            b'\\' if in_string => escape = true,
            b'"' => in_string = !in_string,
            _ if in_string => {},
            _ if b == open => depth += 1,
            _ if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos + 1);
                }
            },
            _ => {},
        }
        pos += 1;
    }
    None
}

// ============================================================================
// RAW VALUE HELPERS (for use with map_array/try_map_array)
// ============================================================================

/// Extract string from raw Value (for use in map_array callbacks).
#[inline]
#[must_use]
pub fn raw_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Extract integer from raw Value (for use in map_array callbacks).
#[inline]
#[must_use]
pub fn raw_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => match n {
            Number::I64(i) => Some(*i),
            Number::U64(u) => (*u).try_into().ok(),
            Number::F64(f) => {
                const MAX_SAFE_INT: f64 = 9007199254740992.0;
                if f.is_finite() && f.abs() <= MAX_SAFE_INT {
                    Some(*f as i64)
                } else {
                    None
                }
            },
        },
        _ => None,
    }
}

/// Extract float from raw Value (for use in map_array callbacks).
#[inline]
#[must_use]
#[allow(clippy::cast_precision_loss)] // Documented: large i64/u64 may lose precision
pub const fn raw_float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => match n {
            Number::F64(f) if f.is_finite() => Some(*f),
            Number::I64(i) => Some(*i as f64),
            Number::U64(u) => Some(*u as f64),
            Number::F64(_) => None, // Non-finite f64
        },
        _ => None,
    }
}

/// Extract boolean from raw Value (for use in map_array callbacks).
#[inline]
#[must_use]
pub const fn raw_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// Check if raw Value is null (for use in map_array callbacks).
#[inline]
#[must_use]
pub const fn raw_is_null(v: &Value) -> bool {
    matches!(v, Value::Null)
}
