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
//! ```ignore
//! let parsed = json::try_parse(body)?;
//! let name = parsed.path_str(&["user", "name"]);  // Scans bytes, ~500ns
//! let age = parsed.path_int(&["user", "age"]);    // Scans bytes, ~500ns
//! // Total: ~1µs vs 76µs for full tree parse
//! ```
//!
//! For operations that need the full tree (iteration, `get()`, `at()`), the tree
//! is built on first access and cached.
//!
//! # Examples
//!
//! ```ignore
//! use mik_sdk::json::{self, JsonValue};
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
//! // => {"name":"Alice","age":30,"tags":["rust","wasm"]}
//!
//! // Parse JSON and extract values (lazy - fast path)
//! let parsed = json::try_parse(b"{\"user\":{\"name\":\"Bob\"}}").unwrap();
//! let name = parsed.path_str(&["user", "name"]);  // Some("Bob")
//! let age = parsed.path_int_or(&["user", "age"], 0);  // 0 (default)
//! ```

use crate::constants::{MAX_JSON_DEPTH, MAX_JSON_SIZE};
use miniserde::json::{Array, Number, Object, Value};
use std::rc::Rc;

// ============================================================================
// LAZY JSON SCANNER - Scans bytes to find paths without full parsing
// ============================================================================

mod lazy {
    //! Lazy JSON scanner for extracting values without full tree parsing.
    //!
    //! This module provides functions to scan JSON bytes and extract specific
    //! values by path without parsing the entire document.

    /// Find a value at a path in JSON bytes and extract it as a string.
    #[inline]
    pub(super) fn path_str(bytes: &[u8], path: &[&str]) -> Option<String> {
        let (start, end) = find_path_value(bytes, path)?;
        parse_string_value(&bytes[start..end])
    }

    /// Find a value at a path in JSON bytes and extract it as an integer.
    #[inline]
    pub(super) fn path_int(bytes: &[u8], path: &[&str]) -> Option<i64> {
        let (start, end) = find_path_value(bytes, path)?;
        parse_int_value(&bytes[start..end])
    }

    /// Find a value at a path in JSON bytes and extract it as a float.
    #[inline]
    pub(super) fn path_float(bytes: &[u8], path: &[&str]) -> Option<f64> {
        let (start, end) = find_path_value(bytes, path)?;
        parse_float_value(&bytes[start..end])
    }

    /// Find a value at a path in JSON bytes and extract it as a boolean.
    #[inline]
    pub(super) fn path_bool(bytes: &[u8], path: &[&str]) -> Option<bool> {
        let (start, end) = find_path_value(bytes, path)?;
        parse_bool_value(&bytes[start..end])
    }

    /// Check if a path exists in JSON bytes.
    #[inline]
    pub(super) fn path_exists(bytes: &[u8], path: &[&str]) -> bool {
        find_path_value(bytes, path).is_some()
    }

    /// Check if the value at a path is null.
    #[inline]
    pub(super) fn path_is_null(bytes: &[u8], path: &[&str]) -> bool {
        if let Some((start, end)) = find_path_value(bytes, path) {
            let value = &bytes[start..end];
            let trimmed = trim_whitespace(value);
            trimmed == b"null"
        } else {
            false
        }
    }

    /// Find the byte range of a value at a given path.
    /// Returns (start, end) indices into the bytes slice.
    fn find_path_value(bytes: &[u8], path: &[&str]) -> Option<(usize, usize)> {
        if path.is_empty() {
            // Return the whole value
            let start = skip_whitespace(bytes, 0)?;
            let end = find_value_end(bytes, start)?;
            return Some((start, end));
        }

        let mut pos = skip_whitespace(bytes, 0)?;

        // Must start with object
        if bytes.get(pos)? != &b'{' {
            return None;
        }
        pos += 1;

        for (depth, key) in path.iter().enumerate() {
            pos = skip_whitespace(bytes, pos)?;

            // Find the key in current object
            pos = find_object_key(bytes, pos, key)?;

            // Skip the colon
            pos = skip_whitespace(bytes, pos)?;
            if bytes.get(pos)? != &b':' {
                return None;
            }
            pos += 1;
            pos = skip_whitespace(bytes, pos)?;

            if depth == path.len() - 1 {
                // Last key - return the value range
                let end = find_value_end(bytes, pos)?;
                return Some((pos, end));
            } else {
                // Need to descend into nested object
                if bytes.get(pos)? != &b'{' {
                    return None;
                }
                pos += 1;
            }
        }

        None
    }

    /// Find a key in an object starting at pos, return position after the closing quote.
    fn find_object_key(bytes: &[u8], mut pos: usize, target_key: &str) -> Option<usize> {
        loop {
            pos = skip_whitespace(bytes, pos)?;

            match bytes.get(pos)? {
                b'}' => return None, // End of object, key not found
                b'"' => {
                    // Parse string key
                    let key_start = pos + 1;
                    let key_end = find_string_end(bytes, key_start)?;
                    let key_bytes = &bytes[key_start..key_end];

                    pos = key_end + 1; // Move past closing quote

                    if key_matches(key_bytes, target_key) {
                        return Some(pos);
                    }

                    // Skip to the value
                    pos = skip_whitespace(bytes, pos)?;
                    if bytes.get(pos)? != &b':' {
                        return None;
                    }
                    pos += 1;
                    pos = skip_whitespace(bytes, pos)?;

                    // Skip the value
                    pos = find_value_end(bytes, pos)?;

                    // Skip comma if present
                    pos = skip_whitespace(bytes, pos)?;
                    if bytes.get(pos) == Some(&b',') {
                        pos += 1;
                    }
                },
                b',' => {
                    pos += 1;
                },
                _ => return None, // Invalid JSON
            }
        }
    }

    /// Check if key bytes match target (handles escape sequences).
    fn key_matches(key_bytes: &[u8], target: &str) -> bool {
        // Fast path: no escapes
        if !key_bytes.contains(&b'\\') {
            return key_bytes == target.as_bytes();
        }

        // Slow path: unescape and compare
        if let Some(unescaped) = unescape_string(key_bytes) {
            unescaped == target
        } else {
            false
        }
    }

    /// Find the end of a string (position of closing quote).
    #[inline]
    fn find_string_end(bytes: &[u8], mut pos: usize) -> Option<usize> {
        while pos < bytes.len() {
            match bytes[pos] {
                b'"' => return Some(pos),
                b'\\' => pos += 2, // Skip escape sequence
                _ => pos += 1,
            }
        }
        None
    }

    /// Find the end of any JSON value starting at pos.
    #[inline]
    fn find_value_end(bytes: &[u8], pos: usize) -> Option<usize> {
        let b = *bytes.get(pos)?;

        match b {
            b'"' => {
                let end = find_string_end(bytes, pos + 1)?;
                Some(end + 1)
            },
            b'{' => find_balanced_end(bytes, pos, b'{', b'}'),
            b'[' => find_balanced_end(bytes, pos, b'[', b']'),
            b't' => {
                // true
                if bytes.get(pos..pos + 4)? == b"true" {
                    Some(pos + 4)
                } else {
                    None
                }
            },
            b'f' => {
                // false
                if bytes.get(pos..pos + 5)? == b"false" {
                    Some(pos + 5)
                } else {
                    None
                }
            },
            b'n' => {
                // null
                if bytes.get(pos..pos + 4)? == b"null" {
                    Some(pos + 4)
                } else {
                    None
                }
            },
            b'-' | b'0'..=b'9' => {
                // Number
                let mut end = pos;
                while end < bytes.len() {
                    match bytes[end] {
                        b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => end += 1,
                        _ => break,
                    }
                }
                Some(end)
            },
            _ => None,
        }
    }

    /// Find the end of a balanced structure (object or array).
    #[inline]
    fn find_balanced_end(bytes: &[u8], mut pos: usize, open: u8, close: u8) -> Option<usize> {
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

    /// Skip whitespace, return new position.
    fn skip_whitespace(bytes: &[u8], mut pos: usize) -> Option<usize> {
        while pos < bytes.len() {
            match bytes[pos] {
                b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
                _ => return Some(pos),
            }
        }
        Some(pos)
    }

    /// Trim whitespace from a byte slice.
    fn trim_whitespace(bytes: &[u8]) -> &[u8] {
        let start = bytes
            .iter()
            .position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .unwrap_or(bytes.len());
        let end = bytes
            .iter()
            .rposition(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .map(|p| p + 1)
            .unwrap_or(0);
        if start < end { &bytes[start..end] } else { &[] }
    }

    /// Parse a JSON string value from bytes (including quotes).
    fn parse_string_value(bytes: &[u8]) -> Option<String> {
        let trimmed = trim_whitespace(bytes);
        if trimmed.len() < 2 || trimmed[0] != b'"' || trimmed[trimmed.len() - 1] != b'"' {
            return None;
        }
        let inner = &trimmed[1..trimmed.len() - 1];

        // Fast path: no escapes
        if !inner.contains(&b'\\') {
            return std::str::from_utf8(inner).ok().map(String::from);
        }

        // Slow path: unescape
        unescape_string(inner)
    }

    /// Unescape a JSON string (without surrounding quotes).
    fn unescape_string(bytes: &[u8]) -> Option<String> {
        let mut result = String::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    b'/' => result.push('/'),
                    b'b' => result.push('\u{0008}'),
                    b'f' => result.push('\u{000C}'),
                    b'n' => result.push('\n'),
                    b'r' => result.push('\r'),
                    b't' => result.push('\t'),
                    b'u' => {
                        // Unicode escape: \uXXXX
                        if i + 5 < bytes.len() {
                            let hex = std::str::from_utf8(&bytes[i + 2..i + 6]).ok()?;
                            let code = u16::from_str_radix(hex, 16).ok()?;
                            if let Some(c) = char::from_u32(code as u32) {
                                result.push(c);
                            }
                            i += 4; // Extra skip for \uXXXX
                        }
                    },
                    _ => {
                        result.push('\\');
                        result.push(bytes[i + 1] as char);
                    },
                }
                i += 2;
            } else {
                // Regular UTF-8 byte
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        Some(result)
    }

    /// Parse a JSON number as i64.
    fn parse_int_value(bytes: &[u8]) -> Option<i64> {
        let trimmed = trim_whitespace(bytes);
        let s = std::str::from_utf8(trimmed).ok()?;

        // Try parsing as integer first
        if let Ok(i) = s.parse::<i64>() {
            return Some(i);
        }

        // Try parsing as float and converting
        if let Ok(f) = s.parse::<f64>() {
            const MAX_SAFE_INT: f64 = 9007199254740992.0; // 2^53
            if f.is_finite() && f.abs() <= MAX_SAFE_INT && f.fract() == 0.0 {
                return Some(f as i64);
            }
        }

        None
    }

    /// Parse a JSON number as f64.
    fn parse_float_value(bytes: &[u8]) -> Option<f64> {
        let trimmed = trim_whitespace(bytes);
        let s = std::str::from_utf8(trimmed).ok()?;
        let f = s.parse::<f64>().ok()?;
        if f.is_finite() { Some(f) } else { None }
    }

    /// Parse a JSON boolean.
    fn parse_bool_value(bytes: &[u8]) -> Option<bool> {
        let trimmed = trim_whitespace(bytes);
        match trimmed {
            b"true" => Some(true),
            b"false" => Some(false),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_path_str_simple() {
            let json = br#"{"name": "Alice"}"#;
            assert_eq!(path_str(json, &["name"]), Some("Alice".to_string()));
        }

        #[test]
        fn test_path_str_nested() {
            let json = br#"{"user": {"name": "Bob", "age": 30}}"#;
            assert_eq!(path_str(json, &["user", "name"]), Some("Bob".to_string()));
        }

        #[test]
        fn test_path_int() {
            let json = br#"{"user": {"age": 30}}"#;
            assert_eq!(path_int(json, &["user", "age"]), Some(30));
        }

        #[test]
        fn test_path_bool() {
            let json = br#"{"active": true}"#;
            assert_eq!(path_bool(json, &["active"]), Some(true));
        }

        #[test]
        fn test_path_exists() {
            let json = br#"{"user": {"name": "Alice"}}"#;
            assert!(path_exists(json, &["user", "name"]));
            assert!(!path_exists(json, &["user", "missing"]));
        }

        #[test]
        fn test_path_is_null() {
            let json = br#"{"value": null}"#;
            assert!(path_is_null(json, &["value"]));
        }

        #[test]
        fn test_escape_handling() {
            let json = br#"{"msg": "Hello \"World\""}"#;
            assert_eq!(
                path_str(json, &["msg"]),
                Some("Hello \"World\"".to_string())
            );
        }

        #[test]
        fn test_key_with_escapes() {
            let json = br#"{"user\"name": "Alice"}"#;
            // This tests that we handle escaped quotes in keys
            assert_eq!(path_str(json, &["user\"name"]), Some("Alice".to_string()));
        }

        #[test]
        fn test_deeply_nested() {
            let json = br#"{"a": {"b": {"c": {"d": "deep"}}}}"#;
            assert_eq!(
                path_str(json, &["a", "b", "c", "d"]),
                Some("deep".to_string())
            );
        }

        #[test]
        fn test_skip_array_values() {
            let json = br#"{"items": [1, 2, 3], "name": "test"}"#;
            assert_eq!(path_str(json, &["name"]), Some("test".to_string()));
        }

        #[test]
        fn test_skip_nested_objects() {
            let json = br#"{"other": {"x": 1}, "target": "found"}"#;
            assert_eq!(path_str(json, &["target"]), Some("found".to_string()));
        }

        #[test]
        fn test_negative_number() {
            let json = br#"{"value": -42}"#;
            assert_eq!(path_int(json, &["value"]), Some(-42));
        }

        #[test]
        fn test_float_value() {
            let json = br#"{"num": 1.23456}"#;
            let result = path_float(json, &["num"]).unwrap();
            assert!((result - 1.23456).abs() < 0.0001);
        }
    }
}

// Re-export Value for use with map_array/try_map_array
pub use miniserde::json::Value as RawValue;

/// Internal representation of a JSON value.
/// Supports both lazy (byte scanning) and parsed (tree) modes.
#[derive(Clone)]
enum JsonInner {
    /// Lazy mode: stores raw bytes, uses scanning for path_* methods.
    Lazy { bytes: Rc<[u8]> },
    /// Parsed mode: fully parsed tree (used for builder APIs and tree traversal).
    Parsed(Rc<Value>),
}

/// A JSON value with fluent builder API and lazy parsing.
///
/// # Lazy Parsing
///
/// When created via `json::try_parse()`, the value starts in lazy mode.
/// The `path_*` methods scan the raw bytes without building a full tree,
/// which is **10-40x faster** when you only need a few fields.
///
/// Operations that require the full tree (`get()`, `at()`, `keys()`, etc.)
/// trigger a full parse on first access, which is then cached.
///
/// # Thread Safety
///
/// `JsonValue` uses `Rc<Value>` internally and is **not** `Send` or `Sync`.
/// It cannot be shared across threads. This is intentional for WASM targets
/// where single-threaded execution is the norm and `Rc` provides cheaper
/// reference counting than `Arc`.
///
/// If you need thread-safe JSON values, consider using a different JSON
/// library like `serde_json` with its thread-safe `Value` type.
#[derive(Clone)]
pub struct JsonValue {
    inner: JsonInner,
}

impl JsonValue {
    /// Create a JsonValue from a parsed Value (eager mode).
    fn new(v: Value) -> Self {
        Self {
            inner: JsonInner::Parsed(Rc::new(v)),
        }
    }

    /// Create a JsonValue from raw bytes (lazy mode).
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            inner: JsonInner::Lazy {
                bytes: Rc::from(bytes),
            },
        }
    }

    fn null() -> Self {
        Self::new(Value::Null)
    }

    /// Get the raw bytes if in lazy mode.
    fn bytes(&self) -> Option<&[u8]> {
        match &self.inner {
            JsonInner::Lazy { bytes } => Some(bytes),
            JsonInner::Parsed(_) => None,
        }
    }

    /// Parse the bytes and return the Value. Used for tree operations.
    fn parse_bytes(bytes: &[u8]) -> Option<Value> {
        let s = std::str::from_utf8(bytes).ok()?;
        miniserde::json::from_str(s).ok()
    }

    /// Get the Value reference, parsing if needed.
    /// For methods that need the full tree.
    fn value(&self) -> &Value {
        // Static null for returning when parse fails
        static NULL: Value = Value::Null;

        match &self.inner {
            JsonInner::Parsed(v) => v,
            JsonInner::Lazy { .. } => &NULL,
        }
    }

    // === Reading (chainable) ===

    /// Get the Value for tree operations, parsing if in lazy mode.
    fn get_value_for_tree(&self) -> Value {
        match &self.inner {
            JsonInner::Parsed(v) => (**v).clone(),
            JsonInner::Lazy { bytes } => Self::parse_bytes(bytes).unwrap_or(Value::Null),
        }
    }

    /// Get object field (returns null if missing or not an object).
    ///
    /// Note: This triggers a full parse if in lazy mode. For extracting
    /// specific fields, prefer `path_str()`, `path_int()`, etc. which use
    /// lazy scanning.
    #[must_use]
    pub fn get(&self, key: &str) -> JsonValue {
        match self.get_value_for_tree() {
            Value::Object(obj) => obj
                .get(key)
                .cloned()
                .map(JsonValue::new)
                .unwrap_or_else(JsonValue::null),
            _ => JsonValue::null(),
        }
    }

    /// Get array element (returns null if out of bounds or not an array).
    ///
    /// Note: This triggers a full parse if in lazy mode and clones the
    /// underlying Value. For parsing large arrays, use `map_array()` or
    /// `try_map_array()` instead for better performance.
    #[must_use]
    pub fn at(&self, index: usize) -> JsonValue {
        match self.get_value_for_tree() {
            Value::Array(arr) => arr
                .get(index)
                .cloned()
                .map(JsonValue::new)
                .unwrap_or_else(JsonValue::null),
            _ => JsonValue::null(),
        }
    }

    /// Process array elements without per-element cloning.
    ///
    /// This is more efficient than calling `at(i)` in a loop because it
    /// avoids cloning each element's Value. Returns `None` if not an array.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    ///
    /// # Example
    /// ```ignore
    /// let strings: Option<Vec<String>> = value.map_array(|v| {
    ///     match v {
    ///         Value::String(s) => Some(s.clone()),
    ///         _ => None,
    ///     }
    /// });
    /// ```
    #[must_use]
    pub fn map_array<T, F>(&self, f: F) -> Option<Vec<T>>
    where
        F: Fn(&Value) -> Option<T>,
    {
        match self.get_value_for_tree() {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for elem in &arr {
                    result.push(f(elem)?);
                }
                Some(result)
            },
            _ => None,
        }
    }

    /// Process array elements with error handling, without per-element cloning.
    ///
    /// Like `map_array()`, but the function can return errors.
    /// Returns `None` if not an array, `Some(Err(_))` if parsing fails.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn try_map_array<T, E, F>(&self, f: F) -> Option<Result<Vec<T>, E>>
    where
        F: Fn(&Value) -> Result<T, E>,
    {
        match self.get_value_for_tree() {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for elem in &arr {
                    match f(elem) {
                        Ok(v) => result.push(v),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(result))
            },
            _ => None,
        }
    }

    /// Wrap a raw Value reference in a temporary JsonValue for parsing.
    ///
    /// This is useful inside `map_array`/`try_map_array` callbacks when you
    /// need to use JsonValue methods like `get()` or `str()`.
    ///
    /// Note: The returned JsonValue clones the Value, so use sparingly.
    #[must_use]
    pub fn from_raw(value: &Value) -> JsonValue {
        JsonValue::new(value.clone())
    }

    /// As string, None if not a string.
    #[must_use]
    pub fn str(&self) -> Option<String> {
        match self.get_value_for_tree() {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// As string, or default if not a string.
    #[must_use]
    pub fn str_or(&self, default: &str) -> String {
        self.str().unwrap_or_else(|| default.to_string())
    }

    /// As integer, None if not a number.
    #[must_use]
    pub fn int(&self) -> Option<i64> {
        match self.get_value_for_tree() {
            Value::Number(n) => match n {
                Number::I64(i) => Some(i),
                Number::U64(u) => u.try_into().ok(),
                Number::F64(f) => {
                    const MAX_SAFE_INT: f64 = 9007199254740992.0; // 2^53
                    if f.is_finite() && f.abs() <= MAX_SAFE_INT {
                        Some(f as i64)
                    } else {
                        None
                    }
                },
            },
            _ => None,
        }
    }

    /// As integer, or default if not a number.
    #[must_use]
    pub fn int_or(&self, default: i64) -> i64 {
        self.int().unwrap_or(default)
    }

    /// As float, None if not a number.
    ///
    /// # Precision Warning
    ///
    /// Converting large integers to f64 may lose precision. Integers with
    /// absolute value > 2^53 (9,007,199,254,740,992) cannot be represented
    /// exactly in f64. For large integers, use [`int()`](Self::int) instead.
    ///
    /// Non-finite values (NaN, Infinity) return `None`.
    #[must_use]
    pub fn float(&self) -> Option<f64> {
        match self.get_value_for_tree() {
            Value::Number(n) => match n {
                Number::F64(f) if f.is_finite() => Some(f),
                Number::I64(i) => Some(i as f64),
                Number::U64(u) => Some(u as f64),
                _ => None,
            },
            _ => None,
        }
    }

    /// As float, or default if not a number.
    ///
    /// See [`float()`](Self::float) for precision warnings.
    #[must_use]
    pub fn float_or(&self, default: f64) -> f64 {
        self.float().unwrap_or(default)
    }

    /// As boolean, None if not a boolean.
    #[must_use]
    pub fn bool(&self) -> Option<bool> {
        match self.get_value_for_tree() {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// As boolean, or default if not a boolean.
    #[must_use]
    pub fn bool_or(&self, default: bool) -> bool {
        self.bool().unwrap_or(default)
    }

    /// Is this value null?
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self.get_value_for_tree(), Value::Null)
    }

    /// Get object keys (empty if not an object).
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        match self.get_value_for_tree() {
            Value::Object(obj) => obj.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// Get array/object length.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn len(&self) -> Option<usize> {
        match self.get_value_for_tree() {
            Value::Array(arr) => Some(arr.len()),
            Value::Object(obj) => Some(obj.len()),
            _ => None,
        }
    }

    /// Is this an empty array/object?
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len().is_some_and(|l| l == 0)
    }

    // === Path-based accessors (lazy scanning when possible) ===

    /// Navigate to a nested value by path, returning a reference to the raw Value.
    ///
    /// This requires a full parse. For lazy scanning, use `path_str`, `path_int`, etc.
    fn get_path(&self, path: &[&str]) -> Option<&Value> {
        let mut current = self.value();
        for key in path {
            match current {
                Value::Object(obj) => {
                    current = obj.get(*key)?;
                },
                _ => return None,
            }
        }
        Some(current)
    }

    /// Get string at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    /// This is **10-40x faster** than full parsing when you only need a few fields.
    ///
    /// # Example
    /// ```ignore
    /// let parsed = json::try_parse(body)?;
    /// let name = parsed.path_str(&["user", "name"]);  // Lazy scan: ~500ns
    /// ```
    #[must_use]
    pub fn path_str(&self, path: &[&str]) -> Option<String> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_str(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Get string at path, or default.
    #[must_use]
    pub fn path_str_or(&self, path: &[&str], default: &str) -> String {
        self.path_str(path).unwrap_or_else(|| default.to_string())
    }

    /// Get integer at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_int(&self, path: &[&str]) -> Option<i64> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_int(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
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

    /// Get integer at path, or default.
    #[must_use]
    pub fn path_int_or(&self, path: &[&str], default: i64) -> i64 {
        self.path_int(path).unwrap_or(default)
    }

    /// Get float at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_float(&self, path: &[&str]) -> Option<f64> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_float(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::Number(n) => match n {
                Number::F64(f) if f.is_finite() => Some(*f),
                Number::I64(i) => Some(*i as f64),
                Number::U64(u) => Some(*u as f64),
                _ => None,
            },
            _ => None,
        }
    }

    /// Get float at path, or default.
    #[must_use]
    pub fn path_float_or(&self, path: &[&str], default: f64) -> f64 {
        self.path_float(path).unwrap_or(default)
    }

    /// Get boolean at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_bool(&self, path: &[&str]) -> Option<bool> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_bool(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get boolean at path, or default.
    #[must_use]
    pub fn path_bool_or(&self, path: &[&str], default: bool) -> bool {
        self.path_bool(path).unwrap_or(default)
    }

    /// Check if value at path is null.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_is_null(&self, path: &[&str]) -> bool {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_is_null(bytes, path);
        }

        // Fallback: tree traversal
        matches!(self.get_path(path), Some(Value::Null))
    }

    /// Check if path exists (even if null).
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_exists(&self, path: &[&str]) -> bool {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_exists(bytes, path);
        }

        // Fallback: tree traversal
        self.get_path(path).is_some()
    }

    // === Building (fluent) ===

    /// Get mutable access to the parsed value, converting from lazy if needed.
    fn get_parsed_mut(&mut self) -> &mut Rc<Value> {
        // First, ensure we're in parsed mode
        if let JsonInner::Lazy { bytes } = &self.inner {
            let value = Self::parse_bytes(bytes).unwrap_or(Value::Null);
            self.inner = JsonInner::Parsed(Rc::new(value));
        }

        // Now we're guaranteed to be in Parsed mode
        match &mut self.inner {
            JsonInner::Parsed(rc) => rc,
            JsonInner::Lazy { .. } => unreachable!(),
        }
    }

    /// Set object field (creates object if needed).
    ///
    /// Uses copy-on-write via `Rc::make_mut` - only clones the object if
    /// there are multiple references. For typical builder patterns like
    /// `obj().set("a", v1).set("b", v2)`, this is O(1) per set, not O(n).
    #[must_use]
    pub fn set(mut self, key: &str, value: JsonValue) -> JsonValue {
        let inner_val = value.value().clone();
        let rc = self.get_parsed_mut();
        let val_mut = Rc::make_mut(rc);

        if let Value::Object(obj) = val_mut {
            obj.insert(key.to_string(), inner_val);
        } else {
            // Not an object, create new one
            let mut obj = Object::new();
            obj.insert(key.to_string(), inner_val);
            *val_mut = Value::Object(obj);
        }

        self
    }

    /// Push to array (creates array if needed).
    ///
    /// Uses copy-on-write via `Rc::make_mut` - only clones the array if
    /// there are multiple references. For typical builder patterns like
    /// `arr().push(v1).push(v2)`, this is O(1) per push, not O(n).
    #[must_use]
    pub fn push(mut self, value: JsonValue) -> JsonValue {
        let inner_val = value.value().clone();
        let rc = self.get_parsed_mut();
        let val_mut = Rc::make_mut(rc);

        if let Value::Array(arr) = val_mut {
            arr.push(inner_val);
        } else {
            // Not an array, create new one
            let mut arr = Array::new();
            arr.push(inner_val);
            *val_mut = Value::Array(arr);
        }

        self
    }

    // === Output ===

    /// Serialize to JSON bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl std::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            // Lazy mode: bytes are already valid JSON, write directly
            JsonInner::Lazy { bytes } => {
                // Safe: parse() validated UTF-8 before creating Lazy
                let s = std::str::from_utf8(bytes).unwrap_or("null");
                f.write_str(s)
            },
            // Parsed mode: serialize the value
            JsonInner::Parsed(v) => {
                write!(f, "{}", miniserde::json::to_string(&**v))
            },
        }
    }
}

impl std::fmt::Debug for JsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

// === Constructors ===

/// Create an empty object `{}`.
#[must_use]
pub fn obj() -> JsonValue {
    JsonValue::new(Value::Object(Object::new()))
}

/// Create an empty array `[]`.
#[must_use]
pub fn arr() -> JsonValue {
    JsonValue::new(Value::Array(Array::new()))
}

/// Create a string value.
#[must_use]
pub fn str<S: AsRef<str>>(value: S) -> JsonValue {
    JsonValue::new(Value::String(value.as_ref().to_string()))
}

/// Create an integer value.
#[must_use]
pub fn int(value: i64) -> JsonValue {
    JsonValue::new(Value::Number(Number::I64(value)))
}

/// Create a float value.
///
/// # Precision Note
///
/// JSON numbers are typically parsed as f64 by JavaScript, which has limited
/// precision for integers > 2^53. If you're building JSON for JavaScript consumption,
/// consider using string values for large integers to preserve precision.
#[must_use]
pub fn float(value: f64) -> JsonValue {
    JsonValue::new(Value::Number(Number::F64(value)))
}

/// Create a boolean value.
#[must_use]
pub fn bool(value: bool) -> JsonValue {
    JsonValue::new(Value::Bool(value))
}

/// Create a null value.
#[must_use]
pub fn null() -> JsonValue {
    JsonValue::null()
}

// ============================================================================
// ToJson TRAIT - Type inference for JSON macros
// ============================================================================

/// A trait for types that can be converted to JSON values.
///
/// This trait enables type inference in the `json!` and `ok!` macros,
/// allowing you to write:
///
/// ```ignore
/// ok!({ "name": name, "age": age })
/// ```
///
/// Instead of the more verbose:
///
/// ```ignore
/// ok!({ "name": str(name), "age": int(age) })
/// ```
///
/// # Implementations
///
/// This trait is implemented for:
/// - Strings: `String`, `&str`, `&String`, `Cow<str>`
/// - Integers: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `isize`
/// - Floats: `f32`, `f64`
/// - Boolean: `bool`
/// - Optional: `Option<T>` where T: ToJson (None becomes null)
/// - Arrays: `Vec<T>`, `&[T]` where T: ToJson
/// - JSON: `JsonValue` (pass-through)
///
/// # Example
///
/// ```ignore
/// use mik_sdk::json::{ToJson, JsonValue};
///
/// let name = "Alice".to_string();
/// let age = 30;
/// let active = true;
/// let tags: Vec<&str> = vec!["admin", "user"];
///
/// // All these types implement ToJson
/// let json = json::obj()
///     .set("name", name.to_json())
///     .set("age", age.to_json())
///     .set("active", active.to_json())
///     .set("tags", tags.to_json());
/// ```
pub trait ToJson {
    /// Convert this value to a JSON value.
    fn to_json(&self) -> JsonValue;
}

// === String implementations ===

impl ToJson for String {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(self)
    }
}

impl ToJson for &str {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(*self)
    }
}

impl ToJson for std::borrow::Cow<'_, str> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(self.as_ref())
    }
}

// === Integer implementations ===

impl ToJson for i8 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i16 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self)
    }
}

impl ToJson for isize {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self as i64)
    }
}

impl ToJson for u8 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u16 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        // Note: Values > i64::MAX will be truncated
        int(*self as i64)
    }
}

impl ToJson for usize {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self as i64)
    }
}

// === Float implementations ===

impl ToJson for f32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        float(f64::from(*self))
    }
}

impl ToJson for f64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        float(*self)
    }
}

// === Boolean implementation ===

impl ToJson for bool {
    #[inline]
    fn to_json(&self) -> JsonValue {
        self::bool(*self)
    }
}

// === Option implementation (None becomes null) ===

impl<T: ToJson> ToJson for Option<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        match self {
            Some(v) => v.to_json(),
            None => null(),
        }
    }
}

// === Array implementations ===

impl<T: ToJson> ToJson for Vec<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in self {
            result = result.push(item.to_json());
        }
        result
    }
}

impl<T: ToJson> ToJson for &[T] {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in *self {
            result = result.push(item.to_json());
        }
        result
    }
}

// Fixed-size array implementations for common sizes
impl<T: ToJson, const N: usize> ToJson for [T; N] {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in self {
            result = result.push(item.to_json());
        }
        result
    }
}

// === JsonValue pass-through ===

impl ToJson for JsonValue {
    #[inline]
    fn to_json(&self) -> JsonValue {
        self.clone()
    }
}

// === Reference implementations ===

impl<T: ToJson + ?Sized> ToJson for &T {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (*self).to_json()
    }
}

impl<T: ToJson + ?Sized> ToJson for &mut T {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson + ?Sized> ToJson for Box<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson> ToJson for std::rc::Rc<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson> ToJson for std::sync::Arc<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

/// Check if JSON nesting depth exceeds limit.
fn json_depth_exceeds_limit(data: &[u8]) -> bool {
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
/// - Input exceeds 1MB (`MAX_JSON_SIZE`)
/// - Nesting depth exceeds 20 levels (`MAX_JSON_DEPTH`, heuristic check)
/// - Input is not valid UTF-8
///
/// Note: Syntax validation is deferred until values are accessed or full parse
/// is triggered. Invalid JSON may return `None` from `path_*` methods.
#[must_use]
pub fn try_parse(data: &[u8]) -> Option<JsonValue> {
    if data.len() > MAX_JSON_SIZE {
        return None;
    }
    if json_depth_exceeds_limit(data) {
        return None;
    }
    // Validate UTF-8 upfront
    std::str::from_utf8(data).ok()?;

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
/// - Input exceeds 1MB (`MAX_JSON_SIZE`)
/// - Nesting depth exceeds 20 levels (`MAX_JSON_DEPTH`, heuristic check)
/// - Input is not valid UTF-8
/// - JSON syntax is invalid
#[must_use]
pub fn try_parse_full(data: &[u8]) -> Option<JsonValue> {
    if data.len() > MAX_JSON_SIZE {
        return None;
    }
    if json_depth_exceeds_limit(data) {
        return None;
    }
    let s = std::str::from_utf8(data).ok()?;
    let parsed: Value = miniserde::json::from_str(s).ok()?;
    Some(JsonValue::new(parsed))
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
pub fn raw_float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => match n {
            Number::F64(f) if f.is_finite() => Some(*f),
            Number::I64(i) => Some(*i as f64),
            Number::U64(u) => Some(*u as f64),
            _ => None,
        },
        _ => None,
    }
}

/// Extract boolean from raw Value (for use in map_array callbacks).
#[inline]
#[must_use]
pub fn raw_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// Check if raw Value is null (for use in map_array callbacks).
#[inline]
#[must_use]
pub fn raw_is_null(v: &Value) -> bool {
    matches!(v, Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PROPTEST PROPERTY TESTS - Fuzz parsers to ensure no panics
    // =========================================================================

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Test that json::try_parse doesn't panic on arbitrary bytes.
            /// Malformed input should return None, never panic.
            #[test]
            fn parse_doesnt_panic_on_arbitrary_bytes(input in prop::collection::vec(any::<u8>(), 0..1024)) {
                let _ = try_parse(&input); // Should not panic
            }

            /// Test that json::try_parse doesn't panic on arbitrary strings.
            #[test]
            fn parse_doesnt_panic_on_arbitrary_strings(input in ".*") {
                let _ = try_parse(input.as_bytes()); // Should not panic
            }

            /// Test that deeply nested JSON doesn't cause stack overflow.
            /// The parser should reject deep nesting gracefully.
            #[test]
            fn parse_rejects_deep_nesting_gracefully(depth in 1usize..100) {
                // Generate nested objects: {"a":{"a":{"a":...}}}
                let mut json = String::new();
                for _ in 0..depth {
                    json.push_str("{\"a\":");
                }
                json.push('1');
                for _ in 0..depth {
                    json.push('}');
                }

                // Should not panic, may return None for deep nesting
                let _ = try_parse(json.as_bytes());
            }

            /// Test that deeply nested arrays don't cause stack overflow.
            #[test]
            fn parse_rejects_deep_array_nesting_gracefully(depth in 1usize..100) {
                // Generate nested arrays: [[[[...]]]]
                let mut json = String::new();
                for _ in 0..depth {
                    json.push('[');
                }
                json.push('1');
                for _ in 0..depth {
                    json.push(']');
                }

                // Should not panic, may return None for deep nesting
                let _ = try_parse(json.as_bytes());
            }

            /// Test that unicode strings are handled correctly.
            #[test]
            fn parse_handles_unicode_strings(s in "\\PC*") {
                // Valid JSON string with unicode content
                let json = format!(r#"{{"text": "{}"}}"#, s.replace('\\', "\\\\").replace('"', "\\\""));
                let _ = try_parse(json.as_bytes()); // Should not panic
            }

            /// Test that valid UTF-8 strings in JSON are parsed correctly.
            #[test]
            fn parse_handles_valid_utf8(s in "[a-zA-Z0-9 ]{0,100}") {
                let json = format!(r#"{{"value": "{}"}}"#, s);
                let result = try_parse(json.as_bytes());
                // Valid JSON should parse successfully
                prop_assert!(result.is_some());
                let value = result.unwrap();
                prop_assert_eq!(value.path_str(&["value"]), Some(s));
            }

            /// Test numeric edge cases - very large integers.
            #[test]
            fn parse_handles_large_integers(n in i64::MIN..=i64::MAX) {
                let json = format!(r#"{{"n": {}}}"#, n);
                let result = try_parse(json.as_bytes());
                // Should parse without panic
                prop_assert!(result.is_some());
            }

            /// Test numeric edge cases - very large unsigned integers.
            #[test]
            fn parse_handles_large_unsigned(n in 0u64..=u64::MAX) {
                let json = format!(r#"{{"n": {}}}"#, n);
                let result = try_parse(json.as_bytes());
                // Should parse without panic
                prop_assert!(result.is_some());
            }

            /// Test numeric edge cases - floating point numbers.
            #[test]
            fn parse_handles_floats(f in any::<f64>().prop_filter("must be finite", |x| x.is_finite())) {
                let json = format!(r#"{{"n": {}}}"#, f);
                let result = try_parse(json.as_bytes());
                // Should parse without panic (finite floats are valid JSON)
                prop_assert!(result.is_some());
            }

            /// Test that NaN representations don't crash the parser.
            /// JSON doesn't support NaN, so these should parse as strings or fail gracefully.
            #[test]
            fn parse_handles_nan_like_strings(s in prop::sample::select(vec![
                "NaN", "nan", "NAN", "Infinity", "-Infinity", "inf", "-inf"
            ])) {
                // As raw value (invalid JSON number)
                let json_raw = format!(r#"{{"n": {}}}"#, s);
                let _ = try_parse(json_raw.as_bytes()); // Should not panic

                // As string value (valid JSON)
                let json_str = format!(r#"{{"n": "{}"}}"#, s);
                let result = try_parse(json_str.as_bytes());
                prop_assert!(result.is_some());
            }

            /// Test that scientific notation is handled.
            #[test]
            fn parse_handles_scientific_notation(
                mantissa in -1000i64..1000i64,
                exponent in -308i32..308i32
            ) {
                let json = format!(r#"{{"n": {}e{}}}"#, mantissa, exponent);
                let _ = try_parse(json.as_bytes()); // Should not panic
            }

            /// Test that very long strings don't cause issues.
            #[test]
            fn parse_handles_long_strings(len in 0usize..10000) {
                let long_string = "x".repeat(len);
                let json = format!(r#"{{"s": "{}"}}"#, long_string);
                let result = try_parse(json.as_bytes());
                // Should parse without panic (within 1MB limit)
                prop_assert!(result.is_some());
            }

            /// Test that arrays with many elements are handled.
            #[test]
            fn parse_handles_large_arrays(len in 0usize..1000) {
                let elements: Vec<String> = (0..len).map(|i| i.to_string()).collect();
                let json = format!("[{}]", elements.join(","));
                let result = try_parse(json.as_bytes());
                prop_assert!(result.is_some());
            }

            /// Test that objects with many keys are handled.
            #[test]
            fn parse_handles_large_objects(len in 0usize..500) {
                let entries: Vec<String> = (0..len).map(|i| format!(r#""k{}": {}"#, i, i)).collect();
                let json = format!("{{{}}}", entries.join(","));
                let result = try_parse(json.as_bytes());
                prop_assert!(result.is_some());
            }

            /// Test that json_depth_exceeds_limit doesn't panic on arbitrary input.
            #[test]
            fn depth_check_doesnt_panic(input in prop::collection::vec(any::<u8>(), 0..2048)) {
                let _ = json_depth_exceeds_limit(&input); // Should not panic
            }

            /// Test that braces in strings don't affect depth calculation.
            #[test]
            fn depth_check_ignores_braces_in_strings(
                prefix in "[a-z]{0,10}",
                braces in "[\\{\\}\\[\\]]{0,50}",
                suffix in "[a-z]{0,10}"
            ) {
                // Create a valid JSON with braces inside a string
                let json = format!(r#"{{"key": "{}{}{}"}}"#, prefix, braces, suffix);
                let result = try_parse(json.as_bytes());
                // Valid JSON with braces in strings should parse (depth = 1)
                prop_assert!(result.is_some());
            }

            /// Test that escape sequences in strings are handled.
            #[test]
            fn parse_handles_escape_sequences(s in prop::sample::select(vec![
                r#"\""#, r#"\\"#, r#"\/"#, r#"\b"#, r#"\f"#, r#"\n"#, r#"\r"#, r#"\t"#
            ])) {
                let json = format!(r#"{{"s": "{}"}}"#, s);
                let result = try_parse(json.as_bytes());
                prop_assert!(result.is_some());
            }

            /// Test Unicode escape sequences.
            #[test]
            fn parse_handles_unicode_escapes(code in 0u16..0xFFFF) {
                // Skip surrogate pairs as they're invalid in JSON
                if !(0xD800..=0xDFFF).contains(&code) {
                    let json = format!(r#"{{"s": "\\u{:04X}"}}"#, code);
                    let _ = try_parse(json.as_bytes()); // Should not panic
                }
            }

            /// Test mixed nested structures.
            #[test]
            fn parse_handles_mixed_nesting(depth in 1usize..20) {
                // Alternate between objects and arrays
                let mut json = String::new();
                for i in 0..depth {
                    if i % 2 == 0 {
                        json.push_str("{\"a\":");
                    } else {
                        json.push('[');
                    }
                }
                json.push('1');
                for i in (0..depth).rev() {
                    if i % 2 == 0 {
                        json.push('}');
                    } else {
                        json.push(']');
                    }
                }

                let result = try_parse(json.as_bytes());
                prop_assert!(result.is_some());
            }
        }
    }

    #[test]
    fn test_build_object() {
        let v = obj().set("name", str("Alice")).set("age", int(30));
        assert_eq!(v.to_string(), r#"{"age":30,"name":"Alice"}"#);
    }

    #[test]
    fn test_build_array() {
        let v = arr().push(int(1)).push(int(2)).push(int(3));
        assert_eq!(v.to_string(), "[1,2,3]");
    }

    #[test]
    fn test_parse_and_read() {
        let v = try_parse(b"{\"name\":\"Bob\",\"age\":25}").unwrap();
        assert_eq!(v.get("name").str(), Some("Bob".to_string()));
        assert_eq!(v.get("age").int(), Some(25));
        assert!(v.get("missing").is_null());
    }

    #[test]
    fn test_nested() {
        let v = obj().set("user", obj().set("name", str("Alice")));
        assert_eq!(v.get("user").get("name").str(), Some("Alice".to_string()));
    }

    #[test]
    fn test_array_access() {
        let v = arr().push(str("a")).push(str("b"));
        assert_eq!(v.at(0).str(), Some("a".to_string()));
        assert_eq!(v.at(1).str(), Some("b".to_string()));
        assert!(v.at(2).is_null());
    }

    #[test]
    fn test_path_accessors() {
        let v = try_parse(b"{\"user\":{\"name\":\"Alice\",\"age\":30,\"active\":true}}").unwrap();

        // path_str
        assert_eq!(v.path_str(&["user", "name"]), Some("Alice".to_string()));
        assert_eq!(v.path_str(&["user", "missing"]), None);
        assert_eq!(v.path_str_or(&["user", "name"], "default"), "Alice");
        assert_eq!(v.path_str_or(&["user", "missing"], "default"), "default");

        // path_int
        assert_eq!(v.path_int(&["user", "age"]), Some(30));
        assert_eq!(v.path_int(&["user", "missing"]), None);
        assert_eq!(v.path_int_or(&["user", "age"], 0), 30);
        assert_eq!(v.path_int_or(&["user", "missing"], 0), 0);

        // path_bool
        assert_eq!(v.path_bool(&["user", "active"]), Some(true));
        assert_eq!(v.path_bool(&["user", "missing"]), None);
        assert!(v.path_bool_or(&["user", "active"], false));
        assert!(!v.path_bool_or(&["user", "missing"], false));

        // path_exists / path_is_null
        assert!(v.path_exists(&["user", "name"]));
        assert!(!v.path_exists(&["user", "missing"]));

        let v2 = try_parse(b"{\"user\":{\"value\":null}}").unwrap();
        assert!(v2.path_is_null(&["user", "value"]));
        assert!(v2.path_exists(&["user", "value"]));
    }

    #[test]
    fn test_path_deep_nesting() {
        let v = try_parse(b"{\"a\":{\"b\":{\"c\":{\"d\":\"deep\"}}}}").unwrap();
        assert_eq!(v.path_str(&["a", "b", "c", "d"]), Some("deep".to_string()));
        assert_eq!(v.path_str(&["a", "b", "c", "missing"]), None);
        assert_eq!(v.path_str(&["a", "b", "missing", "d"]), None);
    }

    // ========================================================================
    // JSON DEPTH BOUNDARY TESTS
    // ========================================================================

    /// Generate nested JSON objects: {"a":{"a":{"a":...}}} at specified depth
    fn generate_nested_objects(depth: usize) -> String {
        let mut json = String::new();
        for _ in 0..depth {
            json.push_str("{\"a\":");
        }
        json.push('1');
        for _ in 0..depth {
            json.push('}');
        }
        json
    }

    /// Generate nested JSON arrays: [[[[...]]]] at specified depth
    fn generate_nested_arrays(depth: usize) -> String {
        let mut json = String::new();
        for _ in 0..depth {
            json.push('[');
        }
        json.push('1');
        for _ in 0..depth {
            json.push(']');
        }
        json
    }

    /// Generate mixed nested JSON: {"a":[{"a":[...]}]} alternating objects and arrays
    fn generate_mixed_nesting(depth: usize) -> String {
        let mut json = String::new();
        for i in 0..depth {
            if i % 2 == 0 {
                json.push_str("{\"a\":");
            } else {
                json.push('[');
            }
        }
        json.push('1');
        for i in (0..depth).rev() {
            if i % 2 == 0 {
                json.push('}');
            } else {
                json.push(']');
            }
        }
        json
    }

    #[test]
    fn test_depth_limit_objects_at_19() {
        // Depth 19: should succeed (below the limit of 20)
        let json = generate_nested_objects(19);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 19 levels of object nesting should parse successfully"
        );
    }

    #[test]
    fn test_depth_limit_objects_at_20() {
        // Depth 20: should succeed (exactly at the limit)
        let json = generate_nested_objects(20);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 20 levels of object nesting should parse successfully (at limit)"
        );
    }

    #[test]
    fn test_depth_limit_objects_at_21() {
        // Depth 21: should fail (exceeds the limit of 20)
        let json = generate_nested_objects(21);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_none(),
            "JSON with 21 levels of object nesting should be rejected (exceeds limit)"
        );
    }

    #[test]
    fn test_depth_limit_arrays_at_19() {
        // Depth 19: should succeed (below the limit of 20)
        let json = generate_nested_arrays(19);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 19 levels of array nesting should parse successfully"
        );
    }

    #[test]
    fn test_depth_limit_arrays_at_20() {
        // Depth 20: should succeed (exactly at the limit)
        let json = generate_nested_arrays(20);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 20 levels of array nesting should parse successfully (at limit)"
        );
    }

    #[test]
    fn test_depth_limit_arrays_at_21() {
        // Depth 21: should fail (exceeds the limit of 20)
        let json = generate_nested_arrays(21);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_none(),
            "JSON with 21 levels of array nesting should be rejected (exceeds limit)"
        );
    }

    #[test]
    fn test_depth_limit_mixed_at_19() {
        // Mixed nesting at depth 19: should succeed
        let json = generate_mixed_nesting(19);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 19 levels of mixed nesting should parse successfully"
        );
    }

    #[test]
    fn test_depth_limit_mixed_at_20() {
        // Mixed nesting at depth 20: should succeed (at limit)
        let json = generate_mixed_nesting(20);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "JSON with 20 levels of mixed nesting should parse successfully (at limit)"
        );
    }

    #[test]
    fn test_depth_limit_mixed_at_21() {
        // Mixed nesting at depth 21: should fail (exceeds limit)
        let json = generate_mixed_nesting(21);
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_none(),
            "JSON with 21 levels of mixed nesting should be rejected (exceeds limit)"
        );
    }

    #[test]
    fn test_depth_check_ignores_braces_in_strings() {
        // Braces inside strings should not count towards depth
        // This is valid JSON with depth 1, but contains many braces in strings
        let json = r#"{"key": "{{{{{{{{{{{{{{{{{{{{{{{{{{"}"#;
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "Braces inside strings should not affect depth calculation"
        );
    }

    #[test]
    fn test_depth_check_handles_escaped_quotes() {
        // Escaped quotes inside strings should be handled correctly
        let json = r#"{"key": "value with \" escaped quote and {nested}"}"#;
        let result = try_parse(json.as_bytes());
        assert!(
            result.is_some(),
            "Escaped quotes should be handled correctly in depth check"
        );
    }

    #[test]
    fn test_json_depth_exceeds_limit_directly() {
        // Test the internal function directly for precise boundary checks
        let json_19 = generate_nested_objects(19);
        let json_20 = generate_nested_objects(20);
        let json_21 = generate_nested_objects(21);

        assert!(
            !json_depth_exceeds_limit(json_19.as_bytes()),
            "Depth 19 should not exceed limit"
        );
        assert!(
            !json_depth_exceeds_limit(json_20.as_bytes()),
            "Depth 20 should not exceed limit (at boundary)"
        );
        assert!(
            json_depth_exceeds_limit(json_21.as_bytes()),
            "Depth 21 should exceed limit"
        );
    }

    // ========================================================================
    // ToJson TRAIT TESTS
    // ========================================================================

    mod to_json_tests {
        use super::*;
        use std::borrow::Cow;

        // === String type tests ===

        #[test]
        fn test_string_to_json() {
            let s = String::from("hello");
            let json = s.to_json();
            assert_eq!(json.to_string(), r#""hello""#);
        }

        #[test]
        fn test_str_to_json() {
            let s: &str = "world";
            let json = s.to_json();
            assert_eq!(json.to_string(), r#""world""#);
        }

        #[test]
        fn test_string_ref_to_json() {
            let s = String::from("test");
            let json = s.to_json();
            assert_eq!(json.to_string(), r#""test""#);
        }

        #[test]
        fn test_cow_str_to_json() {
            let borrowed: Cow<'_, str> = Cow::Borrowed("borrowed");
            let owned: Cow<'_, str> = Cow::Owned(String::from("owned"));

            assert_eq!(borrowed.to_json().to_string(), r#""borrowed""#);
            assert_eq!(owned.to_json().to_string(), r#""owned""#);
        }

        #[test]
        fn test_string_with_escapes_to_json() {
            let s = "hello \"world\"\nwith\ttabs";
            let json = s.to_json();
            // Check it serializes correctly (escapes the quotes, newlines, tabs)
            let output = json.to_string();
            assert!(output.contains("hello"));
            assert!(output.starts_with('"'));
            assert!(output.ends_with('"'));
        }

        #[test]
        fn test_empty_string_to_json() {
            let s = "";
            let json = s.to_json();
            assert_eq!(json.to_string(), r#""""#);
        }

        #[test]
        fn test_unicode_string_to_json() {
            let s = "こんにちは 🌍";
            let json = s.to_json();
            let output = json.to_string();
            assert!(output.contains("こんにちは"));
            assert!(output.contains("🌍"));
        }

        // === Integer type tests ===

        #[test]
        fn test_i8_to_json() {
            assert_eq!(42i8.to_json().to_string(), "42");
            assert_eq!((-128i8).to_json().to_string(), "-128");
            assert_eq!(127i8.to_json().to_string(), "127");
        }

        #[test]
        fn test_i16_to_json() {
            assert_eq!(1000i16.to_json().to_string(), "1000");
            assert_eq!(i16::MIN.to_json().to_string(), "-32768");
            assert_eq!(i16::MAX.to_json().to_string(), "32767");
        }

        #[test]
        fn test_i32_to_json() {
            assert_eq!(123456i32.to_json().to_string(), "123456");
            assert_eq!((-1i32).to_json().to_string(), "-1");
            assert_eq!(0i32.to_json().to_string(), "0");
        }

        #[test]
        fn test_i64_to_json() {
            assert_eq!(i64::MAX.to_json().to_string(), "9223372036854775807");
            assert_eq!(i64::MIN.to_json().to_string(), "-9223372036854775808");
        }

        #[test]
        fn test_isize_to_json() {
            let val: isize = 42;
            assert_eq!(val.to_json().to_string(), "42");
        }

        #[test]
        fn test_u8_to_json() {
            assert_eq!(0u8.to_json().to_string(), "0");
            assert_eq!(255u8.to_json().to_string(), "255");
        }

        #[test]
        fn test_u16_to_json() {
            assert_eq!(u16::MAX.to_json().to_string(), "65535");
        }

        #[test]
        fn test_u32_to_json() {
            assert_eq!(u32::MAX.to_json().to_string(), "4294967295");
        }

        #[test]
        fn test_u64_to_json() {
            // Note: u64::MAX > i64::MAX, so it will be truncated
            // But values within i64 range work fine
            assert_eq!(1000000u64.to_json().to_string(), "1000000");
        }

        #[test]
        fn test_usize_to_json() {
            let val: usize = 42;
            assert_eq!(val.to_json().to_string(), "42");
        }

        // === Float type tests ===

        #[test]
        fn test_f32_to_json() {
            let val: f32 = 1.23;
            let output = val.to_json().to_string();
            assert!(output.starts_with("1.23"));
        }

        #[test]
        fn test_f64_to_json() {
            let val: f64 = 9.87654321;
            let output = val.to_json().to_string();
            assert!(output.starts_with("9.876"));
        }

        #[test]
        fn test_float_zero_to_json() {
            assert_eq!(0.0f64.to_json().to_string(), "0.0");
        }

        #[test]
        fn test_float_negative_to_json() {
            let val: f64 = -99.99;
            let output = val.to_json().to_string();
            assert!(output.starts_with("-99.99"));
        }

        // === Boolean tests ===

        #[test]
        fn test_bool_true_to_json() {
            assert_eq!(true.to_json().to_string(), "true");
        }

        #[test]
        fn test_bool_false_to_json() {
            assert_eq!(false.to_json().to_string(), "false");
        }

        // === Option tests ===

        #[test]
        fn test_some_string_to_json() {
            let opt: Option<String> = Some("hello".to_string());
            assert_eq!(opt.to_json().to_string(), r#""hello""#);
        }

        #[test]
        fn test_none_string_to_json() {
            let opt: Option<String> = None;
            assert_eq!(opt.to_json().to_string(), "null");
        }

        #[test]
        fn test_some_i32_to_json() {
            let opt: Option<i32> = Some(42);
            assert_eq!(opt.to_json().to_string(), "42");
        }

        #[test]
        fn test_none_i32_to_json() {
            let opt: Option<i32> = None;
            assert_eq!(opt.to_json().to_string(), "null");
        }

        #[test]
        fn test_nested_option_to_json() {
            let opt: Option<Option<i32>> = Some(Some(42));
            assert_eq!(opt.to_json().to_string(), "42");

            let opt2: Option<Option<i32>> = Some(None);
            assert_eq!(opt2.to_json().to_string(), "null");

            let opt3: Option<Option<i32>> = None;
            assert_eq!(opt3.to_json().to_string(), "null");
        }

        // === Vec tests ===

        #[test]
        fn test_vec_string_to_json() {
            let v: Vec<String> = vec!["a".to_string(), "b".to_string(), "c".to_string()];
            assert_eq!(v.to_json().to_string(), r#"["a","b","c"]"#);
        }

        #[test]
        fn test_vec_str_to_json() {
            let v: Vec<&str> = vec!["x", "y", "z"];
            assert_eq!(v.to_json().to_string(), r#"["x","y","z"]"#);
        }

        #[test]
        fn test_vec_i32_to_json() {
            let v: Vec<i32> = vec![1, 2, 3, 4, 5];
            assert_eq!(v.to_json().to_string(), "[1,2,3,4,5]");
        }

        #[test]
        fn test_empty_vec_to_json() {
            let v: Vec<i32> = vec![];
            assert_eq!(v.to_json().to_string(), "[]");
        }

        #[test]
        fn test_vec_with_options_to_json() {
            let v: Vec<Option<i32>> = vec![Some(1), None, Some(3)];
            assert_eq!(v.to_json().to_string(), "[1,null,3]");
        }

        // === Slice tests ===

        #[test]
        fn test_slice_to_json() {
            let arr = [1, 2, 3];
            let slice: &[i32] = &arr;
            assert_eq!(slice.to_json().to_string(), "[1,2,3]");
        }

        #[test]
        fn test_str_slice_to_json() {
            let arr = ["a", "b"];
            let slice: &[&str] = &arr;
            assert_eq!(slice.to_json().to_string(), r#"["a","b"]"#);
        }

        // === Fixed-size array tests ===

        #[test]
        fn test_array_i32_to_json() {
            let arr: [i32; 3] = [10, 20, 30];
            assert_eq!(arr.to_json().to_string(), "[10,20,30]");
        }

        #[test]
        fn test_array_str_to_json() {
            let arr: [&str; 2] = ["hello", "world"];
            assert_eq!(arr.to_json().to_string(), r#"["hello","world"]"#);
        }

        #[test]
        fn test_empty_array_to_json() {
            let arr: [i32; 0] = [];
            assert_eq!(arr.to_json().to_string(), "[]");
        }

        // === JsonValue pass-through tests ===

        #[test]
        fn test_json_value_to_json() {
            let original = obj().set("key", str("value"));
            let converted = original.to_json();
            assert_eq!(converted.to_string(), r#"{"key":"value"}"#);
        }

        // === Reference type tests ===

        #[test]
        fn test_box_to_json() {
            let boxed: Box<i32> = Box::new(42);
            assert_eq!(boxed.to_json().to_string(), "42");
        }

        #[test]
        fn test_box_string_to_json() {
            let boxed: Box<String> = Box::new("boxed".to_string());
            assert_eq!(boxed.to_json().to_string(), r#""boxed""#);
        }

        #[test]
        fn test_rc_to_json() {
            use std::rc::Rc;
            let rc: Rc<i32> = Rc::new(99);
            assert_eq!(rc.to_json().to_string(), "99");
        }

        #[test]
        fn test_arc_to_json() {
            use std::sync::Arc;
            let arc: Arc<String> = Arc::new("shared".to_string());
            assert_eq!(arc.to_json().to_string(), r#""shared""#);
        }

        // === Complex nested type tests ===

        #[test]
        fn test_vec_of_vecs_to_json() {
            let matrix: Vec<Vec<i32>> = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
            assert_eq!(matrix.to_json().to_string(), "[[1,2],[3,4],[5,6]]");
        }

        #[test]
        fn test_option_vec_to_json() {
            let opt: Option<Vec<i32>> = Some(vec![1, 2, 3]);
            assert_eq!(opt.to_json().to_string(), "[1,2,3]");

            let none: Option<Vec<i32>> = None;
            assert_eq!(none.to_json().to_string(), "null");
        }

        #[test]
        fn test_vec_of_options_to_json() {
            let v: Vec<Option<&str>> = vec![Some("a"), None, Some("c")];
            assert_eq!(v.to_json().to_string(), r#"["a",null,"c"]"#);
        }

        // === Building objects with ToJson ===

        #[test]
        fn test_build_object_with_to_json() {
            let name = "Alice".to_string();
            let age: i32 = 30;
            let active = true;
            let tags: Vec<&str> = vec!["admin", "user"];
            let score: Option<f64> = Some(95.5);
            let nickname: Option<String> = None;

            let json = obj()
                .set("name", name.to_json())
                .set("age", age.to_json())
                .set("active", active.to_json())
                .set("tags", tags.to_json())
                .set("score", score.to_json())
                .set("nickname", nickname.to_json());

            let output = json.to_string();
            assert!(output.contains(r#""name":"Alice""#));
            assert!(output.contains(r#""age":30"#));
            assert!(output.contains(r#""active":true"#));
            assert!(output.contains(r#""tags":["admin","user"]"#));
            assert!(output.contains(r#""nickname":null"#));
        }

        // === Property-based tests for ToJson ===

        mod proptest_to_json {
            use super::*;
            use proptest::prelude::*;

            proptest! {
                #[test]
                fn string_roundtrip(s in ".*") {
                    let json = s.to_json();
                    // Should produce valid JSON string
                    let output = json.to_string();
                    prop_assert!(output.starts_with('"'));
                    prop_assert!(output.ends_with('"'));
                }

                #[test]
                fn i32_roundtrip(n in any::<i32>()) {
                    let json = n.to_json();
                    let output = json.to_string();
                    // Parse it back
                    let parsed: i64 = output.parse().unwrap();
                    prop_assert_eq!(parsed, n as i64);
                }

                #[test]
                fn i64_roundtrip(n in any::<i64>()) {
                    let json = n.to_json();
                    let output = json.to_string();
                    let parsed: i64 = output.parse().unwrap();
                    prop_assert_eq!(parsed, n);
                }

                #[test]
                fn bool_roundtrip(b in any::<bool>()) {
                    let json = b.to_json();
                    let output = json.to_string();
                    prop_assert_eq!(output, if b { "true" } else { "false" });
                }

                #[test]
                fn option_none_is_null(opt in Just(None::<i32>)) {
                    let json = opt.to_json();
                    prop_assert_eq!(json.to_string(), "null");
                }

                #[test]
                fn vec_length_preserved(v in prop::collection::vec(any::<i32>(), 0..100)) {
                    let json = v.to_json();
                    let len = json.len();
                    prop_assert_eq!(len, Some(v.len()));
                }
            }
        }
    }
}
