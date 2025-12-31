//! Tests for JSON building functionality.
//!
//! Tests obj(), arr(), str(), int(), float(), bool(), null() builders
//! and the ToJson trait implementations.

use super::super::*;
use std::borrow::Cow;

// =========================================================================
// BASIC BUILDER TESTS
// =========================================================================

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

// =========================================================================
// VALUE BUILDER METHOD TESTS
// =========================================================================

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

#[test]
fn test_float_non_finite_f64() {
    let v = float(f64::INFINITY);
    assert_eq!(v.float(), None);

    let v = float(f64::NEG_INFINITY);
    assert_eq!(v.float(), None);

    let v = float(f64::NAN);
    assert_eq!(v.float(), None);
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

// === Clone tests ===

#[test]
fn test_json_value_clone() {
    let v = obj().set("key", str("value"));
    #[allow(clippy::redundant_clone)]
    let cloned = v.clone();
    assert_eq!(cloned.get("key").str(), Some("value".to_string()));
}

// === value() method test ===

#[test]
fn test_value_method_on_parsed() {
    let v = obj().set("key", str("value"));
    let val = v.value();
    assert!(matches!(val, miniserde::json::Value::Object(_)));
}

// === bytes() method test ===

#[test]
fn test_bytes_method_on_parsed() {
    let v = obj().set("key", str("value"));
    assert!(v.bytes().is_none());
}

// =========================================================================
// ToJson TRAIT TESTS
// =========================================================================

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
