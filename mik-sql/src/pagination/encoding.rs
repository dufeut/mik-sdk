//! Base64 encoding/decoding utilities for cursor serialization.

/// Simple base64 encoding (URL-safe, no padding).
///
/// # Why Custom Implementation?
///
/// This crate avoids external dependencies for base64 encoding/decoding to:
/// 1. Minimize binary size in WASM targets
/// 2. Avoid dependency version conflicts
/// 3. Keep the implementation simple and auditable
///
/// The implementation uses URL-safe alphabet (`-_` instead of `+/`) and omits
/// padding, making cursors safe for use in query strings without additional encoding.
pub(super) fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));

        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(n & 0x3F) as usize] as char);
        }
    }

    result
}

/// Simple base64 decoding (URL-safe, no padding).
///
/// Accepts both URL-safe (`-_`) and standard (`+/`) alphabet for compatibility.
/// See [`base64_encode`] for rationale on custom implementation.
pub(super) fn base64_decode(input: &str) -> Result<String, ()> {
    const DECODE: [i8; 128] = {
        let mut table = [-1i8; 128];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i as i8;
            table[(b'a' + i) as usize] = (i + 26) as i8;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            table[(b'0' + i) as usize] = (i + 52) as i8;
            i += 1;
        }
        table[b'-' as usize] = 62;
        table[b'_' as usize] = 63;
        // Also support standard base64
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let bytes: Vec<u8> = input.bytes().collect();
    let mut result = Vec::new();

    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        let mut valid_chars = 0;

        for (i, &b) in chunk.iter().enumerate() {
            if b as usize >= 128 {
                return Err(());
            }
            let val = DECODE[b as usize];
            if val < 0 {
                return Err(());
            }
            n |= (val as u32) << (18 - i * 6);
            valid_chars += 1;
        }

        result.push((n >> 16) as u8);
        if valid_chars > 2 {
            result.push((n >> 8) as u8);
        }
        if valid_chars > 3 {
            result.push(n as u8);
        }
    }

    String::from_utf8(result).map_err(|_| ())
}

/// Escape a string for JSON per RFC 8259.
///
/// Escapes:
/// - `"` -> `\"`
/// - `\` -> `\\`
/// - Control characters (U+0000 to U+001F) -> `\uXXXX` or named escapes
pub(super) fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"), // backspace
            '\x0C' => result.push_str("\\f"), // form feed
            // Other control characters (U+0000 to U+001F)
            c if c.is_control() && (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            },
            c => result.push(c),
        }
    }
    result
}

/// Unescape a JSON string per RFC 8259.
pub(super) fn unescape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('u') => {
                    // Parse \uXXXX escape
                    let mut hex = String::with_capacity(4);
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            hex.push(h);
                        }
                    }
                    if let Ok(code) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                },
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Split JSON object into key:value pairs, respecting nesting.
pub(super) fn split_json_pairs(s: &str) -> Vec<&str> {
    let mut pairs = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, c) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }

        match c {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                pairs.push(&s[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }

    if start < s.len() {
        pairs.push(&s[start..]);
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let original = "{\"id\":100,\"name\":\"test\"}";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    // --- Base64 encoding edge cases ---

    #[test]
    fn test_base64_encode_empty() {
        let encoded = base64_encode("");
        assert_eq!(encoded, "");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_base64_encode_single_char() {
        // 1 byte input: produces 2 base64 chars
        let encoded = base64_encode("a");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "a");
    }

    #[test]
    fn test_base64_encode_two_chars() {
        // 2 byte input: produces 3 base64 chars
        let encoded = base64_encode("ab");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "ab");
    }

    #[test]
    fn test_base64_encode_three_chars() {
        // 3 byte input: produces 4 base64 chars (exact alignment)
        let encoded = base64_encode("abc");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "abc");
    }

    #[test]
    fn test_base64_encode_four_chars() {
        // 4 byte input: 3 + 1 bytes
        let encoded = base64_encode("abcd");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "abcd");
    }

    #[test]
    fn test_base64_encode_five_chars() {
        // 5 byte input: 3 + 2 bytes
        let encoded = base64_encode("abcde");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "abcde");
    }

    #[test]
    fn test_base64_url_safe_alphabet() {
        // Test that URL-safe characters are used
        let encoded = base64_encode("test data with spaces");
        // Should not contain + or /
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        // Should decode correctly
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "test data with spaces");
    }

    #[test]
    fn test_base64_decode_standard_alphabet() {
        // The decoder should accept standard base64 (+/) for compatibility
        // Create an encoded string with standard alphabet manually
        // "abc" in standard base64 is "YWJj"
        let decoded = base64_decode("YWJj").unwrap();
        assert_eq!(decoded, "abc");
    }

    // --- Base64 decode error cases ---

    #[test]
    fn test_base64_decode_invalid_char() {
        // Invalid character that's not in base64 alphabet
        let result = base64_decode("abc!");
        assert!(result.is_err());
    }

    #[test]
    fn test_base64_decode_non_ascii() {
        // Non-ASCII character (>= 128)
        let result = base64_decode("abc\u{00FF}");
        assert!(result.is_err());
    }

    #[test]
    fn test_base64_decode_invalid_utf8_result() {
        // Create base64 that decodes to invalid UTF-8
        // 0xFF 0xFE are invalid UTF-8 start bytes
        // Base64 of [0xFF, 0xFE] is "__4"
        let result = base64_decode("__4");
        assert!(result.is_err());
    }

    #[test]
    fn test_base64_decode_whitespace() {
        // Whitespace is not valid base64
        let result = base64_decode("abc def");
        assert!(result.is_err());
    }

    #[test]
    fn test_base64_decode_equals_padding() {
        // Our implementation doesn't use padding, but equals sign should fail
        let result = base64_decode("YWJj=");
        assert!(result.is_err());
    }

    // --- JSON escaping tests ---

    #[test]
    fn test_escape_json_quotes() {
        assert_eq!(escape_json("say \"hello\""), "say \\\"hello\\\"");
    }

    #[test]
    fn test_escape_json_backslash() {
        assert_eq!(escape_json("path\\to\\file"), "path\\\\to\\\\file");
    }

    #[test]
    fn test_escape_json_newline() {
        assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_json_carriage_return() {
        assert_eq!(escape_json("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_json_tab() {
        assert_eq!(escape_json("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_escape_json_backspace() {
        assert_eq!(escape_json("text\x08here"), "text\\bhere");
    }

    #[test]
    fn test_escape_json_form_feed() {
        assert_eq!(escape_json("text\x0Chere"), "text\\fhere");
    }

    #[test]
    fn test_escape_json_null_char() {
        assert_eq!(escape_json("text\x00here"), "text\\u0000here");
    }

    #[test]
    fn test_escape_json_control_char() {
        // Control character 0x01 (SOH)
        assert_eq!(escape_json("text\x01here"), "text\\u0001here");
    }

    #[test]
    fn test_escape_json_bell() {
        // Bell character 0x07
        assert_eq!(escape_json("text\x07here"), "text\\u0007here");
    }

    #[test]
    fn test_escape_json_no_escaping_needed() {
        assert_eq!(escape_json("hello world"), "hello world");
    }

    #[test]
    fn test_escape_json_empty() {
        assert_eq!(escape_json(""), "");
    }

    #[test]
    fn test_escape_json_unicode() {
        // Unicode characters should pass through unchanged
        assert_eq!(escape_json("hello"), "hello");
    }

    #[test]
    fn test_escape_json_combined() {
        assert_eq!(
            escape_json("say \"hi\"\nand\\bye"),
            "say \\\"hi\\\"\\nand\\\\bye"
        );
    }

    // --- JSON unescaping tests ---

    #[test]
    fn test_unescape_json_quotes() {
        assert_eq!(unescape_json("say \\\"hello\\\""), "say \"hello\"");
    }

    #[test]
    fn test_unescape_json_backslash() {
        assert_eq!(unescape_json("path\\\\to\\\\file"), "path\\to\\file");
    }

    #[test]
    fn test_unescape_json_newline() {
        assert_eq!(unescape_json("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_unescape_json_carriage_return() {
        assert_eq!(unescape_json("line1\\rline2"), "line1\rline2");
    }

    #[test]
    fn test_unescape_json_tab() {
        assert_eq!(unescape_json("col1\\tcol2"), "col1\tcol2");
    }

    #[test]
    fn test_unescape_json_backspace() {
        assert_eq!(unescape_json("text\\bhere"), "text\x08here");
    }

    #[test]
    fn test_unescape_json_form_feed() {
        assert_eq!(unescape_json("text\\fhere"), "text\x0Chere");
    }

    #[test]
    fn test_unescape_json_forward_slash() {
        // JSON allows \/ escape for forward slash
        assert_eq!(unescape_json("path\\/to"), "path/to");
    }

    #[test]
    fn test_unescape_json_unicode_escape() {
        assert_eq!(unescape_json("\\u0041"), "A"); // U+0041 is 'A'
    }

    #[test]
    fn test_unescape_json_unicode_escape_lowercase() {
        assert_eq!(unescape_json("\\u006e"), "n"); // U+006E is 'n'
    }

    #[test]
    fn test_unescape_json_unicode_null() {
        assert_eq!(unescape_json("\\u0000"), "\x00");
    }

    #[test]
    fn test_unescape_json_unicode_high() {
        assert_eq!(unescape_json("\\u00e9"), "\u{00e9}"); // e with acute
    }

    #[test]
    fn test_unescape_json_invalid_escape() {
        // Invalid escape sequences should be kept as-is
        assert_eq!(unescape_json("\\x"), "\\x");
    }

    #[test]
    fn test_unescape_json_trailing_backslash() {
        // Backslash at end of string
        assert_eq!(unescape_json("text\\"), "text\\");
    }

    #[test]
    fn test_unescape_json_empty() {
        assert_eq!(unescape_json(""), "");
    }

    #[test]
    fn test_unescape_json_no_escapes() {
        assert_eq!(unescape_json("hello world"), "hello world");
    }

    #[test]
    fn test_unescape_json_incomplete_unicode() {
        // \u followed by less than 4 hex digits - parses what it can find (00 -> \x00)
        assert_eq!(unescape_json("\\u00"), "\x00");
    }

    #[test]
    fn test_unescape_json_invalid_unicode_surrogate() {
        // Surrogate codepoints (D800-DFFF) are not valid Unicode scalar values
        // char::from_u32 returns None for these, so they should be silently dropped
        let result = unescape_json("before\\uD800after");
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        let original = "Hello \"World\"!\nPath: C:\\Users\\test\ttab";
        let escaped = escape_json(original);
        let unescaped = unescape_json(&escaped);
        assert_eq!(original, unescaped);
    }

    // --- split_json_pairs tests ---

    #[test]
    fn test_split_json_pairs_simple() {
        let pairs = split_json_pairs("\"a\":1,\"b\":2");
        assert_eq!(pairs, vec!["\"a\":1", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_single() {
        let pairs = split_json_pairs("\"a\":1");
        assert_eq!(pairs, vec!["\"a\":1"]);
    }

    #[test]
    fn test_split_json_pairs_empty() {
        let pairs = split_json_pairs("");
        assert!(pairs.is_empty() || pairs == vec![""]);
    }

    #[test]
    fn test_split_json_pairs_nested_object() {
        let pairs = split_json_pairs("\"a\":{\"x\":1},\"b\":2");
        assert_eq!(pairs, vec!["\"a\":{\"x\":1}", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_nested_array() {
        let pairs = split_json_pairs("\"a\":[1,2,3],\"b\":4");
        assert_eq!(pairs, vec!["\"a\":[1,2,3]", "\"b\":4"]);
    }

    #[test]
    fn test_split_json_pairs_string_with_comma() {
        let pairs = split_json_pairs("\"a\":\"hello, world\",\"b\":2");
        assert_eq!(pairs, vec!["\"a\":\"hello, world\"", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_string_with_brace() {
        let pairs = split_json_pairs("\"a\":\"text{brace}\",\"b\":2");
        assert_eq!(pairs, vec!["\"a\":\"text{brace}\"", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_escaped_quote_in_string() {
        let pairs = split_json_pairs("\"a\":\"say \\\"hi\\\"\",\"b\":2");
        assert_eq!(pairs, vec!["\"a\":\"say \\\"hi\\\"\"", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_multiple_nested() {
        let pairs = split_json_pairs("\"a\":{\"x\":[1,2]},\"b\":[{\"y\":3}]");
        assert_eq!(pairs, vec!["\"a\":{\"x\":[1,2]}", "\"b\":[{\"y\":3}]"]);
    }

    #[test]
    fn test_split_json_pairs_three_pairs() {
        let pairs = split_json_pairs("\"a\":1,\"b\":2,\"c\":3");
        assert_eq!(pairs, vec!["\"a\":1", "\"b\":2", "\"c\":3"]);
    }

    #[test]
    fn test_split_json_pairs_escaped_backslash_in_string() {
        let pairs = split_json_pairs("\"a\":\"path\\\\to\",\"b\":2");
        assert_eq!(pairs, vec!["\"a\":\"path\\\\to\"", "\"b\":2"]);
    }

    #[test]
    fn test_split_json_pairs_trailing_comma() {
        // When input ends with a comma, start == s.len(), so no final push
        let pairs = split_json_pairs("\"a\":1,");
        assert_eq!(pairs, vec!["\"a\":1"]);
    }

    // --- Combined base64 + JSON tests ---

    #[test]
    fn test_base64_json_roundtrip() {
        let json = "{\"name\":\"test\",\"value\":123}";
        let encoded = base64_encode(json);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(json, decoded);
    }

    #[test]
    fn test_base64_with_escaped_json() {
        let json = "{\"msg\":\"Hello \\\"World\\\"\"}";
        let encoded = base64_encode(json);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(json, decoded);
    }

    #[test]
    fn test_base64_long_string() {
        // Test with a longer string that spans multiple chunks
        let long = "a".repeat(100);
        let encoded = base64_encode(&long);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(long, decoded);
    }

    #[test]
    fn test_base64_all_bytes() {
        // Test with various printable ASCII characters
        let input = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let encoded = base64_encode(input);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(input, decoded);
    }
}
