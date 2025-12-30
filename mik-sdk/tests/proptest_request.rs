//! Property-based tests for request parsing using proptest.
//!
//! These tests generate random inputs to find edge cases in
//! query parsing and request handling.

use mik_sdk::{Method, Request};
use proptest::prelude::*;
use std::collections::HashMap;

// =============================================================================
// Query String Parsing Property Tests
// =============================================================================

proptest! {
    /// Simple key=value pairs should parse correctly
    #[test]
    fn simple_query_parses(
        key in "[a-zA-Z][a-zA-Z0-9_]{0,20}",
        value in "[a-zA-Z0-9]{0,20}"
    ) {
        let path = format!("/test?{key}={value}");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        let parsed = req.query(&key);
        prop_assert_eq!(
            parsed,
            Some(value.as_str()),
            "Key should have value: {}", key
        );
    }

    /// Multiple query params should all be accessible
    #[test]
    fn multiple_params_accessible(
        k1 in "[a-zA-Z]{1,5}",
        v1 in "[a-zA-Z0-9]{1,5}",
        k2 in "[a-zA-Z]{6,10}",  // Different length to ensure different keys
        v2 in "[a-zA-Z0-9]{1,5}"
    ) {
        let path = format!("/test?{k1}={v1}&{k2}={v2}");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        prop_assert_eq!(req.query(&k1), Some(v1.as_str()));
        prop_assert_eq!(req.query(&k2), Some(v2.as_str()));
    }

    /// Missing keys should return None
    #[test]
    fn missing_key_returns_none(
        existing_key in "[a-zA-Z]{1,5}",
        missing_key in "[a-zA-Z]{6,10}" // Different length to ensure different
    ) {
        let path = format!("/test?{existing_key}=value");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        prop_assert!(req.query(&missing_key).is_none());
    }

    /// URL-encoded query values should decode
    #[test]
    fn encoded_values_decode(key in "[a-zA-Z]{1,10}") {
        let path = format!("/test?{key}=hello%20world");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        prop_assert_eq!(req.query(&key), Some("hello world"));
    }
}

// =============================================================================
// Header Parsing Property Tests
// =============================================================================

proptest! {
    /// Headers should be case-insensitive
    #[test]
    fn headers_case_insensitive(
        name in "[a-zA-Z][a-zA-Z0-9-]{0,20}",
        value in "[a-zA-Z0-9 ]{1,20}"
    ) {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![(name.to_lowercase(), value.clone())],
            None,
            HashMap::new(),
        );

        // Should work with original case
        prop_assert_eq!(req.header(&name.to_lowercase()), Some(value.as_str()));
        // Should work with uppercase
        prop_assert_eq!(req.header(&name.to_uppercase()), Some(value.as_str()));
    }

    /// Multiple header values should be accessible
    #[test]
    fn multiple_header_values(
        name in "[a-zA-Z]{1,10}",
        v1 in "[a-zA-Z0-9]{1,10}",
        v2 in "[a-zA-Z0-9]{1,10}"
    ) {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![
                (name.to_lowercase(), v1),
                (name.to_lowercase(), v2),
            ],
            None,
            HashMap::new(),
        );

        let all = req.header_all(&name);
        prop_assert_eq!(all.len(), 2);
    }
}

// =============================================================================
// Path Parsing Property Tests
// =============================================================================

proptest! {
    /// Query string should be parsed from path
    #[test]
    fn query_parsed_from_path(
        path_segment in "[a-zA-Z][a-zA-Z0-9_]{0,20}",
        query_key in "[a-zA-Z]{1,10}",
        query_val in "[a-zA-Z0-9]{1,10}"
    ) {
        let full_path = format!("/{path_segment}?{query_key}={query_val}");
        let req = Request::new(
            Method::Get,
            full_path,
            vec![],
            None,
            HashMap::new(),
        );

        // path() returns just the path portion (before ?)
        let expected_prefix = format!("/{path_segment}");
        prop_assert!(req.path().starts_with(&expected_prefix));
        prop_assert_eq!(req.query(&query_key), Some(query_val.as_str()));
    }

    /// Paths without query should have no query params
    #[test]
    fn no_query_returns_none(path_segment in "[a-zA-Z][a-zA-Z0-9_]{0,50}") {
        let path = format!("/{path_segment}");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        prop_assert!(req.query("anything").is_none());
    }
}

// =============================================================================
// Body Handling Property Tests
// =============================================================================

proptest! {
    /// Valid UTF-8 body should be accessible as text
    #[test]
    fn utf8_body_as_text(content in "[a-zA-Z0-9 ]{0,1000}") {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(content.as_bytes().to_vec()),
            HashMap::new(),
        );

        prop_assert_eq!(req.text(), Some(content.as_str()));
    }

    /// Binary body should be accessible as bytes
    #[test]
    fn binary_body_accessible(bytes in prop::collection::vec(any::<u8>(), 0..1000)) {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(bytes.clone()),
            HashMap::new(),
        );

        prop_assert_eq!(req.body(), Some(bytes.as_slice()));
    }

    /// Invalid UTF-8 body should return None for text()
    #[test]
    fn invalid_utf8_no_text(
        prefix in prop::collection::vec(any::<u8>(), 0..10),
        invalid in prop_oneof![
            Just(vec![0xFF, 0xFE]),
            Just(vec![0x80]),
            Just(vec![0xC0, 0x80]),
        ],
        suffix in prop::collection::vec(any::<u8>(), 0..10)
    ) {
        let mut bytes = prefix;
        bytes.extend(invalid);
        bytes.extend(suffix);

        // Only test if actually invalid UTF-8
        if std::str::from_utf8(&bytes).is_err() {
            let req = Request::new(
                Method::Post,
                "/".to_string(),
                vec![],
                Some(bytes),
                HashMap::new(),
            );

            prop_assert!(req.text().is_none());
        }
    }
}

// =============================================================================
// Fuzzing-style Random Input Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Random paths should never panic
    #[test]
    fn random_path_no_panic(path in "[[:print:]]{0,100}") {
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        // Just access everything - should not panic
        let _ = req.path();
        let _ = req.query("test");
        let _ = req.query_all("test");
    }

    /// Random headers should never panic
    #[test]
    fn random_headers_no_panic(
        name in "[[:print:]]{0,50}",
        value in "[[:print:]]{0,50}"
    ) {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![(name.clone(), value)],
            None,
            HashMap::new(),
        );

        let _ = req.header(&name);
        let _ = req.header_all(&name);
    }

    /// All HTTP methods should be representable
    #[test]
    fn all_methods_work(
        method in prop_oneof![
            Just(Method::Get),
            Just(Method::Post),
            Just(Method::Put),
            Just(Method::Delete),
            Just(Method::Patch),
            Just(Method::Head),
            Just(Method::Options),
        ]
    ) {
        let req = Request::new(
            method,
            "/".to_string(),
            vec![],
            None,
            HashMap::new(),
        );

        prop_assert!(req.method().as_str() == method.as_str());
    }
}
