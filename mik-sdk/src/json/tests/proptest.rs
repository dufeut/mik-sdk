//! Property-based fuzzing tests using proptest.
//!
//! These tests ensure the JSON parser doesn't panic on arbitrary/malformed input
//! and handles edge cases gracefully.

use super::super::*;
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
        let json = format!(r#"{{"value": "{s}"}}"#);
        let result = try_parse(json.as_bytes());
        // Valid JSON should parse successfully
        prop_assert!(result.is_some());
        let value = result.unwrap();
        prop_assert_eq!(value.path_str(&["value"]), Some(s));
    }

    /// Test numeric edge cases - very large integers.
    #[test]
    fn parse_handles_large_integers(n in i64::MIN..=i64::MAX) {
        let json = format!(r#"{{"n": {n}}}"#);
        let result = try_parse(json.as_bytes());
        // Should parse without panic
        prop_assert!(result.is_some());
    }

    /// Test numeric edge cases - very large unsigned integers.
    #[test]
    fn parse_handles_large_unsigned(n in 0u64..=u64::MAX) {
        let json = format!(r#"{{"n": {n}}}"#);
        let result = try_parse(json.as_bytes());
        // Should parse without panic
        prop_assert!(result.is_some());
    }

    /// Test numeric edge cases - floating point numbers.
    #[test]
    fn parse_handles_floats(f in any::<f64>().prop_filter("must be finite", |x| x.is_finite())) {
        let json = format!(r#"{{"n": {f}}}"#);
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
        let json_raw = format!(r#"{{"n": {s}}}"#);
        let _ = try_parse(json_raw.as_bytes()); // Should not panic

        // As string value (valid JSON)
        let json_str = format!(r#"{{"n": "{s}"}}"#);
        let result = try_parse(json_str.as_bytes());
        prop_assert!(result.is_some());
    }

    /// Test that scientific notation is handled.
    #[test]
    fn parse_handles_scientific_notation(
        mantissa in -1000i64..1000i64,
        exponent in -308i32..308i32
    ) {
        let json = format!(r#"{{"n": {mantissa}e{exponent}}}"#);
        let _ = try_parse(json.as_bytes()); // Should not panic
    }

    /// Test that very long strings don't cause issues.
    #[test]
    fn parse_handles_long_strings(len in 0usize..10000) {
        let long_string = "x".repeat(len);
        let json = format!(r#"{{"s": "{long_string}"}}"#);
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
        let entries: Vec<String> = (0..len).map(|i| format!(r#""k{i}": {i}"#)).collect();
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
        let json = format!(r#"{{"key": "{prefix}{braces}{suffix}"}}"#);
        let result = try_parse(json.as_bytes());
        // Valid JSON with braces in strings should parse (depth = 1)
        prop_assert!(result.is_some());
    }

    /// Test that escape sequences in strings are handled.
    #[test]
    fn parse_handles_escape_sequences(s in prop::sample::select(vec![
        r#"\""#, r"\\", r"\/", r"\b", r"\f", r"\n", r"\r", r"\t"
    ])) {
        let json = format!(r#"{{"s": "{s}"}}"#);
        let result = try_parse(json.as_bytes());
        prop_assert!(result.is_some());
    }

    /// Test Unicode escape sequences.
    #[test]
    fn parse_handles_unicode_escapes(code in 0u16..0xFFFF) {
        // Skip surrogate pairs as they're invalid in JSON
        if !(0xD800..=0xDFFF).contains(&code) {
            let json = format!(r#"{{"s": "\\u{code:04X}"}}"#);
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

// =========================================================================
// ToJson trait property tests
// =========================================================================

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
        prop_assert_eq!(parsed, i64::from(n));
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
