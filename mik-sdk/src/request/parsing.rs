//! URL decoding and parsing utilities.
//!
//! This module provides functions for URL decoding and case-insensitive string matching
//! used by the Request module for query string, form body, and header parsing.

use crate::constants::MAX_URL_DECODED_LEN;

/// Error returned when URL decoding fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Decoded output would exceed maximum length.
    TooLong,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLong => write!(f, "url decoded output exceeds maximum length"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Case-insensitive ASCII substring check (no allocation).
#[inline]
pub(super) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    // Edge case: empty needle is always contained, and windows(0) panics
    if needle.is_empty() {
        return true;
    }
    // Edge case: needle longer than haystack can never be contained
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Basic URL decoding (handles %XX sequences and + as space).
///
/// # Errors
///
/// Returns [`DecodeError::TooLong`] if decoded output would exceed
/// `MAX_URL_DECODED_LEN` (64KB). This prevents memory exhaustion from
/// maliciously crafted inputs.
///
/// # Examples
///
/// ```ignore
/// use mik_sdk::url_decode;
///
/// assert_eq!(url_decode("hello%20world").unwrap(), "hello world");
/// assert_eq!(url_decode("hello+world").unwrap(), "hello world");
/// assert_eq!(url_decode("caf%C3%A9").unwrap(), "café");
/// ```
pub fn url_decode(s: &str) -> Result<String, DecodeError> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();

    while let Some(b) = chars.next() {
        // Defense-in-depth: limit decoded output size
        if bytes.len() >= MAX_URL_DECODED_LEN {
            return Err(DecodeError::TooLong);
        }

        match b {
            b'%' => {
                // Try to read two hex digits
                let h1 = chars.next();
                let h2 = chars.next();
                if let (Some(h1), Some(h2)) = (h1, h2) {
                    let hex_str = [h1, h2];
                    if let Ok(hex_str) = std::str::from_utf8(&hex_str)
                        && let Ok(decoded) = u8::from_str_radix(hex_str, 16)
                    {
                        bytes.push(decoded);
                        continue;
                    }
                    // Invalid escape, keep original bytes
                    bytes.push(b'%');
                    bytes.push(h1);
                    bytes.push(h2);
                } else {
                    // Not enough chars after %, keep as-is
                    bytes.push(b'%');
                    if let Some(h1) = h1 {
                        bytes.push(h1);
                    }
                }
            },
            b'+' => bytes.push(b' '),
            _ => bytes.push(b),
        }
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello%20world").unwrap(), "hello world");
        assert_eq!(url_decode("hello+world").unwrap(), "hello world");
        assert_eq!(url_decode("a%2Fb").unwrap(), "a/b");
        assert_eq!(url_decode("plain").unwrap(), "plain");
    }

    #[test]
    fn test_url_decode_utf8() {
        assert_eq!(url_decode("caf%C3%A9").unwrap(), "café");
        assert_eq!(url_decode("%E4%B8%AD%E6%96%87").unwrap(), "中文");
    }

    #[test]
    fn test_url_decode_double_encoding() {
        // %2520 = %20 (double-encoded space)
        assert_eq!(url_decode("%2520").unwrap(), "%20"); // Only decodes one level
    }

    #[test]
    fn test_url_decode_unicode() {
        // ✓ character
        assert_eq!(url_decode("%E2%9C%93").unwrap(), "✓");
        // 日本語
        assert_eq!(url_decode("%E6%97%A5%E6%9C%AC%E8%AA%9E").unwrap(), "日本語");
    }

    #[test]
    fn test_url_decode_invalid_sequences() {
        // Invalid hex
        assert_eq!(url_decode("%GG").unwrap(), "%GG");
        // Incomplete sequence
        assert_eq!(url_decode("%2").unwrap(), "%2");
        assert_eq!(url_decode("%").unwrap(), "%");
        // Mixed valid/invalid
        assert_eq!(url_decode("a%20b%GGc%2").unwrap(), "a b%GGc%2");
    }

    #[test]
    fn test_url_decode_plus_sign() {
        assert_eq!(url_decode("hello+world").unwrap(), "hello world");
        assert_eq!(url_decode("a+b+c").unwrap(), "a b c");
        assert_eq!(url_decode("+++").unwrap(), "   ");
    }

    #[test]
    fn test_contains_ignore_ascii_case() {
        assert!(contains_ignore_ascii_case("application/json", "json"));
        assert!(contains_ignore_ascii_case("APPLICATION/JSON", "json"));
        assert!(contains_ignore_ascii_case("Application/Json", "JSON"));
        assert!(!contains_ignore_ascii_case("text/html", "json"));
        assert!(contains_ignore_ascii_case("", ""));
        assert!(contains_ignore_ascii_case("anything", ""));
        assert!(!contains_ignore_ascii_case("", "something"));
    }
}
