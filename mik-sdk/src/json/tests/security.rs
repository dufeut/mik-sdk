//! Security-related tests for the JSON parser.
//!
//! Tests for:
//! - Trailing content rejection (prevents JSON injection attacks)
//! - Depth limits (prevents stack overflow)
//! - Size limits (prevents memory exhaustion)

use super::super::*;
use crate::constants::MAX_JSON_SIZE;

// =========================================================================
// HELPER FUNCTIONS
// =========================================================================

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

// =========================================================================
// DEPTH LIMIT TESTS - OBJECTS
// =========================================================================

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

// =========================================================================
// DEPTH LIMIT TESTS - ARRAYS
// =========================================================================

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

// =========================================================================
// DEPTH LIMIT TESTS - MIXED NESTING
// =========================================================================

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

// =========================================================================
// DEPTH CHECK EDGE CASES
// =========================================================================

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

#[test]
fn test_depth_check_with_escape_in_string() {
    // Escaped backslash followed by quote in string should not affect depth
    let json = br#"{"key": "value\\\"more"}"#;
    assert!(!json_depth_exceeds_limit(json));
}

#[test]
fn test_depth_check_empty_input() {
    assert!(!json_depth_exceeds_limit(&[]));
}

#[test]
fn test_depth_check_no_nesting() {
    let json = br#""just a string""#;
    assert!(!json_depth_exceeds_limit(json));
}

#[test]
fn test_depth_check_closing_without_opening() {
    // Malformed JSON - closing brace without opening
    // saturating_sub should handle this gracefully
    let json = b"}}}";
    assert!(!json_depth_exceeds_limit(json));
}

#[test]
fn test_depth_exactly_at_limit() {
    // Build JSON with exactly MAX_JSON_DEPTH levels
    let mut json = String::new();
    for _ in 0..20 {
        json.push_str("{\"a\":");
    }
    json.push('1');
    for _ in 0..20 {
        json.push('}');
    }
    // Should succeed at exactly the limit
    assert!(try_parse(json.as_bytes()).is_some());
}

#[test]
fn test_depth_one_over_limit() {
    // Build JSON with MAX_JSON_DEPTH + 1 levels
    let mut json = String::new();
    for _ in 0..21 {
        json.push_str("{\"a\":");
    }
    json.push('1');
    for _ in 0..21 {
        json.push('}');
    }
    // Should fail
    assert!(try_parse(json.as_bytes()).is_none());
}

// =========================================================================
// SIZE LIMIT TESTS
// =========================================================================

#[test]
fn test_try_parse_full_exceeds_size_limit() {
    // Create JSON larger than MAX_JSON_SIZE (1MB)
    let large = vec![b'x'; 1_000_001];
    assert!(try_parse_full(&large).is_none());
}

#[test]
fn test_try_parse_exceeds_size_limit() {
    let large = vec![b' '; 1_000_001];
    assert!(try_parse(&large).is_none());
}

#[test]
fn test_json_at_size_limit() {
    // Create JSON just under the limit
    let padding = "a".repeat(MAX_JSON_SIZE - 20);
    let json = format!(r#"{{"x": "{padding}"}}"#);
    if json.len() <= MAX_JSON_SIZE {
        assert!(try_parse(json.as_bytes()).is_some());
    }
}

// =========================================================================
// INVALID UTF-8 TESTS
// =========================================================================

#[test]
fn test_try_parse_full_invalid_utf8() {
    let invalid_utf8 = [0x80, 0x81, 0x82];
    assert!(try_parse_full(&invalid_utf8).is_none());
}

#[test]
fn test_try_parse_full_exceeds_depth_limit() {
    let json = generate_nested_objects(21);
    assert!(try_parse_full(json.as_bytes()).is_none());
}

// =========================================================================
// TRAILING CONTENT VALIDATION TESTS (Security)
// =========================================================================
// These tests verify that JSON with non-whitespace content after the
// valid JSON value is rejected. This prevents JSON injection attacks.

// === try_parse trailing content tests ===

#[test]
fn test_try_parse_rejects_trailing_garbage_object() {
    // Valid JSON followed by garbage should be rejected
    let json = br#"{"key": "value"}garbage"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject JSON with trailing non-whitespace"
    );
}

#[test]
fn test_try_parse_rejects_trailing_garbage_array() {
    let json = br"[1, 2, 3]extra";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject array with trailing content"
    );
}

#[test]
fn test_try_parse_rejects_trailing_garbage_string() {
    let json = br#""hello"world"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject string with trailing content"
    );
}

#[test]
fn test_try_parse_rejects_trailing_garbage_number() {
    let json = br"42garbage";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject number with trailing content"
    );
}

#[test]
fn test_try_parse_rejects_trailing_garbage_boolean() {
    let json = br"truefoo";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject true with trailing content"
    );

    let json = br"falsebar";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject false with trailing content"
    );
}

#[test]
fn test_try_parse_rejects_trailing_garbage_null() {
    let json = br"nullextra";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject null with trailing content"
    );
}

#[test]
fn test_try_parse_accepts_trailing_whitespace_object() {
    // Trailing whitespace should be accepted
    let json = br#"{"key": "value"}   "#;
    assert!(
        try_parse(json).is_some(),
        "try_parse should accept JSON with trailing whitespace"
    );
}

#[test]
fn test_try_parse_accepts_trailing_whitespace_various() {
    // Various whitespace characters: space, tab, newline, carriage return
    let json = b"{\"key\": \"value\"}\n\t\r ";
    assert!(
        try_parse(json).is_some(),
        "try_parse should accept various trailing whitespace"
    );
}

#[test]
fn test_try_parse_accepts_no_trailing_content() {
    let json = br#"{"key": "value"}"#;
    assert!(try_parse(json).is_some());
}

// === try_parse_full trailing content tests ===

#[test]
fn test_try_parse_full_rejects_trailing_garbage_object() {
    let json = br#"{"key": "value"}garbage"#;
    assert!(
        try_parse_full(json).is_none(),
        "try_parse_full should reject JSON with trailing non-whitespace"
    );
}

#[test]
fn test_try_parse_full_rejects_trailing_garbage_array() {
    let json = br"[1, 2, 3]extra";
    assert!(
        try_parse_full(json).is_none(),
        "try_parse_full should reject array with trailing content"
    );
}

#[test]
fn test_try_parse_full_accepts_trailing_whitespace() {
    let json = br#"{"key": "value"}   "#;
    assert!(
        try_parse_full(json).is_some(),
        "try_parse_full should accept JSON with trailing whitespace"
    );
}

// === Edge cases ===

#[test]
fn test_try_parse_rejects_multiple_json_values() {
    // Two valid JSON objects concatenated - should reject
    let json = br#"{"a":1}{"b":2}"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject multiple concatenated JSON values"
    );
}

#[test]
fn test_try_parse_rejects_json_followed_by_json() {
    // Valid JSON followed by another valid JSON (JSONL style) - should reject
    let json = br#"{"key": "value"}
{"key2": "value2"}"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject JSONL (newline-delimited JSON)"
    );
}

#[test]
fn test_try_parse_accepts_leading_whitespace() {
    // Leading whitespace should be accepted
    let json = br#"   {"key": "value"}"#;
    assert!(
        try_parse(json).is_some(),
        "try_parse should accept JSON with leading whitespace"
    );
}

#[test]
fn test_try_parse_accepts_leading_and_trailing_whitespace() {
    let json = br#"   {"key": "value"}   "#;
    assert!(
        try_parse(json).is_some(),
        "try_parse should accept JSON with leading and trailing whitespace"
    );
}

#[test]
fn test_try_parse_rejects_comment_after_json() {
    // JSON doesn't support comments - trailing // should be rejected
    let json = br#"{"key": "value"} // comment"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject JSON followed by comment"
    );
}

#[test]
fn test_try_parse_nested_object_trailing_garbage() {
    let json = br#"{"outer": {"inner": "value"}}garbage"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject nested object with trailing garbage"
    );
}

#[test]
fn test_try_parse_nested_array_trailing_garbage() {
    let json = br"[[1, 2], [3, 4]]extra";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject nested array with trailing garbage"
    );
}

#[test]
fn test_try_parse_string_with_quotes_trailing_garbage() {
    // String containing escaped quotes, followed by garbage
    let json = br#"{"msg": "hello \"world\""}extra"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should handle escaped quotes correctly"
    );
}

#[test]
fn test_try_parse_scientific_notation_trailing_garbage() {
    let json = br#"{"n": 1.23e+10}garbage"#;
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject scientific notation with trailing garbage"
    );
}

#[test]
fn test_try_parse_negative_number_trailing_garbage() {
    let json = br"-42garbage";
    assert!(
        try_parse(json).is_none(),
        "try_parse should reject negative number with trailing garbage"
    );
}
