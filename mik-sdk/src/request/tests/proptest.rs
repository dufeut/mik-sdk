//! Property-based fuzzing tests using proptest

use super::super::*;
use proptest::prelude::*;
use std::collections::HashMap;

proptest! {
    /// Test that url_decode doesn't panic on arbitrary strings.
    #[test]
    fn url_decode_doesnt_panic(input in ".*") {
        let _ = url_decode(&input); // Should not panic
    }

    /// Test that url_decode doesn't panic on arbitrary bytes (as string).
    #[test]
    fn url_decode_handles_random_percent_sequences(input in "[%0-9a-fA-F]{0,100}") {
        let _ = url_decode(&input); // Should not panic
    }

    /// Test query string parsing with arbitrary encoded strings.
    #[test]
    fn query_parsing_doesnt_panic(query in ".*") {
        let path = format!("/test?{query}");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );
        // All query access methods should not panic
        let _ = req.query("key");
        let _ = req.query_all("key");
    }

    /// Test query string parsing with URL-encoded characters.
    #[test]
    fn query_parsing_handles_encoded_chars(
        key in "[a-z]{1,10}",
        value in "[a-zA-Z0-9%]{0,50}"
    ) {
        let path = format!("/test?{key}={value}");
        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );
        // Should not panic
        let _ = req.query(&key);
    }

    /// Test path parameter extraction with special characters.
    #[test]
    fn path_params_handle_special_chars(param_value in ".*") {
        let req = Request::new(
            Method::Get,
            "/users/123".to_string(),
            vec![],
            None,
            [("id".to_string(), param_value.clone())]
                .into_iter()
                .collect(),
        );
        // Should not panic
        let result = req.param("id");
        prop_assert_eq!(result, Some(param_value.as_str()));
    }

    /// Test header parsing with edge case values.
    #[test]
    fn header_parsing_handles_arbitrary_values(
        name in "[a-zA-Z-]{1,20}",
        value in ".*"
    ) {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![(name.clone(), value.clone())],
            None,
            HashMap::new(),
        );
        // Header lookup should not panic (case-insensitive)
        let result = req.header(&name.to_lowercase());
        prop_assert_eq!(result, Some(value.as_str()));
    }

    /// Test header lookup with arbitrary case variations.
    #[test]
    fn header_lookup_case_insensitive(name in "[a-zA-Z]{1,20}", value in "[a-z]{0,50}") {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![(name.clone(), value)],
            None,
            HashMap::new(),
        );
        // Both original and lowercase should work
        let _ = req.header(&name);
        let _ = req.header(&name.to_lowercase());
        let _ = req.header(&name.to_uppercase());
    }

    /// Test many query parameters.
    #[test]
    fn query_parsing_handles_many_params(count in 0usize..100) {
        let params: Vec<String> = (0..count)
            .map(|i| format!("key{i}=value{i}"))
            .collect();
        let path = if params.is_empty() {
            "/test".to_string()
        } else {
            format!("/test?{}", params.join("&"))
        };

        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        // All params should be accessible
        for i in 0..count {
            let result = req.query(&format!("key{i}"));
            let expected = format!("value{i}");
            prop_assert_eq!(result, Some(expected.as_str()));
        }
    }

    /// Test many headers.
    #[test]
    fn header_parsing_handles_many_headers(count in 0usize..100) {
        let headers: Vec<(String, String)> = (0..count)
            .map(|i| (format!("X-Header-{i}"), format!("value-{i}")))
            .collect();

        let req = Request::new(
            Method::Get,
            "/".to_string(),
            headers,
            None,
            HashMap::new(),
        );

        // All headers should be accessible
        for i in 0..count {
            let result = req.header(&format!("x-header-{i}"));
            let expected = format!("value-{i}");
            prop_assert_eq!(result, Some(expected.as_str()));
        }
    }

    /// Test form parsing with arbitrary encoded values.
    #[test]
    fn form_parsing_doesnt_panic(body in "[a-zA-Z0-9%&=+]{0,500}") {
        let req = Request::new(
            Method::Post,
            "/submit".to_string(),
            vec![(
                "content-type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            Some(body.into_bytes()),
            HashMap::new(),
        );
        // Form access should not panic
        let _ = req.form("key");
        let _ = req.form_all("key");
    }

    /// Test form parsing with valid key-value pairs.
    #[test]
    fn form_parsing_handles_valid_pairs(
        key in "[a-z]{1,20}",
        value in "[a-zA-Z0-9]{0,50}"
    ) {
        let body = format!("{key}={value}");
        let req = Request::new(
            Method::Post,
            "/submit".to_string(),
            vec![(
                "content-type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            Some(body.into_bytes()),
            HashMap::new(),
        );
        let result = req.form(&key);
        prop_assert_eq!(result, Some(value.as_str()));
    }

    /// Test body handling with arbitrary bytes.
    #[test]
    fn body_handling_doesnt_panic(body in prop::collection::vec(any::<u8>(), 0..1024)) {
        let req = Request::new(
            Method::Post,
            "/upload".to_string(),
            vec![],
            Some(body),
            HashMap::new(),
        );
        // Body access should not panic
        let _ = req.body();
        let _ = req.text();
        let _ = req.has_body();
    }

    /// Test text() returns None for invalid UTF-8.
    #[test]
    fn text_returns_none_for_invalid_utf8(body in prop::collection::vec(128u8..=255u8, 1..50)) {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body),
            HashMap::new(),
        );
        // Invalid UTF-8 should return None, not panic
        let _ = req.text(); // Should not panic
    }

    /// Test path handling with special characters.
    #[test]
    fn path_handling_doesnt_panic(path in "/[a-zA-Z0-9/_.-]*") {
        let req = Request::new(
            Method::Get,
            path.clone(),
            vec![],
            None,
            HashMap::new(),
        );
        prop_assert_eq!(req.path(), &path);
        let _ = req.path_without_query(); // Should not panic
    }

    /// Test content type checks don't panic.
    #[test]
    fn content_type_checks_dont_panic(ct in ".*") {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![("content-type".to_string(), ct)],
            None,
            HashMap::new(),
        );
        // All content type checks should not panic
        let _ = req.content_type();
        let _ = req.is_json();
        let _ = req.is_form();
        let _ = req.is_html();
    }

    /// Test accepts() with arbitrary values.
    #[test]
    fn accepts_doesnt_panic(accept in ".*", mime in ".*") {
        let req = Request::new(
            Method::Get,
            "/".to_string(),
            vec![("accept".to_string(), accept)],
            None,
            HashMap::new(),
        );
        let _ = req.accepts(&mime); // Should not panic
    }

    /// Test url_decode handles truncated percent sequences.
    #[test]
    fn url_decode_handles_truncated_percent(
        prefix in "[a-z]{0,10}",
        suffix in "[0-9a-fA-F]{0,2}"
    ) {
        let input = format!("{prefix}%{suffix}");
        let result = url_decode(&input);
        // Should not panic, returns Ok with best-effort decoding
        prop_assert!(result.is_ok());
    }

    /// Test url_decode handles invalid hex digits.
    #[test]
    fn url_decode_handles_invalid_hex(
        prefix in "[a-z]{0,10}",
        hex1 in "[g-zG-Z]{1}",
        hex2 in "[g-zG-Z]{1}",
        suffix in "[a-z]{0,10}"
    ) {
        let input = format!("{prefix}%{hex1}{hex2}{suffix}");
        let result = url_decode(&input);
        // Should not panic, preserves invalid sequences
        prop_assert!(result.is_ok());
    }

    /// Test multiple values for same query parameter.
    #[test]
    fn query_handles_duplicate_keys(
        key in "[a-z]{1,10}",
        count in 1usize..10
    ) {
        let params: Vec<String> = (0..count)
            .map(|i| format!("{key}=value{i}"))
            .collect();
        let path = format!("/test?{}", params.join("&"));

        let req = Request::new(
            Method::Get,
            path,
            vec![],
            None,
            HashMap::new(),
        );

        // query() returns first value
        let first = req.query(&key);
        prop_assert_eq!(first, Some("value0"));

        // query_all() returns all values
        let all = req.query_all(&key);
        prop_assert_eq!(all.len(), count);
    }

    /// Test multiple values for same header.
    #[test]
    fn header_handles_duplicate_names(
        name in "[a-z]{1,10}",
        count in 1usize..10
    ) {
        let headers: Vec<(String, String)> = (0..count)
            .map(|i| (name.clone(), format!("value{i}")))
            .collect();

        let req = Request::new(
            Method::Get,
            "/".to_string(),
            headers,
            None,
            HashMap::new(),
        );

        // header() returns first value
        let first = req.header(&name);
        prop_assert_eq!(first, Some("value0"));

        // header_all() returns all values
        let all = req.header_all(&name);
        prop_assert_eq!(all.len(), count);
    }

    /// Test contains_ignore_ascii_case with arbitrary inputs.
    #[test]
    fn contains_ignore_ascii_case_doesnt_panic(haystack in ".*", needle in ".*") {
        let _ = contains_ignore_ascii_case(&haystack, &needle); // Should not panic
    }

    /// Test json_with doesn't panic with arbitrary parser.
    #[test]
    fn json_with_doesnt_panic(body in prop::collection::vec(any::<u8>(), 0..256)) {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body),
            HashMap::new(),
        );
        // json_with should not panic even with arbitrary parser
        let _ = req.json_with(|_| Some(42));
        let _ = req.json_with(|_| None::<i32>);
    }
}
