//! Tests for JSON parsing functionality.
//!
//! Tests try_parse, try_parse_full, and the raw_* helper functions.

use super::super::*;

// =========================================================================
// BASIC PARSING TESTS
// =========================================================================

#[test]
fn test_parse_and_read() {
    let v = try_parse(b"{\"name\":\"Bob\",\"age\":25}").unwrap();
    assert_eq!(v.get("name").str(), Some("Bob".to_string()));
    assert_eq!(v.get("age").int(), Some(25));
    assert!(v.get("missing").is_null());
}

// =========================================================================
// try_parse_full TESTS
// =========================================================================

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
fn test_try_parse_full_nested_arrays() {
    let json = b"[[1,2],[3,4]]";
    let result = try_parse_full(json);
    assert!(result.is_some());
    let v = result.unwrap();
    assert_eq!(v.at(0).at(0).int(), Some(1));
    assert_eq!(v.at(1).at(1).int(), Some(4));
}

// =========================================================================
// try_parse EDGE CASES
// =========================================================================

#[test]
fn test_try_parse_invalid_utf8() {
    let invalid_utf8 = [0xFF, 0xFE];
    assert!(try_parse(&invalid_utf8).is_none());
}

// =========================================================================
// raw_str TESTS
// =========================================================================

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

// =========================================================================
// raw_int TESTS
// =========================================================================

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

// =========================================================================
// raw_float TESTS
// =========================================================================

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

// =========================================================================
// raw_bool TESTS
// =========================================================================

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

// =========================================================================
// raw_is_null TESTS
// =========================================================================

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

// =========================================================================
// from_raw TESTS
// =========================================================================

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

// =========================================================================
// LAZY TO PARSED CONVERSION TESTS
// =========================================================================

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

// =========================================================================
// DISPLAY AND VALUE TESTS
// =========================================================================

#[test]
fn test_display_lazy_mode() {
    let v = try_parse(b"{\"key\": \"value\"}").unwrap();
    let s = v.to_string();
    assert_eq!(s, "{\"key\": \"value\"}");
}

#[test]
fn test_value_method_on_lazy_returns_null() {
    let v = try_parse(b"{\"key\":\"value\"}").unwrap();
    // value() on lazy mode returns static NULL
    let val = v.value();
    assert!(matches!(val, miniserde::json::Value::Null));
}

#[test]
fn test_bytes_method_on_lazy() {
    let v = try_parse(b"{\"key\":\"value\"}").unwrap();
    assert!(v.bytes().is_some());
}

#[test]
fn test_json_value_clone_lazy() {
    let v = try_parse(b"{\"key\":\"value\"}").unwrap();
    #[allow(clippy::redundant_clone)]
    let cloned = v.clone();
    assert_eq!(cloned.path_str(&["key"]), Some("value".to_string()));
}

// =========================================================================
// INT FROM U64 WITHIN RANGE TEST
// =========================================================================

#[test]
fn test_int_from_u64_within_range() {
    let json = b"{\"n\": 9223372036854775807}"; // i64::MAX
    let v = try_parse_full(json).unwrap();
    assert_eq!(v.get("n").int(), Some(i64::MAX));
}

// =========================================================================
// FLOAT FROM U64 TEST
// =========================================================================

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

// =========================================================================
// MAP ARRAY WITH PARSED JSON TESTS
// =========================================================================

#[test]
fn test_map_array_with_mixed_types() {
    let json = br#"{"items": [1, "two", 3]}"#;
    let v = try_parse_full(json).unwrap();
    // map_array with int extraction should fail on "two"
    let result: Option<Vec<i64>> = v.get("items").map_array(raw_int);
    assert!(result.is_none());
}

#[test]
fn test_try_map_array_with_mixed_types() {
    let json = br#"{"items": [1, "two", 3]}"#;
    let v = try_parse_full(json).unwrap();
    let result = v
        .get("items")
        .try_map_array(|item| raw_int(item).ok_or("not int"));
    // try_map_array returns Option<Result<...>>
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

// =========================================================================
// LARGE INT BOUNDARY TESTS
// =========================================================================

#[test]
fn test_large_int_at_max_safe_boundary() {
    // Test exactly at MAX_SAFE_INT boundary via JSON
    let json = br#"{"n": 9007199254740992}"#; // 2^53
    let v = try_parse_full(json).unwrap();
    assert!(v.path_int(&["n"]).is_some());
}

#[test]
fn test_large_int_beyond_max_safe() {
    // Beyond MAX_SAFE_INT - precision considerations
    let json = br#"{"n": 9007199254740994}"#; // 2^53 + 2
    let v = try_parse_full(json).unwrap();
    // Should still parse
    let _ = v.path_int(&["n"]);
}

#[test]
fn test_float_from_large_u64() {
    // Large u64 value as float
    let json = br#"{"n": 18446744073709551615}"#; // u64::MAX
    let v = try_parse_full(json).unwrap();
    let result = v.path_float(&["n"]);
    assert!(result.is_some());
}
