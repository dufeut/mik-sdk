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
        let start = skip_whitespace(bytes, 0);
        let end = find_value_end(bytes, start)?;
        return Some((start, end));
    }

    let mut pos = skip_whitespace(bytes, 0);

    // Must start with object
    if bytes.get(pos)? != &b'{' {
        return None;
    }
    pos += 1;

    for (depth, key) in path.iter().enumerate() {
        pos = skip_whitespace(bytes, pos);

        // Find the key in current object
        pos = find_object_key(bytes, pos, key)?;

        // Skip the colon
        pos = skip_whitespace(bytes, pos);
        if bytes.get(pos)? != &b':' {
            return None;
        }
        pos += 1;
        pos = skip_whitespace(bytes, pos);

        if depth == path.len() - 1 {
            // Last key - return the value range
            let end = find_value_end(bytes, pos)?;
            return Some((pos, end));
        }
        // Need to descend into nested object
        if bytes.get(pos)? != &b'{' {
            return None;
        }
        pos += 1;
    }

    None
}

/// Find a key in an object starting at pos, return position after the closing quote.
fn find_object_key(bytes: &[u8], mut pos: usize, target_key: &str) -> Option<usize> {
    loop {
        pos = skip_whitespace(bytes, pos);

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
                pos = skip_whitespace(bytes, pos);
                if bytes.get(pos)? != &b':' {
                    return None;
                }
                pos += 1;
                pos = skip_whitespace(bytes, pos);

                // Skip the value
                pos = find_value_end(bytes, pos)?;

                // Skip comma if present
                pos = skip_whitespace(bytes, pos);
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
    unescape_string(key_bytes).is_some_and(|unescaped| unescaped == target)
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
fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() {
        match bytes[pos] {
            b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
            _ => return pos,
        }
    }
    pos
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
        .map_or(0, |p| p + 1);
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
                        if let Some(c) = char::from_u32(u32::from(code)) {
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
