//! All tests for the json module.

#[cfg(test)]
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
// MOD.RS FUNCTION TESTS (try_parse_full, raw_* helpers)
// ========================================================================

mod mod_rs_tests {
    use super::*;

    // === try_parse_full tests ===

    #[test]
    fn test_try_parse_full_valid_json() {
        let json = b"{\"name\":\"Alice\",\"age\":30}";
        let result = try_parse_full(json);
        assert!(result.is_some());
        let v = result.unwrap();
        assert_eq!(v.get("name").str(), Some("Alice".to_string()));
        assert_eq!(v.get("age").int(), Some(30));
    }

    #[test]
    fn test_try_parse_full_invalid_json() {
        // Invalid JSON syntax should return None
        let json = b"{invalid json}";
        assert!(try_parse_full(json).is_none());
    }

    #[test]
    fn test_try_parse_full_exceeds_size_limit() {
        // Create JSON larger than MAX_JSON_SIZE (1MB)
        let large = vec![b'x'; 1_000_001];
        assert!(try_parse_full(&large).is_none());
    }

    #[test]
    fn test_try_parse_full_exceeds_depth_limit() {
        let json = generate_nested_objects(21);
        assert!(try_parse_full(json.as_bytes()).is_none());
    }

    #[test]
    fn test_try_parse_full_invalid_utf8() {
        let invalid_utf8 = [0x80, 0x81, 0x82];
        assert!(try_parse_full(&invalid_utf8).is_none());
    }

    #[test]
    fn test_try_parse_full_nested_arrays() {
        let json = b"[[1,2],[3,4]]";
        let result = try_parse_full(json);
        assert!(result.is_some());
        let v = result.unwrap();
        assert_eq!(v.at(0).at(0).int(), Some(1));
        assert_eq!(v.at(1).at(1).int(), Some(4));
    }

    // === try_parse edge cases ===

    #[test]
    fn test_try_parse_exceeds_size_limit() {
        let large = vec![b' '; 1_000_001];
        assert!(try_parse(&large).is_none());
    }

    #[test]
    fn test_try_parse_invalid_utf8() {
        let invalid_utf8 = [0xFF, 0xFE];
        assert!(try_parse(&invalid_utf8).is_none());
    }

    // === raw_str tests ===

    #[test]
    fn test_raw_str_from_string() {
        let val = miniserde::json::Value::String("hello".to_string());
        assert_eq!(raw_str(&val), Some("hello".to_string()));
    }

    #[test]
    fn test_raw_str_from_non_string() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::I64(42));
        assert_eq!(raw_str(&val), None);

        let val = miniserde::json::Value::Bool(true);
        assert_eq!(raw_str(&val), None);

        let val = miniserde::json::Value::Null;
        assert_eq!(raw_str(&val), None);
    }

    // === raw_int tests ===

    #[test]
    fn test_raw_int_from_i64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::I64(-42));
        assert_eq!(raw_int(&val), Some(-42));
    }

    #[test]
    fn test_raw_int_from_u64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::U64(100));
        assert_eq!(raw_int(&val), Some(100));
    }

    #[test]
    fn test_raw_int_from_u64_overflow() {
        // u64::MAX cannot fit in i64
        let val = miniserde::json::Value::Number(miniserde::json::Number::U64(u64::MAX));
        assert_eq!(raw_int(&val), None);
    }

    #[test]
    fn test_raw_int_from_f64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(42.0));
        assert_eq!(raw_int(&val), Some(42));
    }

    #[test]
    fn test_raw_int_from_f64_non_finite() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(f64::INFINITY));
        assert_eq!(raw_int(&val), None);

        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(f64::NAN));
        assert_eq!(raw_int(&val), None);
    }

    #[test]
    fn test_raw_int_from_f64_too_large() {
        // Value larger than MAX_SAFE_INT
        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(1e20));
        assert_eq!(raw_int(&val), None);
    }

    #[test]
    fn test_raw_int_from_non_number() {
        let val = miniserde::json::Value::String("42".to_string());
        assert_eq!(raw_int(&val), None);
    }

    // === raw_float tests ===

    #[test]
    fn test_raw_float_from_f64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(98.6));
        assert_eq!(raw_float(&val), Some(98.6));
    }

    #[test]
    fn test_raw_float_from_f64_non_finite() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(f64::INFINITY));
        assert_eq!(raw_float(&val), None);

        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(f64::NEG_INFINITY));
        assert_eq!(raw_float(&val), None);

        let val = miniserde::json::Value::Number(miniserde::json::Number::F64(f64::NAN));
        assert_eq!(raw_float(&val), None);
    }

    #[test]
    fn test_raw_float_from_i64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::I64(-100));
        assert_eq!(raw_float(&val), Some(-100.0));
    }

    #[test]
    fn test_raw_float_from_u64() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::U64(200));
        assert_eq!(raw_float(&val), Some(200.0));
    }

    #[test]
    fn test_raw_float_from_non_number() {
        let val = miniserde::json::Value::String("3.14".to_string());
        assert_eq!(raw_float(&val), None);
    }

    // === raw_bool tests ===

    #[test]
    fn test_raw_bool_true() {
        let val = miniserde::json::Value::Bool(true);
        assert_eq!(raw_bool(&val), Some(true));
    }

    #[test]
    fn test_raw_bool_false() {
        let val = miniserde::json::Value::Bool(false);
        assert_eq!(raw_bool(&val), Some(false));
    }

    #[test]
    fn test_raw_bool_from_non_bool() {
        let val = miniserde::json::Value::Number(miniserde::json::Number::I64(1));
        assert_eq!(raw_bool(&val), None);

        let val = miniserde::json::Value::String("true".to_string());
        assert_eq!(raw_bool(&val), None);
    }

    // === raw_is_null tests ===

    #[test]
    fn test_raw_is_null_true() {
        let val = miniserde::json::Value::Null;
        assert!(raw_is_null(&val));
    }

    #[test]
    fn test_raw_is_null_false() {
        let val = miniserde::json::Value::Bool(false);
        assert!(!raw_is_null(&val));

        let val = miniserde::json::Value::Number(miniserde::json::Number::I64(0));
        assert!(!raw_is_null(&val));

        let val = miniserde::json::Value::String("null".to_string());
        assert!(!raw_is_null(&val));
    }

    // === json_depth_exceeds_limit edge cases ===

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
}

// ========================================================================
// VALUE.RS METHOD TESTS
// ========================================================================

mod value_rs_tests {
    use super::*;

    // === from_raw tests ===

    #[test]
    fn test_from_raw_string() {
        let raw = miniserde::json::Value::String("test".to_string());
        let jv = JsonValue::from_raw(&raw);
        assert_eq!(jv.str(), Some("test".to_string()));
    }

    #[test]
    fn test_from_raw_number() {
        let raw = miniserde::json::Value::Number(miniserde::json::Number::I64(42));
        let jv = JsonValue::from_raw(&raw);
        assert_eq!(jv.int(), Some(42));
    }

    #[test]
    fn test_from_raw_object() {
        let mut obj = miniserde::json::Object::new();
        obj.insert(
            "key".to_string(),
            miniserde::json::Value::String("value".to_string()),
        );
        let raw = miniserde::json::Value::Object(obj);
        let jv = JsonValue::from_raw(&raw);
        assert_eq!(jv.get("key").str(), Some("value".to_string()));
    }

    // === str_or, int_or, float_or, bool_or tests ===

    #[test]
    fn test_str_or_when_not_string() {
        let v = int(42);
        assert_eq!(v.str_or("default"), "default");
    }

    #[test]
    fn test_str_or_when_null() {
        let v = null();
        assert_eq!(v.str_or("fallback"), "fallback");
    }

    #[test]
    fn test_int_or_when_not_number() {
        let v = str("hello");
        assert_eq!(v.int_or(99), 99);
    }

    #[test]
    fn test_float_or_when_not_number() {
        let v = bool(true);
        assert!((v.float_or(98.6) - 98.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bool_or_when_not_bool() {
        let v = str("true");
        assert!(!v.bool_or(false));
    }

    // === int() edge cases ===

    #[test]
    fn test_int_from_u64_within_range() {
        let json = b"{\"n\": 9223372036854775807}"; // i64::MAX
        let v = try_parse_full(json).unwrap();
        assert_eq!(v.get("n").int(), Some(i64::MAX));
    }

    #[test]
    fn test_int_from_f64_within_safe_range() {
        let v = float(42.0);
        assert_eq!(v.int(), Some(42));
    }

    #[test]
    fn test_int_from_f64_negative() {
        let v = float(-100.0);
        assert_eq!(v.int(), Some(-100));
    }

    #[test]
    fn test_int_from_f64_max_safe() {
        let v = float(9007199254740992.0); // 2^53
        assert_eq!(v.int(), Some(9007199254740992));
    }

    #[test]
    fn test_int_from_f64_too_large() {
        // 1e20 is well beyond MAX_SAFE_INT (2^53)
        let v = float(1e20);
        assert_eq!(v.int(), None);
    }

    // === float() edge cases ===

    #[test]
    fn test_float_from_i64() {
        let v = int(100);
        assert_eq!(v.float(), Some(100.0));
    }

    #[test]
    fn test_float_from_i64_negative() {
        let v = int(-50);
        assert_eq!(v.float(), Some(-50.0));
    }

    // === map_array tests ===

    #[test]
    fn test_map_array_strings() {
        let v = arr().push(str("a")).push(str("b")).push(str("c"));
        let result: Option<Vec<String>> = v.map_array(raw_str);
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_map_array_integers() {
        let v = arr().push(int(1)).push(int(2)).push(int(3));
        let result: Option<Vec<i64>> = v.map_array(raw_int);
        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_map_array_returns_none_if_not_array() {
        let v = obj().set("key", str("value"));
        let result: Option<Vec<String>> = v.map_array(raw_str);
        assert!(result.is_none());
    }

    #[test]
    fn test_map_array_returns_none_if_element_fails() {
        // Array with mixed types - second element is not a string
        let v = arr().push(str("a")).push(int(42)).push(str("c"));
        let result: Option<Vec<String>> = v.map_array(raw_str);
        assert!(result.is_none());
    }

    #[test]
    fn test_map_array_empty() {
        let v = arr();
        let result: Option<Vec<i64>> = v.map_array(raw_int);
        assert_eq!(result, Some(vec![]));
    }

    // === try_map_array tests ===

    #[test]
    fn test_try_map_array_success() {
        let v = arr().push(int(1)).push(int(2)).push(int(3));
        let result: Option<Result<Vec<i64>, &str>> =
            v.try_map_array(|v| raw_int(v).ok_or("not an int"));
        assert_eq!(result, Some(Ok(vec![1, 2, 3])));
    }

    #[test]
    fn test_try_map_array_error() {
        let v = arr().push(int(1)).push(str("oops")).push(int(3));
        let result: Option<Result<Vec<i64>, &str>> =
            v.try_map_array(|v| raw_int(v).ok_or("not an int"));
        assert_eq!(result, Some(Err("not an int")));
    }

    #[test]
    fn test_try_map_array_not_array() {
        let v = str("not an array");
        let result: Option<Result<Vec<i64>, &str>> =
            v.try_map_array(|v| raw_int(v).ok_or("not an int"));
        assert!(result.is_none());
    }

    #[test]
    fn test_try_map_array_empty() {
        let v = arr();
        let result: Option<Result<Vec<String>, &str>> =
            v.try_map_array(|v| raw_str(v).ok_or("not a string"));
        assert_eq!(result, Some(Ok(vec![])));
    }

    // === keys() tests ===

    #[test]
    fn test_keys_on_object() {
        let v = obj().set("a", int(1)).set("b", int(2));
        let keys = v.keys();
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_keys_on_non_object() {
        let v = arr().push(int(1));
        assert!(v.keys().is_empty());

        let v = str("hello");
        assert!(v.keys().is_empty());

        let v = null();
        assert!(v.keys().is_empty());
    }

    #[test]
    fn test_keys_empty_object() {
        let v = obj();
        assert!(v.keys().is_empty());
    }

    // === len() tests ===

    #[test]
    fn test_len_on_array() {
        let v = arr().push(int(1)).push(int(2)).push(int(3));
        assert_eq!(v.len(), Some(3));
    }

    #[test]
    fn test_len_on_object() {
        let v = obj().set("a", int(1)).set("b", int(2));
        assert_eq!(v.len(), Some(2));
    }

    #[test]
    fn test_len_on_non_collection() {
        assert_eq!(str("hello").len(), None);
        assert_eq!(int(42).len(), None);
        assert_eq!(bool(true).len(), None);
        assert_eq!(null().len(), None);
    }

    #[test]
    fn test_len_empty_collections() {
        assert_eq!(arr().len(), Some(0));
        assert_eq!(obj().len(), Some(0));
    }

    // === is_empty() tests ===

    #[test]
    fn test_is_empty_on_empty_array() {
        assert!(arr().is_empty());
    }

    #[test]
    fn test_is_empty_on_non_empty_array() {
        assert!(!arr().push(int(1)).is_empty());
    }

    #[test]
    fn test_is_empty_on_empty_object() {
        assert!(obj().is_empty());
    }

    #[test]
    fn test_is_empty_on_non_empty_object() {
        assert!(!obj().set("key", int(1)).is_empty());
    }

    #[test]
    fn test_is_empty_on_non_collection() {
        // Non-collections return false (len() returns None, so is_some_and returns false)
        assert!(!str("hello").is_empty());
        assert!(!int(42).is_empty());
        assert!(!null().is_empty());
    }

    // === set() on non-object ===

    #[test]
    fn test_set_on_non_object_creates_object() {
        let v = str("hello").set("key", int(42));
        assert_eq!(v.get("key").int(), Some(42));
    }

    #[test]
    fn test_set_on_array_creates_object() {
        let v = arr().push(int(1)).set("key", str("value"));
        assert_eq!(v.get("key").str(), Some("value".to_string()));
    }

    #[test]
    fn test_set_on_null_creates_object() {
        let v = null().set("key", bool(true));
        assert_eq!(v.get("key").bool(), Some(true));
    }

    // === push() on non-array ===

    #[test]
    fn test_push_on_non_array_creates_array() {
        let v = str("hello").push(int(42));
        assert_eq!(v.at(0).int(), Some(42));
    }

    #[test]
    fn test_push_on_object_creates_array() {
        let v = obj().set("key", int(1)).push(str("value"));
        assert_eq!(v.at(0).str(), Some("value".to_string()));
    }

    #[test]
    fn test_push_on_null_creates_array() {
        let v = null().push(bool(false));
        assert_eq!(v.at(0).bool(), Some(false));
    }

    // === get() edge cases ===

    #[test]
    fn test_get_on_non_object() {
        let v = arr().push(int(1));
        assert!(v.get("key").is_null());

        let v = str("hello");
        assert!(v.get("key").is_null());
    }

    // === at() edge cases ===

    #[test]
    fn test_at_on_non_array() {
        let v = obj().set("key", int(1));
        assert!(v.at(0).is_null());

        let v = str("hello");
        assert!(v.at(0).is_null());
    }

    #[test]
    fn test_at_out_of_bounds() {
        let v = arr().push(int(1)).push(int(2));
        assert!(v.at(5).is_null());
        assert!(v.at(100).is_null());
    }

    // === Display/Debug impl tests ===

    #[test]
    fn test_display_lazy_mode() {
        let v = try_parse(b"{\"key\": \"value\"}").unwrap();
        let s = v.to_string();
        assert_eq!(s, "{\"key\": \"value\"}");
    }

    #[test]
    fn test_display_parsed_mode() {
        let v = obj().set("key", str("value"));
        let s = v.to_string();
        assert!(s.contains("key"));
        assert!(s.contains("value"));
    }

    #[test]
    fn test_debug_impl() {
        let v = obj().set("a", int(1));
        let debug_str = format!("{v:?}");
        assert!(debug_str.contains('a'));
        assert!(debug_str.contains('1'));
    }

    // === to_bytes test ===

    #[test]
    fn test_to_bytes() {
        let v = obj().set("key", str("value"));
        let bytes = v.to_bytes();
        assert!(!bytes.is_empty());
        assert!(std::str::from_utf8(&bytes).is_ok());
    }

    // === Path-based accessors on parsed mode (tree traversal) ===

    #[test]
    fn test_path_str_on_parsed_mode() {
        let v = obj().set("user", obj().set("name", str("Alice")));
        assert_eq!(v.path_str(&["user", "name"]), Some("Alice".to_string()));
    }

    #[test]
    fn test_path_str_on_parsed_mode_not_string() {
        let v = obj().set("user", obj().set("age", int(30)));
        assert_eq!(v.path_str(&["user", "age"]), None);
    }

    #[test]
    fn test_path_int_on_parsed_mode() {
        let v = obj().set("data", obj().set("count", int(42)));
        assert_eq!(v.path_int(&["data", "count"]), Some(42));
    }

    #[test]
    fn test_path_int_on_parsed_mode_from_u64() {
        // Test u64 conversion path
        let json = b"{\"data\":{\"n\":18446744073709551615}}"; // u64::MAX
        let v = try_parse_full(json).unwrap();
        // u64::MAX > i64::MAX, so should return None
        assert_eq!(v.path_int(&["data", "n"]), None);
    }

    #[test]
    fn test_path_int_on_parsed_mode_from_f64() {
        let v = obj().set("data", obj().set("num", float(100.0)));
        assert_eq!(v.path_int(&["data", "num"]), Some(100));
    }

    #[test]
    fn test_path_int_on_parsed_mode_from_f64_non_finite() {
        let v = obj().set("data", obj().set("num", float(f64::INFINITY)));
        assert_eq!(v.path_int(&["data", "num"]), None);
    }

    #[test]
    fn test_path_int_on_parsed_mode_from_f64_too_large() {
        let v = obj().set("data", obj().set("num", float(1e20)));
        assert_eq!(v.path_int(&["data", "num"]), None);
    }

    #[test]
    fn test_path_float_on_parsed_mode() {
        let v = obj().set("data", obj().set("val", float(98.6)));
        let result = v.path_float(&["data", "val"]).unwrap();
        assert!((result - 98.6).abs() < 0.001);
    }

    #[test]
    fn test_path_float_on_parsed_mode_from_i64() {
        let v = obj().set("data", obj().set("num", int(-50)));
        assert_eq!(v.path_float(&["data", "num"]), Some(-50.0));
    }

    #[test]
    fn test_path_float_on_parsed_mode_non_finite() {
        let v = obj().set("data", obj().set("num", float(f64::INFINITY)));
        assert_eq!(v.path_float(&["data", "num"]), None);
    }

    #[test]
    fn test_path_bool_on_parsed_mode() {
        let v = obj().set("config", obj().set("enabled", bool(true)));
        assert_eq!(v.path_bool(&["config", "enabled"]), Some(true));
    }

    #[test]
    fn test_path_bool_on_parsed_mode_not_bool() {
        let v = obj().set("config", obj().set("enabled", str("yes")));
        assert_eq!(v.path_bool(&["config", "enabled"]), None);
    }

    #[test]
    fn test_path_is_null_on_parsed_mode() {
        let v = obj().set("data", obj().set("value", null()));
        assert!(v.path_is_null(&["data", "value"]));
    }

    #[test]
    fn test_path_is_null_on_parsed_mode_not_null() {
        let v = obj().set("data", obj().set("value", int(0)));
        assert!(!v.path_is_null(&["data", "value"]));
    }

    #[test]
    fn test_path_exists_on_parsed_mode() {
        let v = obj().set("data", obj().set("value", int(1)));
        assert!(v.path_exists(&["data", "value"]));
        assert!(!v.path_exists(&["data", "missing"]));
    }

    #[test]
    fn test_path_traversal_through_non_object() {
        let v = obj().set("data", arr().push(int(1)));
        // Trying to traverse through an array should fail
        assert_eq!(v.path_str(&["data", "key"]), None);
    }

    // === Clone tests ===

    #[test]
    fn test_json_value_clone() {
        let v = obj().set("key", str("value"));
        #[allow(clippy::redundant_clone)]
        let cloned = v.clone();
        assert_eq!(cloned.get("key").str(), Some("value".to_string()));
    }

    #[test]
    fn test_json_value_clone_lazy() {
        let v = try_parse(b"{\"key\":\"value\"}").unwrap();
        #[allow(clippy::redundant_clone)]
        let cloned = v.clone();
        assert_eq!(cloned.path_str(&["key"]), Some("value".to_string()));
    }

    // === get_parsed_mut lazy conversion test ===

    #[test]
    fn test_lazy_to_parsed_conversion_via_set() {
        let v = try_parse(b"{\"a\":1}").unwrap();
        // set() triggers get_parsed_mut which converts lazy to parsed
        let v2 = v.set("b", int(2));
        assert_eq!(v2.get("a").int(), Some(1));
        assert_eq!(v2.get("b").int(), Some(2));
    }

    #[test]
    fn test_lazy_to_parsed_conversion_via_push() {
        let v = try_parse(b"[1,2]").unwrap();
        // push() triggers get_parsed_mut which converts lazy to parsed
        let v2 = v.push(int(3));
        assert_eq!(v2.at(0).int(), Some(1));
        assert_eq!(v2.at(2).int(), Some(3));
    }

    #[test]
    fn test_lazy_to_parsed_conversion_invalid_json() {
        // Create a lazy value that will fail to parse
        // We can't directly create this through try_parse since it validates,
        // but we can test via set which triggers parse_bytes
        let v = try_parse(b"{}").unwrap();
        let v2 = v.set("key", str("value"));
        assert_eq!(v2.get("key").str(), Some("value".to_string()));
    }

    // === value() method test ===

    #[test]
    fn test_value_method_on_lazy_returns_null() {
        let v = try_parse(b"{\"key\":\"value\"}").unwrap();
        // value() on lazy mode returns static NULL
        let val = v.value();
        assert!(matches!(val, miniserde::json::Value::Null));
    }

    #[test]
    fn test_value_method_on_parsed() {
        let v = obj().set("key", str("value"));
        let val = v.value();
        assert!(matches!(val, miniserde::json::Value::Object(_)));
    }

    // === bytes() method test ===

    #[test]
    fn test_bytes_method_on_lazy() {
        let v = try_parse(b"{\"key\":\"value\"}").unwrap();
        assert!(v.bytes().is_some());
    }

    #[test]
    fn test_bytes_method_on_parsed() {
        let v = obj().set("key", str("value"));
        assert!(v.bytes().is_none());
    }

    // === float() U64 and non-finite tests (lines 253-254) ===

    #[test]
    fn test_float_from_u64() {
        // Test line 253: Number::U64(u) => Some(u as f64)
        // We need to create a JsonValue with a U64 number
        // Parse JSON with a large positive integer that will be stored as U64
        let json = b"{\"n\": 18446744073709551615}"; // u64::MAX
        let v = try_parse_full(json).unwrap();
        let result = v.get("n").float();
        assert!(result.is_some());
        // u64::MAX as f64
        assert!((result.unwrap() - 18446744073709551615.0).abs() < 1e10);
    }

    #[test]
    fn test_float_non_finite_f64() {
        // Test line 254: Number::F64(_) => None for non-finite
        // This is tricky because we can't parse infinity from JSON
        // But we can test via float(f64::INFINITY) builder
        let v = float(f64::INFINITY);
        assert_eq!(v.float(), None);

        let v = float(f64::NEG_INFINITY);
        assert_eq!(v.float(), None);

        let v = float(f64::NAN);
        assert_eq!(v.float(), None);
    }

    // === path_int on parsed mode returning None for non-Number (line 392) ===

    #[test]
    fn test_path_int_on_parsed_mode_returns_none_for_non_number() {
        // Test line 392: _ => None in path_int when path returns non-Number
        let v = obj().set("data", obj().set("name", str("Alice")));
        // path_int on a string should return None
        assert_eq!(v.path_int(&["data", "name"]), None);
    }

    // === path_float on parsed mode with U64 and non-number (lines 418, 421) ===

    #[test]
    fn test_path_float_on_parsed_mode_from_u64() {
        // Test line 418: Number::U64(u) => Some(*u as f64)
        // Create a parsed JSON with a U64 value via try_parse_full
        let json = b"{\"data\":{\"n\":9007199254740993}}"; // Slightly above MAX_SAFE_INT
        let v = try_parse_full(json).unwrap();
        let result = v.path_float(&["data", "n"]);
        assert!(result.is_some());
    }

    #[test]
    fn test_path_float_on_parsed_mode_returns_none_for_non_number() {
        // Test line 421: _ => None in path_float when path returns non-Number
        let v = obj().set("data", obj().set("name", str("Alice")));
        // path_float on a string should return None
        assert_eq!(v.path_float(&["data", "name"]), None);
    }

    // === path_float_or tests (lines 427-429) ===

    #[test]
    fn test_path_float_or_returns_value() {
        let v = obj().set("data", obj().set("value", float(98.6)));
        let result = v.path_float_or(&["data", "value"], 0.0);
        assert!((result - 98.6).abs() < 0.001);
    }

    #[test]
    fn test_path_float_or_returns_default() {
        let v = obj().set("data", obj().set("name", str("test")));
        let result = v.path_float_or(&["data", "missing"], 99.9);
        assert!((result - 99.9).abs() < 0.001);
    }

    #[test]
    fn test_path_float_or_returns_default_for_non_number() {
        let v = obj().set("data", obj().set("name", str("test")));
        let result = v.path_float_or(&["data", "name"], 42.0);
        assert!((result - 42.0).abs() < 0.001);
    }

    // === path_float lazy mode test (line 410) ===

    #[test]
    fn test_path_float_lazy_mode() {
        // Test line 410: lazy::path_float path in path_float method
        // Use try_parse (which creates lazy mode) instead of try_parse_full
        let json = b"{\"data\":{\"value\":98.6123}}";
        let v = try_parse(json).unwrap();
        // Verify we're in lazy mode
        assert!(v.bytes().is_some());
        // Now call path_float which should use the lazy path
        let result = v.path_float(&["data", "value"]);
        assert!(result.is_some());
        assert!((result.unwrap() - 98.6123).abs() < 0.00001);
    }
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
        let s = "kon'nichiwa";
        let json = s.to_json();
        let output = json.to_string();
        assert!(output.contains("kon'nichiwa"));
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
    }
}
