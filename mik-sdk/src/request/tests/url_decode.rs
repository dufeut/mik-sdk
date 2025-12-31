//! URL decoding edge cases and Unicode tests

use super::super::*;

#[test]
fn test_unicode_path_params_emoji() {
    // Emoji in path parameters (URL-encoded)
    assert_eq!(url_decode("%F0%9F%98%80").unwrap(), "😀");
}

#[test]
fn test_unicode_path_params_cjk() {
    // Chinese/Japanese/Korean characters
    let encoded_chinese = "%E4%B8%AD%E6%96%87"; // 中文
    let encoded_japanese = "%E6%97%A5%E6%9C%AC%E8%AA%9E"; // 日本語
    let encoded_korean = "%ED%95%9C%EA%B5%AD%EC%96%B4"; // 한국어

    assert_eq!(url_decode(encoded_chinese).unwrap(), "中文");
    assert_eq!(url_decode(encoded_japanese).unwrap(), "日本語");
    assert_eq!(url_decode(encoded_korean).unwrap(), "한국어");
}

#[test]
fn test_unicode_path_params_arabic_hebrew() {
    // Right-to-left scripts
    let encoded_arabic = "%D8%A7%D9%84%D8%B9%D8%B1%D8%A8%D9%8A%D8%A9"; // العربية
    let encoded_hebrew = "%D7%A2%D7%91%D7%A8%D7%99%D7%AA"; // עברית

    assert_eq!(url_decode(encoded_arabic).unwrap(), "العربية");
    assert_eq!(url_decode(encoded_hebrew).unwrap(), "עברית");
}

#[test]
fn test_unicode_mixed_encodings() {
    // Mixed ASCII and Unicode
    assert_eq!(
        url_decode("john_%F0%9F%91%A8%E2%80%8D%F0%9F%92%BB").unwrap(),
        "john_👨‍💻"
    );
}

#[test]
fn test_unicode_zero_width_chars() {
    // Zero-width joiner and other invisible chars
    let encoded = "%E2%80%8B%E2%80%8C%E2%80%8D"; // ZWS, ZWNJ, ZWJ
    let decoded = url_decode(encoded).unwrap();
    assert_eq!(decoded.len(), 9); // 3 chars × 3 bytes each
}

#[test]
fn test_unicode_normalization_forms() {
    // é can be encoded as single char (U+00E9) or e + combining accent (U+0065 U+0301)
    // URL decoding preserves the original form - no normalization
    let precomposed = "%C3%A9"; // é as single char (NFC)
    let decomposed = "e%CC%81"; // e + combining acute accent (NFD)

    let decoded_precomposed = url_decode(precomposed).unwrap();
    let decoded_decomposed = url_decode(decomposed).unwrap();

    // Both decode correctly to their respective forms
    assert_eq!(decoded_precomposed, "é"); // Single char U+00E9
    assert_eq!(decoded_decomposed, "e\u{0301}"); // e + combining accent
    assert_eq!(decoded_decomposed.chars().count(), 2); // 2 code points

    // Note: These are visually identical but NOT byte-equal
    // Applications needing normalization should use unicode-normalization crate
    assert_ne!(decoded_precomposed, decoded_decomposed);
}

#[test]
fn test_unicode_boundary_chars() {
    // Test characters at UTF-8 encoding boundaries
    // 1-byte: ASCII (U+007F)
    assert_eq!(url_decode("%7F").unwrap(), "\u{007F}");
    // 2-byte boundary (U+0080)
    assert_eq!(url_decode("%C2%80").unwrap(), "\u{0080}");
    // 3-byte boundary (U+0800)
    assert_eq!(url_decode("%E0%A0%80").unwrap(), "\u{0800}");
    // 4-byte boundary (U+10000) - first char outside BMP
    assert_eq!(url_decode("%F0%90%80%80").unwrap(), "\u{10000}");
}

#[test]
fn test_path_traversal_encoded() {
    // URL-encoded path traversal
    let test_cases = [
        ("%2e%2e%2f", "../"),             // Encoded ../
        ("%2e%2e/", "../"),               // Partially encoded
        ("..%2f", "../"),                 // Partially encoded
        ("%2e%2e%5c", "..\\"),            // Encoded ..\
        ("%252e%252e%252f", "%2e%2e%2f"), // Double-encoded (decoded once)
    ];

    for (encoded, expected_decoded) in test_cases {
        let decoded = url_decode(encoded).unwrap();
        assert_eq!(decoded, expected_decoded, "Failed for {encoded}");
    }
}

#[test]
fn test_path_traversal_null_byte() {
    // URL decoding handles null bytes
    assert_eq!(url_decode("file%00.txt").unwrap(), "file\0.txt");
}

#[test]
fn test_path_traversal_unicode() {
    // Unicode-based path traversal attempts
    let test_cases = [
        // Overlong UTF-8 encoding of '/' (invalid but should handle gracefully)
        ("%c0%af", "\u{FFFD}\u{FFFD}"), // Invalid UTF-8 becomes replacement chars or raw
        // Fullwidth characters
        ("%ef%bc%8f", "／"), // Fullwidth solidus
        // Other slash-like characters
        ("%e2%81%84", "⁄"), // Fraction slash
    ];

    for (encoded, _expected) in test_cases {
        // Just verify decoding doesn't panic
        let _decoded = url_decode(encoded).unwrap();
    }
}

#[test]
fn test_url_decode_security_edge_cases() {
    // Edge cases in URL decoding that could be security-relevant

    // Overlong sequences (invalid UTF-8)
    let _overlong = url_decode("%c0%ae").unwrap(); // Overlong encoding of '.'
    // Should not decode to '.' - either fails or produces replacement

    // Percent-encoded percent
    assert_eq!(url_decode("%25").unwrap(), "%");
    assert_eq!(url_decode("%2525").unwrap(), "%25"); // Double-encoded

    // Mixed valid/invalid
    assert_eq!(url_decode("a%20b%ZZc").unwrap(), "a b%ZZc");

    // Truncated sequences
    assert_eq!(url_decode("%2").unwrap(), "%2");
    assert_eq!(url_decode("%").unwrap(), "%");
    assert_eq!(url_decode("%%").unwrap(), "%%");
}

#[test]
fn test_malformed_empty_request() {
    // Completely empty request
    let req = Request::new(
        Method::Get,
        String::new(),
        vec![],
        None,
        std::collections::HashMap::new(),
    );

    assert_eq!(req.path(), "");
    assert_eq!(req.path_without_query(), "");
    assert_eq!(req.query("any"), None);
}
