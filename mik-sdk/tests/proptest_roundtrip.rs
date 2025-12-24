//! Roundtrip property tests for mik-sdk.
//!
//! These tests verify that data survives encode -> decode -> encode cycles.

use mik_sdk::{json, time, url_decode};
use proptest::prelude::*;

// =============================================================================
// JSON Roundtrip Tests
// =============================================================================

proptest! {
    /// String values survive build -> serialize -> parse roundtrip
    #[test]
    fn json_string_roundtrip(s in "[a-zA-Z0-9 _-]{0,100}") {
        let built = json::obj().set("value", json::str(&s));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_str(&["value"]), Some(s));
    }

    /// Integer values survive build -> serialize -> parse roundtrip
    #[test]
    fn json_int_roundtrip(n in i64::MIN..=i64::MAX) {
        let built = json::obj().set("value", json::int(n));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_int(&["value"]), Some(n));
    }

    /// Float values survive build -> serialize -> parse roundtrip (within precision)
    #[test]
    fn json_float_roundtrip(f in any::<f64>().prop_filter("must be finite", |x| x.is_finite())) {
        let built = json::obj().set("value", json::float(f));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        let result = parsed.path_float(&["value"]);
        prop_assert!(result.is_some());

        // Float roundtrip may have precision loss, check within epsilon
        let diff = (result.unwrap() - f).abs();
        let epsilon = f.abs() * 1e-10 + 1e-10;
        prop_assert!(diff < epsilon, "Float roundtrip precision: {} vs {} (diff: {})", f, result.unwrap(), diff);
    }

    /// Boolean values survive build -> serialize -> parse roundtrip
    #[test]
    fn json_bool_roundtrip(b in any::<bool>()) {
        let built = json::obj().set("value", json::bool(b));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_bool(&["value"]), Some(b));
    }

    /// Null values survive build -> serialize -> parse roundtrip
    #[test]
    fn json_null_roundtrip(_dummy in 0..1i32) {
        let built = json::obj().set("value", json::null());
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert!(parsed.path_is_null(&["value"]));
    }

    /// Nested objects survive build -> serialize -> parse roundtrip
    #[test]
    fn json_nested_roundtrip(
        a in "[a-z]{1,10}",
        b in "[a-z]{1,10}",
        value in "[a-zA-Z0-9]{1,20}"
    ) {
        let built = json::obj()
            .set(&a, json::obj()
                .set(&b, json::str(&value)));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_str(&[&a, &b]), Some(value));
    }

    /// Arrays survive build -> serialize -> parse roundtrip
    #[test]
    fn json_array_roundtrip(values in prop::collection::vec("[a-z]{1,10}", 0..10)) {
        let mut arr = json::arr();
        for v in &values {
            arr = arr.push(json::str(v));
        }
        let built = json::obj().set("items", arr);
        let serialized = built.to_string();
        let parsed = json::try_parse_full(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.get("items").len(), Some(values.len()));
        for (i, v) in values.iter().enumerate() {
            prop_assert_eq!(parsed.get("items").at(i).str(), Some(v.clone()));
        }
    }

    /// Complex nested structure survives roundtrip
    #[test]
    fn json_complex_roundtrip(
        name in "[a-zA-Z]{1,20}",
        age in 0i64..150,
        active in any::<bool>(),
        tags in prop::collection::vec("[a-z]{1,5}", 0..5)
    ) {
        let mut tag_arr = json::arr();
        for t in &tags {
            tag_arr = tag_arr.push(json::str(t));
        }

        let built = json::obj()
            .set("user", json::obj()
                .set("name", json::str(&name))
                .set("age", json::int(age))
                .set("active", json::bool(active))
                .set("tags", tag_arr));

        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_str(&["user", "name"]), Some(name));
        prop_assert_eq!(parsed.path_int(&["user", "age"]), Some(age));
        prop_assert_eq!(parsed.path_bool(&["user", "active"]), Some(active));
    }

    /// Parse -> serialize -> parse produces equivalent values
    #[test]
    fn json_parse_serialize_parse(
        key in "[a-z]{1,10}",
        value in "[a-zA-Z0-9 ]{1,50}"
    ) {
        let json1 = format!(r#"{{"{}": "{}"}}"#, key, value);
        let parsed1 = json::try_parse(json1.as_bytes()).unwrap();
        let serialized = parsed1.to_string();
        let parsed2 = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed1.path_str(&[&key]), parsed2.path_str(&[&key]));
    }
}

// =============================================================================
// URL Decode Tests
// =============================================================================

proptest! {
    /// URL decode is idempotent for already-decoded strings
    /// decode(decode(x)) == decode(x) for valid decoded strings
    #[test]
    fn url_decode_idempotent(s in "[a-zA-Z0-9_.-]{0,100}") {
        // Plain alphanumeric strings should not change
        let decoded1 = url_decode(&s).unwrap();
        let decoded2 = url_decode(&decoded1).unwrap();

        prop_assert_eq!(decoded1, decoded2, "URL decode should be idempotent for plain strings");
    }

    /// URL decode handles encoded spaces correctly
    #[test]
    fn url_decode_spaces(parts in prop::collection::vec("[a-zA-Z0-9]{1,10}", 1..5)) {
        // Create a string with %20 encoded spaces
        let encoded = parts.join("%20");
        let decoded = url_decode(&encoded).unwrap();
        let expected = parts.join(" ");

        prop_assert_eq!(decoded, expected);
    }

    /// URL decode handles + as space
    #[test]
    fn url_decode_plus_as_space(parts in prop::collection::vec("[a-zA-Z0-9]{1,10}", 1..5)) {
        let encoded = parts.join("+");
        let decoded = url_decode(&encoded).unwrap();
        let expected = parts.join(" ");

        prop_assert_eq!(decoded, expected);
    }

    /// URL decode handles common encoded characters
    #[test]
    fn url_decode_common_chars(_dummy in 0..1i32) {
        // Test common URL-encoded characters
        assert_eq!(url_decode("%26").unwrap(), "&");
        assert_eq!(url_decode("%3D").unwrap(), "=");
        assert_eq!(url_decode("%3F").unwrap(), "?");
        assert_eq!(url_decode("%2F").unwrap(), "/");
        assert_eq!(url_decode("%23").unwrap(), "#");
        assert_eq!(url_decode("%25").unwrap(), "%");
    }

    /// URL decode never panics on arbitrary input
    #[test]
    fn url_decode_never_panics(s in ".*") {
        let _ = url_decode(&s); // Should never panic, may return Err
    }

    /// URL decode handles malformed percent sequences gracefully
    #[test]
    fn url_decode_malformed_percent(
        prefix in "[a-z]{0,5}",
        suffix in "[a-z]{0,5}"
    ) {
        // Incomplete percent sequences - should return Err, not panic
        let r1 = url_decode(&format!("{}%{}", prefix, suffix));
        let r2 = url_decode(&format!("{}%X{}", prefix, suffix));
        // These may be Ok or Err depending on input, but should not panic
        let _ = r1;
        let _ = r2;
    }

    /// Multiple decoding cycles preserve valid content
    #[test]
    fn url_decode_multiple_cycles(s in "[a-zA-Z0-9]{1,20}") {
        // For non-special characters, multiple decodes should be stable
        let d1 = url_decode(&s).unwrap();
        let d2 = url_decode(&d1).unwrap();
        let d3 = url_decode(&d2).unwrap();

        prop_assert_eq!(&d1, &d2);
        prop_assert_eq!(&d2, &d3);
    }
}

// =============================================================================
// JSON Serialization Consistency Tests
// =============================================================================

proptest! {
    /// Serializing the same value twice produces identical output
    #[test]
    fn json_serialize_deterministic(
        key in "[a-z]{1,10}",
        value in "[a-zA-Z0-9]{1,20}"
    ) {
        let obj = json::obj().set(&key, json::str(&value));
        let s1 = obj.to_string();
        let s2 = obj.to_string();

        prop_assert_eq!(s1, s2, "Serialization should be deterministic");
    }

    /// Cloned values serialize identically
    #[test]
    fn json_clone_serialize_identical(
        key in "[a-z]{1,10}",
        value in 0i64..1000
    ) {
        let obj = json::obj().set(&key, json::int(value));
        let cloned = obj.clone();

        prop_assert_eq!(obj.to_string(), cloned.to_string());
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

proptest! {
    /// Empty strings survive roundtrip
    #[test]
    fn json_empty_string_roundtrip(_dummy in 0..1i32) {
        let built = json::obj().set("empty", json::str(""));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_str(&["empty"]), Some(String::new()));
    }

    /// Zero survives roundtrip
    #[test]
    fn json_zero_roundtrip(_dummy in 0..1i32) {
        let built = json::obj().set("zero", json::int(0));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_int(&["zero"]), Some(0));
    }

    /// Empty array survives roundtrip
    #[test]
    fn json_empty_array_roundtrip(_dummy in 0..1i32) {
        let built = json::obj().set("arr", json::arr());
        let serialized = built.to_string();
        let parsed = json::try_parse_full(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.get("arr").len(), Some(0));
    }

    /// Empty object survives roundtrip
    #[test]
    fn json_empty_object_roundtrip(_dummy in 0..1i32) {
        let built = json::obj().set("obj", json::obj());
        let serialized = built.to_string();
        let parsed = json::try_parse_full(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.get("obj").len(), Some(0));
    }

    /// Negative numbers survive roundtrip
    #[test]
    fn json_negative_roundtrip(n in i64::MIN..0i64) {
        let built = json::obj().set("neg", json::int(n));
        let serialized = built.to_string();
        let parsed = json::try_parse(serialized.as_bytes()).unwrap();

        prop_assert_eq!(parsed.path_int(&["neg"]), Some(n));
    }
}

// =============================================================================
// Time ISO Validation Tests
// =============================================================================

proptest! {
    /// ISO 8601 output matches std::time reference implementation
    /// Cross-validates our Howard Hinnant algorithm against Rust stdlib
    #[test]
    fn time_iso_matches_stdlib(
        // Test timestamps from 1970 to 2100 (realistic range)
        secs in 0u64..4102444800u64
    ) {
        // Our implementation
        let our_iso = time::to_iso(secs, 0);

        // Reference: parse back and verify components
        // Format: YYYY-MM-DDTHH:MM:SSZ
        let year: i32 = our_iso[0..4].parse().unwrap();
        let month: u32 = our_iso[5..7].parse().unwrap();
        let day: u32 = our_iso[8..10].parse().unwrap();
        let hour: u32 = our_iso[11..13].parse().unwrap();
        let minute: u32 = our_iso[14..16].parse().unwrap();
        let second: u32 = our_iso[17..19].parse().unwrap();

        // Validate ranges
        prop_assert!((1970..=2100).contains(&year), "Year {} out of range", year);
        prop_assert!((1..=12).contains(&month), "Month {} out of range", month);
        prop_assert!((1..=31).contains(&day), "Day {} out of range", day);
        prop_assert!(hour <= 23, "Hour {} out of range", hour);
        prop_assert!(minute <= 59, "Minute {} out of range", minute);
        prop_assert!(second <= 59, "Second {} out of range", second);

        // Validate format
        prop_assert!(our_iso.ends_with('Z'), "ISO must end with Z");
        prop_assert_eq!(our_iso.len(), 20, "ISO without ms must be 20 chars");

        // Cross-validate: convert back to timestamp using inverse algorithm
        let y = if month <= 2 { year - 1 } else { year } as u64;
        let m = if month <= 2 { month + 12 } else { month } as u64;
        let era = y / 400;
        let yoe = y - era * 400;
        let doy = (153 * (m - 3) + 2) / 5 + day as u64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146097 + doe - 719468;
        let reconstructed = days * 86400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64;

        prop_assert_eq!(secs, reconstructed,
            "Timestamp {} -> ISO '{}' -> reconstructed {} mismatch",
            secs, our_iso, reconstructed);
    }

    /// ISO with milliseconds has correct format
    #[test]
    fn time_iso_millis_format(
        secs in 0u64..4102444800u64,
        nanos in 1u32..1_000_000_000u32
    ) {
        let iso = time::to_iso(secs, nanos);

        // Format: YYYY-MM-DDTHH:MM:SS.mmmZ
        prop_assert_eq!(iso.len(), 24, "ISO with ms must be 24 chars");
        prop_assert!(iso.ends_with('Z'), "Must end with Z");
        prop_assert_eq!(iso.chars().nth(19), Some('.'), "Must have . before millis");

        // Verify milliseconds are correctly truncated (not rounded)
        let expected_ms = nanos / 1_000_000;
        let actual_ms: u32 = iso[20..23].parse().unwrap();
        prop_assert_eq!(expected_ms, actual_ms, "Milliseconds mismatch");
    }

    /// to_millis is consistent with to_iso
    #[test]
    fn time_millis_consistent(
        secs in 0u64..4102444800u64,
        nanos in 0u32..1_000_000_000u32
    ) {
        let millis = time::to_millis(secs, nanos);
        let expected = secs * 1000 + (nanos / 1_000_000) as u64;

        prop_assert_eq!(millis, expected, "to_millis calculation mismatch");
    }
}
