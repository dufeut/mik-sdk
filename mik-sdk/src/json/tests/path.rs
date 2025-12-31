//! Tests for JSON path accessor methods.
//!
//! Tests path_str, path_int, path_float, path_bool, path_exists, path_is_null
//! and the lazy scanner functionality.

use super::super::*;

// =========================================================================
// BASIC PATH ACCESSOR TESTS
// =========================================================================

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

// =========================================================================
// PATH ACCESSORS ON PARSED MODE (tree traversal)
// =========================================================================

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
fn test_path_int_on_parsed_mode_returns_none_for_non_number() {
    let v = obj().set("data", obj().set("name", str("Alice")));
    // path_int on a string should return None
    assert_eq!(v.path_int(&["data", "name"]), None);
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
fn test_path_float_on_parsed_mode_from_u64() {
    // Create a parsed JSON with a U64 value via try_parse_full
    let json = b"{\"data\":{\"n\":9007199254740993}}"; // Slightly above MAX_SAFE_INT
    let v = try_parse_full(json).unwrap();
    let result = v.path_float(&["data", "n"]);
    assert!(result.is_some());
}

#[test]
fn test_path_float_on_parsed_mode_returns_none_for_non_number() {
    let v = obj().set("data", obj().set("name", str("Alice")));
    // path_float on a string should return None
    assert_eq!(v.path_float(&["data", "name"]), None);
}

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

#[test]
fn test_path_float_lazy_mode() {
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

#[test]
fn test_path_int_tree_mode_u64_within_range() {
    // Create a parsed (not lazy) value with number that fits in i64
    let v = obj().set("data", obj().set("n", int(100)));
    assert_eq!(v.path_int(&["data", "n"]), Some(100));
}

#[test]
fn test_path_float_tree_mode_from_int() {
    let v = obj().set("data", obj().set("n", int(42)));
    assert_eq!(v.path_float(&["data", "n"]), Some(42.0));
}

#[test]
fn test_path_through_non_object_intermediate() {
    // Path traversal where intermediate value is not an object
    let json = br#"{"user": "not_an_object"}"#;
    let v = try_parse(json).unwrap();
    assert_eq!(v.path_str(&["user", "name"]), None);
}

#[test]
fn test_path_through_array_intermediate() {
    let json = br#"{"user": [1, 2, 3]}"#;
    let v = try_parse(json).unwrap();
    assert_eq!(v.path_str(&["user", "name"]), None);
}

// =========================================================================
// LAZY SCANNER EDGE CASES
// =========================================================================

#[test]
fn test_lazy_empty_path_on_object() {
    // Empty path should check existence of root value
    let json = br#"{"key": "value"}"#;
    assert!(lazy::path_exists(json, &[]));
}

#[test]
fn test_lazy_empty_path_on_string() {
    let json = br#""hello""#;
    assert!(lazy::path_exists(json, &[]));
}

#[test]
fn test_lazy_empty_path_on_number() {
    let json = br"42";
    assert!(lazy::path_exists(json, &[]));
}

#[test]
fn test_lazy_path_on_array_root() {
    // Root is array, not object - path lookup should return None
    let json = br"[1, 2, 3]";
    assert_eq!(lazy::path_str(json, &["key"]), None);
    assert_eq!(lazy::path_int(json, &["0"]), None);
}

#[test]
fn test_lazy_path_on_string_root() {
    let json = br#""hello""#;
    assert_eq!(lazy::path_str(json, &["key"]), None);
}

#[test]
fn test_lazy_path_on_number_root() {
    let json = br"42";
    assert_eq!(lazy::path_int(json, &["key"]), None);
}

#[test]
fn test_lazy_unterminated_string_value() {
    let json = br#"{"key": "hello"#;
    assert_eq!(lazy::path_str(json, &["key"]), None);
}

#[test]
fn test_lazy_unterminated_string_key() {
    let json = br#"{"key: "value"}"#;
    assert_eq!(lazy::path_str(json, &["key"]), None);
}

#[test]
fn test_lazy_missing_colon() {
    let json = br#"{"key" "value"}"#;
    assert_eq!(lazy::path_str(json, &["key"]), None);
}

#[test]
fn test_lazy_wrong_separator() {
    let json = br#"{"key"; "value"}"#;
    assert_eq!(lazy::path_str(json, &["key"]), None);
}

#[test]
fn test_lazy_invalid_true_literal() {
    let json = br#"{"flag": tru}"#;
    assert_eq!(lazy::path_bool(json, &["flag"]), None);
}

#[test]
fn test_lazy_invalid_false_literal() {
    let json = br#"{"flag": fals}"#;
    assert_eq!(lazy::path_bool(json, &["flag"]), None);
}

#[test]
fn test_lazy_invalid_null_literal() {
    let json = br#"{"val": nul}"#;
    assert!(!lazy::path_is_null(json, &["val"]));
}

#[test]
fn test_lazy_unbalanced_object() {
    // Parser is lenient - finds value even in unbalanced JSON
    let json = br#"{"key": {"nested": 1}"#;
    // The value is found before the unbalanced end is reached
    assert_eq!(lazy::path_int(json, &["key", "nested"]), Some(1));
}

#[test]
fn test_lazy_unbalanced_array() {
    let json = br#"{"arr": [1, 2, 3}"#;
    // Try to access something after the broken array
    assert_eq!(lazy::path_str(json, &["other"]), None);
}

#[test]
fn test_lazy_int_from_float_with_fraction() {
    let json = br#"{"num": 42.5}"#;
    // Should return None because 42.5 has fractional part
    assert_eq!(lazy::path_int(json, &["num"]), None);
}

#[test]
fn test_lazy_int_from_float_whole_number() {
    let json = br#"{"num": 42.0}"#;
    // Should succeed because 42.0 has no fractional part
    assert_eq!(lazy::path_int(json, &["num"]), Some(42));
}

// =========================================================================
// ESCAPE SEQUENCE TESTS (lazy.rs unescape_string)
// =========================================================================

#[test]
fn test_unescape_backspace() {
    let json = br#"{"msg": "hello\bworld"}"#;
    let result = lazy::path_str(json, &["msg"]);
    assert_eq!(result, Some("hello\x08world".to_string()));
}

#[test]
fn test_unescape_formfeed() {
    let json = br#"{"msg": "hello\fworld"}"#;
    let result = lazy::path_str(json, &["msg"]);
    assert_eq!(result, Some("hello\x0Cworld".to_string()));
}

#[test]
fn test_unescape_unicode_valid() {
    let json = br#"{"msg": "\u0041\u0042\u0043"}"#;
    let result = lazy::path_str(json, &["msg"]);
    assert_eq!(result, Some("ABC".to_string()));
}

#[test]
fn test_unescape_unicode_invalid_hex() {
    let json = br#"{"msg": "\uXXXX"}"#;
    let result = lazy::path_str(json, &["msg"]);
    assert!(result.is_none());
}

#[test]
fn test_unescape_unicode_incomplete() {
    // Incomplete unicode at end of string - parser behavior varies
    let json = br#"{"msg": "\u00"}"#;
    let result = lazy::path_str(json, &["msg"]);
    // Parser handles this gracefully (may succeed with partial)
    // Just verify it doesn't panic
    let _ = result;
}

// =========================================================================
// SET/PUSH ON LAZY TRIGGERS PARSE TESTS
// =========================================================================

#[test]
fn test_set_on_lazy_triggers_parse() {
    let json = br#"{"existing": "value"}"#;
    let v = try_parse(json).unwrap();
    // set() triggers get_parsed_mut
    let v2 = v.set("new_key", int(42));
    assert_eq!(v2.get("new_key").int(), Some(42));
    assert_eq!(v2.get("existing").str(), Some("value".to_string()));
}

#[test]
fn test_push_on_lazy_array_triggers_parse() {
    let json = br"[1, 2, 3]";
    let v = try_parse(json).unwrap();
    let v2 = v.push(int(4));
    assert_eq!(v2.len(), Some(4));
}

// =========================================================================
// DISPLAY IMPLEMENTATION EDGE CASES
// =========================================================================

#[test]
fn test_display_lazy_mode_coverage() {
    let json = br#"{"key": "value"}"#;
    let v = try_parse(json).unwrap();
    let display = format!("{v}");
    assert!(display.contains("key"));
    assert!(display.contains("value"));
}

#[test]
fn test_display_parsed_mode_coverage() {
    let v = obj().set("key", str("value"));
    let display = format!("{v}");
    assert!(display.contains("key"));
    assert!(display.contains("value"));
}
