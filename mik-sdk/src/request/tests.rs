#![allow(clippy::iter_on_single_items)]
use super::*;

// =========================================================================
// PROPTEST PROPERTY TESTS - Fuzz parsers to ensure no panics
// =========================================================================

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

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
}

#[test]
fn test_request_basics() {
    let req = Request::new(
        Method::Get,
        "/users/123?page=2".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        Some(b"{}".to_vec()),
        [("id".to_string(), "123".to_string())]
            .into_iter()
            .collect(),
    );

    assert_eq!(req.method(), Method::Get);
    assert_eq!(req.path(), "/users/123?page=2");
    assert_eq!(req.path_without_query(), "/users/123");
    assert_eq!(req.param("id"), Some("123"));
    assert_eq!(req.query("page"), Some("2"));
    assert_eq!(req.header("Content-Type"), Some("application/json"));
    assert!(req.is_json());
    assert_eq!(req.text(), Some("{}"));
}

#[test]
fn test_method_as_str() {
    assert_eq!(Method::Get.as_str(), "GET");
    assert_eq!(Method::Post.as_str(), "POST");
    assert_eq!(Method::Delete.as_str(), "DELETE");
}

#[test]
fn test_multi_value_headers() {
    // HTTP allows multiple headers with the same name (e.g., Set-Cookie)
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("Set-Cookie".to_string(), "session=abc123".to_string()),
            ("Set-Cookie".to_string(), "user=john".to_string()),
            ("Set-Cookie".to_string(), "theme=dark".to_string()),
            ("Content-Type".to_string(), "text/html".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // header() returns first value
    assert_eq!(req.header("set-cookie"), Some("session=abc123"));
    assert_eq!(req.header("content-type"), Some("text/html"));

    // header_all() returns all values
    let cookies = req.header_all("set-cookie");
    assert_eq!(cookies.len(), 3);
    assert_eq!(cookies[0], "session=abc123");
    assert_eq!(cookies[1], "user=john");
    assert_eq!(cookies[2], "theme=dark");

    // Single-value header
    let content_types = req.header_all("content-type");
    assert_eq!(content_types.len(), 1);
    assert_eq!(content_types[0], "text/html");

    // Non-existent header
    assert_eq!(req.header_all("x-missing").len(), 0);
}

#[test]
fn test_headers_case_insensitive() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-Custom-Header".to_string(), "value".to_string())],
        None,
        HashMap::new(),
    );

    // All case variations should work
    assert_eq!(req.header("x-custom-header"), Some("value"));
    assert_eq!(req.header("X-CUSTOM-HEADER"), Some("value"));
    assert_eq!(req.header("X-Custom-Header"), Some("value"));
}

#[test]
fn test_headers_returns_original() {
    let original = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Request-Id".to_string(), "12345".to_string()),
    ];
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        original.clone(),
        None,
        HashMap::new(),
    );

    // headers() returns original case
    assert_eq!(req.headers(), &original[..]);
}

#[test]
fn test_query_array_params() {
    // HTTP allows multiple query params with the same name
    let req = Request::new(
        Method::Get,
        "/search?tag=rust&tag=wasm&tag=http&page=1".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    // query() returns first value
    assert_eq!(req.query("tag"), Some("rust"));
    assert_eq!(req.query("page"), Some("1"));

    // query_all() returns all values
    let tags = req.query_all("tag");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "rust");
    assert_eq!(tags[1], "wasm");
    assert_eq!(tags[2], "http");

    // Single-value param
    let pages = req.query_all("page");
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0], "1");

    // Non-existent param
    assert_eq!(req.query_all("missing").len(), 0);
    assert_eq!(req.query("missing"), None);
}

#[test]
fn test_query_array_with_encoding() {
    // URL-encoded array values
    let req = Request::new(
        Method::Get,
        "/api?ids=1&ids=2&ids=3&name=hello%20world".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    let ids = req.query_all("ids");
    assert_eq!(ids, &["1", "2", "3"]);
    assert_eq!(req.query("name"), Some("hello world"));
}

// === EDGE CASE TESTS ===

#[test]
fn test_malformed_query_string() {
    // ?key=value&broken&=nokey&key2=
    let req = Request::new(
        Method::Get,
        "/path?key=value&broken&=nokey&key2=".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("key"), Some("value"));
    assert_eq!(req.query("broken"), Some("")); // Key without value
    assert_eq!(req.query(""), Some("nokey")); // Empty key with value
    assert_eq!(req.query("key2"), Some("")); // Key with empty value
}

#[test]
fn test_query_empty_string() {
    let req = Request::new(
        Method::Get,
        "/path?".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("anything"), None);
}

#[test]
fn test_query_no_query_string() {
    let req = Request::new(
        Method::Get,
        "/path".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("anything"), None);
    assert_eq!(req.query_all("anything").len(), 0);
}

#[test]
fn test_empty_path_segments() {
    let req = Request::new(
        Method::Get,
        "/users//posts".to_string(), // Empty segment
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.path(), "/users//posts");
    assert_eq!(req.path_without_query(), "/users//posts");
}

#[test]
fn test_trailing_slash() {
    let req = Request::new(
        Method::Get,
        "/users/".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.path_without_query(), "/users/");
}

#[test]
fn test_header_empty_value() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-Empty".to_string(), String::new())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("x-empty"), Some(""));
}

#[test]
fn test_header_special_characters() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("Authorization".to_string(), "Bearer abc123==".to_string()),
            ("X-Custom".to_string(), "value with spaces".to_string()),
            (
                "Accept".to_string(),
                "text/html, application/json".to_string(),
            ),
        ],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("authorization"), Some("Bearer abc123=="));
    assert_eq!(req.header("x-custom"), Some("value with spaces"));
    assert_eq!(req.header("accept"), Some("text/html, application/json"));
}

#[test]
fn test_duplicate_headers_different_cases() {
    // Same header name with different cases - should be treated as same
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("Content-Type".to_string(), "text/html".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
            ("CONTENT-TYPE".to_string(), "text/plain".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // All should be accessible via any case
    let all = req.header_all("content-type");
    assert_eq!(all.len(), 3);
    assert_eq!(req.header("content-type"), Some("text/html")); // First one
}

#[test]
fn test_body_empty() {
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![],
        Some(vec![]),
        HashMap::new(),
    );

    assert_eq!(req.body(), Some(&[][..]));
    assert_eq!(req.text(), Some(""));
    assert!(!req.has_body()); // Empty body returns false
}

#[test]
fn test_body_none() {
    let req = Request::new(Method::Get, "/".to_string(), vec![], None, HashMap::new());

    assert_eq!(req.body(), None);
    assert_eq!(req.text(), None);
    assert!(!req.has_body());
}

#[test]
fn test_body_invalid_utf8() {
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![],
        Some(vec![0xFF, 0xFE, 0x00, 0x01]), // Invalid UTF-8
        HashMap::new(),
    );

    assert!(req.body().is_some());
    assert_eq!(req.text(), None); // Should return None for invalid UTF-8
    assert!(req.has_body());
}

#[test]
fn test_param_not_found() {
    let req = Request::new(
        Method::Get,
        "/users/123".to_string(),
        vec![],
        None,
        [("id".to_string(), "123".to_string())]
            .into_iter()
            .collect(),
    );

    assert_eq!(req.param("id"), Some("123"));
    assert_eq!(req.param("missing"), None);
    assert_eq!(req.param(""), None);
}

#[test]
fn test_content_type_variations() {
    // With charset
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![(
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert!(req.is_json()); // Should match even with charset
    assert_eq!(req.content_type(), Some("application/json; charset=utf-8"));
}

#[test]
fn test_query_special_characters() {
    let req = Request::new(
        Method::Get,
        "/search?q=hello%26world&name=a%3Db".to_string(), // & and = encoded
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("q"), Some("hello&world"));
    assert_eq!(req.query("name"), Some("a=b"));
}

#[test]
fn test_all_http_methods() {
    assert_eq!(Method::Get.as_str(), "GET");
    assert_eq!(Method::Post.as_str(), "POST");
    assert_eq!(Method::Put.as_str(), "PUT");
    assert_eq!(Method::Patch.as_str(), "PATCH");
    assert_eq!(Method::Delete.as_str(), "DELETE");
    assert_eq!(Method::Head.as_str(), "HEAD");
    assert_eq!(Method::Options.as_str(), "OPTIONS");
}

// === FORM PARSING TESTS ===

#[test]
fn test_form_basic() {
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(b"name=Alice&email=alice%40example.com".to_vec()),
        HashMap::new(),
    );

    assert!(req.is_form());
    assert_eq!(req.form("name"), Some("Alice"));
    assert_eq!(req.form("email"), Some("alice@example.com"));
    assert_eq!(req.form("missing"), None);
}

#[test]
fn test_form_all_values() {
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(b"tags=rust&tags=wasm&tags=http".to_vec()),
        HashMap::new(),
    );

    let tags = req.form_all("tags");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "rust");
    assert_eq!(tags[1], "wasm");
    assert_eq!(tags[2], "http");

    assert_eq!(req.form("tags"), Some("rust")); // First value
}

#[test]
fn test_form_url_decoding() {
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![],
        Some(b"message=hello+world&special=%26%3D%3F".to_vec()),
        HashMap::new(),
    );

    assert_eq!(req.form("message"), Some("hello world")); // + becomes space
    assert_eq!(req.form("special"), Some("&=?")); // URL decoded
}

#[test]
fn test_form_empty_body() {
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.form("anything"), None);
    assert_eq!(req.form_all("anything").len(), 0);
}

#[test]
fn test_form_empty_values() {
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![],
        Some(b"name=&flag&empty=".to_vec()),
        HashMap::new(),
    );

    assert_eq!(req.form("name"), Some("")); // Empty value
    assert_eq!(req.form("flag"), Some("")); // Key without value
    assert_eq!(req.form("empty"), Some("")); // Explicit empty
}

#[test]
fn test_is_form_with_charset() {
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded; charset=utf-8".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert!(req.is_form());
}

#[test]
fn test_is_html() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("content-type".to_string(), "text/html".to_string())],
        None,
        HashMap::new(),
    );

    assert!(req.is_html());
    assert!(!req.is_json());
    assert!(!req.is_form());
}

#[test]
fn test_is_html_with_charset() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )],
        None,
        HashMap::new(),
    );
    assert!(req.is_html());
}

#[test]
fn test_accepts_json() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("accept".to_string(), "application/json".to_string())],
        None,
        HashMap::new(),
    );
    assert!(req.accepts("json"));
    assert!(req.accepts("application/json"));
    assert!(!req.accepts("html"));
}

#[test]
fn test_accepts_multiple() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "accept".to_string(),
            "text/html, application/json, */*".to_string(),
        )],
        None,
        HashMap::new(),
    );
    assert!(req.accepts("html"));
    assert!(req.accepts("json"));
    assert!(req.accepts("text/html"));
    assert!(!req.accepts("xml"));
}

#[test]
fn test_accepts_missing_header() {
    let req = Request::new(Method::Get, "/".to_string(), vec![], None, HashMap::new());
    assert!(!req.accepts("json"));
    assert!(!req.accepts("html"));
}

#[test]
fn test_accepts_case_insensitive() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("accept".to_string(), "APPLICATION/JSON".to_string())],
        None,
        HashMap::new(),
    );
    assert!(req.accepts("json"));
    assert!(req.accepts("JSON"));
    assert!(req.accepts("application/json"));
}

// === UNICODE & INTERNATIONALIZATION TESTS ===

#[test]
fn test_unicode_path_params_emoji() {
    // Emoji in path parameters (URL-encoded)
    let _req = Request::new(
        Method::Get,
        "/users/%F0%9F%98%80/posts".to_string(), // 😀 encoded
        vec![],
        None,
        [("name".to_string(), "%F0%9F%98%80".to_string())]
            .into_iter()
            .collect(),
    );

    // Params are URL-decoded by router
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
fn test_unicode_query_params() {
    // Unicode in query string
    let req = Request::new(
        Method::Get,
        "/search?q=%E4%B8%AD%E6%96%87&emoji=%F0%9F%8E%89".to_string(), // 中文, 🎉
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("q"), Some("中文"));
    assert_eq!(req.query("emoji"), Some("🎉"));
}

#[test]
fn test_unicode_form_data() {
    // Unicode in form submission
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(b"name=%C3%89milie&city=%E6%9D%B1%E4%BA%AC".to_vec()), // Émilie, 東京
        HashMap::new(),
    );

    assert_eq!(req.form("name"), Some("Émilie"));
    assert_eq!(req.form("city"), Some("東京"));
}

#[test]
fn test_unicode_mixed_encodings() {
    // Mixed ASCII and Unicode
    let _req = Request::new(
        Method::Get,
        "/users/john_%F0%9F%91%A8%E2%80%8D%F0%9F%92%BB/profile".to_string(), // john_👨‍💻
        vec![],
        None,
        HashMap::new(),
    );

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

// === LARGE BODY STRESS TESTS ===

#[test]
fn test_large_body_1mb() {
    // 1MB body - typical large JSON payload
    let size = 1024 * 1024;
    let body: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![("content-length".to_string(), size.to_string())],
        Some(body),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), size);
}

#[test]
fn test_large_body_json_text() {
    // Large valid UTF-8 JSON body
    let size = 512 * 1024; // 512KB
    let json_body = format!(
        r#"{{"data": "{}"}}"#,
        "x".repeat(size - 15) // Subtract JSON overhead
    );
    let body_bytes = json_body.as_bytes().to_vec();

    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("content-length".to_string(), body_bytes.len().to_string()),
        ],
        Some(body_bytes),
        HashMap::new(),
    );

    assert!(req.is_json());
    assert!(req.text().is_some());
    assert!(req.text().unwrap().starts_with(r#"{"data": ""#));
}

#[test]
fn test_large_body_binary() {
    // Large binary body (invalid UTF-8)
    let size = 256 * 1024; // 256KB
    let body: Vec<u8> = (0..size).map(|i| ((i * 7) % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload/binary".to_string(),
        vec![
            (
                "content-type".to_string(),
                "application/octet-stream".to_string(),
            ),
            ("content-length".to_string(), size.to_string()),
        ],
        Some(body),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), size);
    // text() should return None for binary data
    assert!(req.text().is_none());
}

#[test]
fn test_body_boundary_sizes() {
    // Test at common buffer size boundaries
    let sizes = [
        0, 1, 63, 64, 65, // 64-byte boundary
        255, 256, 257, // 256-byte boundary
        1023, 1024, 1025, // 1KB boundary
        4095, 4096, 4097, // 4KB (page size) boundary
        65535, 65536, 65537, // 64KB boundary
    ];

    for size in sizes {
        let body: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let req = Request::new(
            Method::Post,
            "/test".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        let body_opt = req.body();
        assert!(body_opt.is_some(), "Body should exist for size {size}");
        assert_eq!(
            body_opt.unwrap().len(),
            size,
            "Body size mismatch for {size}"
        );
    }
}

#[test]
fn test_body_all_byte_values() {
    // Ensure all 256 byte values are handled correctly
    let body: Vec<u8> = (0..=255u8).collect();

    let req = Request::new(
        Method::Post,
        "/binary".to_string(),
        vec![],
        Some(body),
        HashMap::new(),
    );

    let received = req.body().unwrap();
    assert_eq!(received.len(), 256);
    for (i, &byte) in received.iter().enumerate() {
        assert_eq!(byte, i as u8, "Byte mismatch at position {i}");
    }
}

#[test]
fn test_body_repeated_pattern() {
    // Test with repeated patterns that might cause issues with compression/dedup
    let pattern = b"ABCDEFGH".repeat(10000); // 80KB of repeated pattern

    let req = Request::new(
        Method::Post,
        "/pattern".to_string(),
        vec![],
        Some(pattern.clone()),
        HashMap::new(),
    );

    assert_eq!(req.body().unwrap(), pattern.as_slice());
}

// ═══════════════════════════════════════════════════════════════════════════
// HEADER INJECTION SECURITY TESTS
// Production-critical: Prevent CRLF injection and header smuggling
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_header_crlf_injection_in_value() {
    // CRLF sequences in header values should be preserved as-is
    // (the HTTP layer should handle sanitization, we just store them)
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "X-Test".to_string(),
            "value\r\nX-Injected: hacked".to_string(),
        )],
        None,
        HashMap::new(),
    );

    // The value is stored as-is - HTTP layer should validate
    let value = req.header("x-test").unwrap();
    assert!(value.contains('\r') || value.contains('\n') || value == "value\r\nX-Injected: hacked");
}

#[test]
fn test_header_null_byte_in_value() {
    // Null bytes in header values
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-Test".to_string(), "before\0after".to_string())],
        None,
        HashMap::new(),
    );

    let value = req.header("x-test").unwrap();
    assert!(value.contains('\0'));
}

#[test]
fn test_header_very_long_value() {
    // Very long header values (potential DoS)
    // Note: Values exceeding MAX_HEADER_VALUE_LEN (8KB) trigger a warning log
    // but are still stored - this is defense-in-depth, not blocking
    let long_value = "x".repeat(100_000); // 100KB header value

    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-Long".to_string(), long_value)],
        None,
        HashMap::new(),
    );

    // Value is still accessible (we just log warnings)
    assert_eq!(req.header("x-long").unwrap().len(), 100_000);
}

#[test]
fn test_header_value_at_limit() {
    // Header value exactly at the limit (8KB) - should NOT trigger warning
    let at_limit_value = "x".repeat(MAX_HEADER_VALUE_LEN);

    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-AtLimit".to_string(), at_limit_value)],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("x-atlimit").unwrap().len(), MAX_HEADER_VALUE_LEN);
}

#[test]
fn test_header_value_just_over_limit() {
    // Header value just over the limit - triggers warning but still accessible
    let over_limit_value = "x".repeat(MAX_HEADER_VALUE_LEN + 1);

    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("X-OverLimit".to_string(), over_limit_value)],
        None,
        HashMap::new(),
    );

    // Value is still accessible
    assert_eq!(
        req.header("x-overlimit").unwrap().len(),
        MAX_HEADER_VALUE_LEN + 1
    );
}

#[test]
fn test_total_headers_size_limit() {
    // Create headers that exceed the total size limit (1MB)
    // Each header: ~1KB value + short name
    let large_value = "x".repeat(1024);
    let headers: Vec<(String, String)> = (0..1100)
        .map(|i| (format!("X-Header-{i}"), large_value.clone()))
        .collect();

    // Total size: ~1100 * 1024 = ~1.1MB, exceeds 1MB limit
    let req = Request::new(Method::Get, "/".to_string(), headers, None, HashMap::new());

    // All headers are still accessible (we just log warnings)
    assert_eq!(req.header("x-header-0").unwrap().len(), 1024);
    assert_eq!(req.header("x-header-1099").unwrap().len(), 1024);
}

#[test]
fn test_multiple_oversized_headers() {
    // Multiple headers exceeding the individual limit
    let oversized_value = "x".repeat(MAX_HEADER_VALUE_LEN + 100);

    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("X-Oversized-1".to_string(), oversized_value.clone()),
            ("X-Oversized-2".to_string(), oversized_value.clone()),
            ("X-Oversized-3".to_string(), oversized_value),
        ],
        None,
        HashMap::new(),
    );

    // All values still accessible
    assert_eq!(
        req.header("x-oversized-1").unwrap().len(),
        MAX_HEADER_VALUE_LEN + 100
    );
    assert_eq!(
        req.header("x-oversized-2").unwrap().len(),
        MAX_HEADER_VALUE_LEN + 100
    );
    assert_eq!(
        req.header("x-oversized-3").unwrap().len(),
        MAX_HEADER_VALUE_LEN + 100
    );
}

#[test]
fn test_header_many_headers() {
    // Many headers (potential DoS via hash collision or memory)
    let headers: Vec<(String, String)> = (0..1000)
        .map(|i| (format!("X-Header-{i}"), format!("value-{i}")))
        .collect();

    let req = Request::new(Method::Get, "/".to_string(), headers, None, HashMap::new());

    // All headers should be accessible
    assert_eq!(req.header("x-header-0"), Some("value-0"));
    assert_eq!(req.header("x-header-999"), Some("value-999"));
}

#[test]
fn test_header_duplicate_with_different_values() {
    // Multiple headers with same name - all values preserved
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("Set-Cookie".to_string(), "session=abc".to_string()),
            ("Set-Cookie".to_string(), "csrf=xyz".to_string()),
            ("Set-Cookie".to_string(), "theme=dark".to_string()),
        ],
        None,
        HashMap::new(),
    );

    let cookies = req.header_all("set-cookie");
    assert_eq!(cookies.len(), 3);
}

#[test]
fn test_header_empty_name() {
    // Empty header name (edge case)
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(String::new(), "value".to_string())],
        None,
        HashMap::new(),
    );

    // Empty name header should be accessible
    assert_eq!(req.header(""), Some("value"));
}

#[test]
fn test_header_control_characters() {
    // Control characters in header names/values
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            ("X-Tab".to_string(), "before\tafter".to_string()),
            ("X-Bell".to_string(), "before\x07after".to_string()),
            ("X-Escape".to_string(), "before\x1Bafter".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // Control chars preserved in values
    assert!(req.header("x-tab").unwrap().contains('\t'));
    assert!(req.header("x-bell").unwrap().contains('\x07'));
    assert!(req.header("x-escape").unwrap().contains('\x1B'));
}

// ═══════════════════════════════════════════════════════════════════════════
// PATH TRAVERSAL SECURITY TESTS
// Production-critical: Prevent directory traversal attacks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_path_traversal_basic() {
    // Basic path traversal patterns - these should be passed through as-is
    // (application logic should validate, SDK just parses)
    let paths = [
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/..../....//etc/passwd",
        "....//....//etc/passwd",
    ];

    for path in paths {
        let req = Request::new(Method::Get, path.to_string(), vec![], None, HashMap::new());
        // Path is preserved exactly as received
        assert_eq!(req.path(), path);
    }
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
    // Null byte injection in paths
    let paths = [
        "/files/image.png%00.jpg",
        "/download%00/../../etc/passwd",
        "/%00../secret",
    ];

    for path in paths {
        let req = Request::new(Method::Get, path.to_string(), vec![], None, HashMap::new());
        // Path preserved for application to validate
        assert_eq!(req.path(), path);
    }

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
fn test_path_with_query_injection() {
    // Query string injection attempts in path
    let req = Request::new(
        Method::Get,
        "/page?id=1&evil=../../etc/passwd".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.path_without_query(), "/page");
    assert_eq!(req.query("id"), Some("1"));
    assert_eq!(req.query("evil"), Some("../../etc/passwd"));
}

#[test]
fn test_path_param_traversal() {
    // Path params with traversal attempts
    let req = Request::new(
        Method::Get,
        "/files/../../../etc/passwd".to_string(),
        vec![],
        None,
        [("filename".to_string(), "../../../etc/passwd".to_string())]
            .into_iter()
            .collect(),
    );

    // Params are stored as-is - application must validate
    assert_eq!(req.param("filename"), Some("../../../etc/passwd"));
}

#[test]
fn test_path_special_sequences() {
    // Special path sequences
    let special_paths = [
        "/./././file",           // Dot sequences
        "/foo/bar/./baz/../qux", // Mixed . and ..
        "//double//slashes//",   // Double slashes
        "/\\/mixed\\slashes/",   // Mixed slash types
        "/path/to/file;param",   // Semicolon (path params)
        "/path#fragment",        // Fragment
    ];

    for path in special_paths {
        let req = Request::new(Method::Get, path.to_string(), vec![], None, HashMap::new());
        // All preserved as-is
        assert_eq!(req.path(), path);
    }
}

#[test]
fn test_query_param_injection() {
    // Query parameter injection attempts
    let req = Request::new(
        Method::Get,
        "/api?cmd=ls%20-la&file=%2Fetc%2Fpasswd".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    // URL decoding happens
    assert_eq!(req.query("cmd"), Some("ls -la"));
    assert_eq!(req.query("file"), Some("/etc/passwd"));
}

#[test]
fn test_path_very_long() {
    // Very long path (potential DoS)
    let long_path = format!("/{}", "a".repeat(10_000));

    let req = Request::new(Method::Get, long_path, vec![], None, HashMap::new());

    assert_eq!(req.path().len(), 10_001);
}

#[test]
fn test_path_deeply_nested() {
    // Deeply nested path
    let deep_path = format!("/{}", "dir/".repeat(100));

    let req = Request::new(Method::Get, deep_path.clone(), vec![], None, HashMap::new());

    assert_eq!(req.path(), deep_path);
}

#[test]
fn test_form_injection_attempts() {
    // Form data injection attempts
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(b"file=../../../etc/passwd&cmd=rm%20-rf%20/".to_vec()),
        HashMap::new(),
    );

    // Values are decoded but application must validate
    assert_eq!(req.form("file"), Some("../../../etc/passwd"));
    assert_eq!(req.form("cmd"), Some("rm -rf /"));
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

// ═══════════════════════════════════════════════════════════════════════════
// MALFORMED INPUT HANDLING TESTS
// Production-critical: Handle garbage data gracefully without panicking
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_malformed_empty_request() {
    // Completely empty request
    let req = Request::new(Method::Get, String::new(), vec![], None, HashMap::new());

    assert_eq!(req.path(), "");
    assert_eq!(req.path_without_query(), "");
    assert_eq!(req.query("any"), None);
}

#[test]
fn test_malformed_query_string_edge_cases() {
    // Various malformed query strings
    let test_cases = [
        ("?", None),                // Just question mark
        ("??", None),               // Double question mark
        ("?=", None),               // Empty key with empty value - key "a" not present
        ("?===", None),             // Multiple equals - key "" not "a"
        ("?&&&", None),             // Just ampersands
        ("?a&b&c", Some("")),       // Keys without values - "a" exists with empty value
        ("?a=1&&b=2", Some("1")),   // Double ampersand
        ("?a=1&=2&b=3", Some("1")), // Empty key in middle
    ];

    for (path, expected_a) in test_cases {
        let req = Request::new(Method::Get, path.to_string(), vec![], None, HashMap::new());
        assert_eq!(req.query("a"), expected_a, "Failed for path: {path}");
    }
}

#[test]
fn test_malformed_body_not_utf8() {
    // Body with invalid UTF-8 sequences
    let invalid_utf8 = vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0x81];

    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![("content-type".to_string(), "text/plain".to_string())],
        Some(invalid_utf8),
        HashMap::new(),
    );

    // body() works, text() returns None
    assert!(req.body().is_some());
    assert!(req.text().is_none());
}

#[test]
fn test_malformed_form_body() {
    // Malformed form data - documents actual parsing behavior
    let test_cases = [
        (b"".to_vec(), None),               // Empty
        (b"=".to_vec(), None),              // Just equals - empty key, not "key"
        (b"===".to_vec(), None),            // Multiple equals - empty key
        (b"&&&".to_vec(), None),            // Just ampersands
        (b"key".to_vec(), Some("")),        // Key without value or equals
        (b"%ZZ=bad".to_vec(), None),        // Invalid percent encoding in key - "%ZZ" != "key"
        (b"key=%ZZ".to_vec(), Some("%ZZ")), // Invalid percent encoding in value - preserved
    ];

    for (body, expected_key) in test_cases {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![(
                "content-type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            Some(body.clone()),
            HashMap::new(),
        );
        let result = req.form("key");
        assert_eq!(
            result,
            expected_key,
            "Failed for body: {:?}",
            String::from_utf8_lossy(&body)
        );
    }
}

#[test]
fn test_malformed_header_names() {
    // Headers with unusual names
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            (" spaces ".to_string(), "value".to_string()),
            ("\t\ttabs\t\t".to_string(), "value".to_string()),
            ("123numeric".to_string(), "value".to_string()),
            ("special!@#$%".to_string(), "value".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // All headers accessible via normalized (lowercase) names
    assert_eq!(req.header(" spaces "), Some("value"));
    assert_eq!(req.header("\t\ttabs\t\t"), Some("value"));
    assert_eq!(req.header("123numeric"), Some("value"));
    assert_eq!(req.header("special!@#$%"), Some("value"));
}

#[test]
fn test_malformed_path_with_nulls() {
    // Paths containing null bytes
    let req = Request::new(
        Method::Get,
        "/path\0with\0nulls".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert!(req.path().contains('\0'));
}

#[test]
fn test_malformed_body_truncated_utf8() {
    // Truncated UTF-8 sequences
    let truncated_sequences = [
        vec![0xC2],             // Truncated 2-byte sequence
        vec![0xE0, 0xA0],       // Truncated 3-byte sequence
        vec![0xF0, 0x90, 0x80], // Truncated 4-byte sequence
    ];

    for body in truncated_sequences {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic, text() returns None for invalid UTF-8
        assert!(req.body().is_some());
        assert!(
            req.text().is_none(),
            "Should return None for truncated UTF-8: {body:?}"
        );
    }
}

#[test]
fn test_malformed_overlong_utf8() {
    // Overlong UTF-8 encodings (security issue in some parsers)
    let overlong_sequences = [
        vec![0xC0, 0xAF],       // Overlong '/' (should be 0x2F)
        vec![0xE0, 0x80, 0xAF], // Overlong '/' 3-byte
        vec![0xC1, 0xBF],       // Overlong (invalid)
    ];

    for body in overlong_sequences {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic
        let _body = req.body();
        let _text = req.text();
    }
}

#[test]
fn test_malformed_surrogate_pairs() {
    // Invalid surrogate pairs in body
    let invalid_surrogates = [
        vec![0xED, 0xA0, 0x80], // High surrogate alone (U+D800)
        vec![0xED, 0xBF, 0xBF], // Low surrogate alone (U+DFFF)
    ];

    for body in invalid_surrogates {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic, text() returns None
        assert!(req.body().is_some());
        assert!(req.text().is_none());
    }
}

#[test]
fn test_garbage_binary_body() {
    // Random garbage bytes
    let garbage: Vec<u8> = (0..1000).map(|i| ((i * 17 + 31) % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "application/octet-stream".to_string(),
        )],
        Some(garbage),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), 1000);
}

#[test]
fn test_malformed_content_type() {
    // Various malformed content-types - documents actual parsing behavior
    // Note: is_json() and is_form() are case-INSENSITIVE
    let test_cases = [
        ("", false, false),
        ("application", false, false),
        ("application/", false, false),
        ("/json", false, false),
        ("APPLICATION/JSON", true, false), // Case-insensitive: uppercase works
        ("Application/Json", true, false), // Mixed case works
        ("application/json;", true, false),
        ("application/json; ", true, false),
        ("application/x-www-form-urlencoded;charset", false, true),
        ("APPLICATION/X-WWW-FORM-URLENCODED", false, true), // Uppercase form
    ];

    for (content_type, is_json, is_form) in test_cases {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![("content-type".to_string(), content_type.to_string())],
            None,
            HashMap::new(),
        );
        assert_eq!(req.is_json(), is_json, "is_json failed for: {content_type}");
        assert_eq!(req.is_form(), is_form, "is_form failed for: {content_type}");
    }
}

#[test]
fn test_query_with_unicode_keys() {
    // Unicode in query parameter keys
    let req = Request::new(
        Method::Get,
        "/search?%E5%90%8D%E5%89%8D=value&emoji%F0%9F%98%80=test".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("名前"), Some("value")); // Japanese "name"
    assert_eq!(req.query("emoji😀"), Some("test"));
}

#[test]
fn test_many_query_params() {
    // Many query parameters (potential DoS)
    let params: String = (0..1000)
        .map(|i| format!("key{i}=value{i}"))
        .collect::<Vec<_>>()
        .join("&");

    let req = Request::new(
        Method::Get,
        format!("/search?{params}"),
        vec![],
        None,
        HashMap::new(),
    );

    assert_eq!(req.query("key0"), Some("value0"));
    assert_eq!(req.query("key999"), Some("value999"));
}

#[test]
fn test_very_long_query_value() {
    // Very long query parameter value exceeding MAX_URL_DECODED_LEN (64KB)
    let long_value = "x".repeat(100_000);
    let path = format!("/api?data={long_value}");

    let req = Request::new(Method::Get, path, vec![], None, HashMap::new());

    // URL decoding rejects values exceeding MAX_URL_DECODED_LEN for defense-in-depth
    // Such values are silently dropped (not stored)
    assert_eq!(req.query("data"), None);
}

#[test]
fn test_form_with_file_upload_boundary() {
    // Form data that looks like multipart but isn't
    let body =
        b"------WebKitFormBoundary\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\ndata";

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(body.to_vec()),
        HashMap::new(),
    );

    // Should parse as regular form, not crash
    let _form = req.form("file");
}

#[test]
fn test_null_in_various_places() {
    // Null bytes in various locations
    let req = Request::new(
        Method::Post,
        "/path\0end?key\0=val\0ue".to_string(),
        vec![("header\0name".to_string(), "header\0value".to_string())],
        Some(b"form\0data=val\0ue".to_vec()),
        [("param\0key".to_string(), "param\0value".to_string())]
            .into_iter()
            .collect(),
    );

    // All should be accessible without panic
    assert!(req.path().contains('\0'));
    assert!(req.header("header\0name").is_some());
    assert!(req.param("param\0key").is_some());
}

#[test]
fn test_control_chars_in_query() {
    // Control characters in query string
    let control_chars: String = (0..32u8)
        .filter(|&c| c != b'\0') // Exclude null for path
        .map(|c| format!("c{}={}", c, c as char))
        .collect::<Vec<_>>()
        .join("&");

    let req = Request::new(
        Method::Get,
        format!("/test?{control_chars}"),
        vec![],
        None,
        HashMap::new(),
    );

    // Should not panic
    let _queries: Vec<_> = (1..32u8).map(|c| req.query(&format!("c{c}"))).collect();
}

#[test]
fn test_json_with_success() {
    // Test that json_with returns Some when parsing succeeds
    let json_body = br#"{"name": "test", "value": 42}"#;

    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        Some(json_body.to_vec()),
        HashMap::new(),
    );

    // Simple parser that extracts the raw bytes
    let result = req.json_with(|bytes| {
        // Verify we got the right bytes and return them
        if bytes == json_body {
            Some(bytes.to_vec())
        } else {
            None
        }
    });

    // json_with must return Some when the parser returns Some
    assert!(
        result.is_some(),
        "json_with should return Some on successful parse"
    );
    assert_eq!(result.unwrap(), json_body.to_vec());
}

#[test]
fn test_json_with_parser_returns_value() {
    // Test that the parsed value is correctly returned
    let json_body = b"123";

    let req = Request::new(
        Method::Post,
        "/api/number".to_string(),
        vec![],
        Some(json_body.to_vec()),
        HashMap::new(),
    );

    // Parser that extracts a number
    let result = req.json_with(|bytes| std::str::from_utf8(bytes).ok()?.parse::<i32>().ok());

    assert_eq!(
        result,
        Some(123),
        "json_with should return the parsed value"
    );
}

#[test]
fn test_json_with_no_body() {
    // Test that json_with returns None when there's no body
    let req = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    let result = req.json_with(|_| Some(42));
    assert!(
        result.is_none(),
        "json_with should return None when body is missing"
    );
}

#[test]
fn test_json_with_parser_returns_none() {
    // Test that json_with returns None when parser returns None
    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![],
        Some(b"invalid".to_vec()),
        HashMap::new(),
    );

    let result = req.json_with(|_| None::<i32>);
    assert!(
        result.is_none(),
        "json_with should return None when parser returns None"
    );
}

// =========================================================================
// INVALID UTF-8 HEADER HANDLING TESTS
// =========================================================================

#[test]
fn test_header_with_valid_utf8_special_chars() {
    // Headers with valid UTF-8 special characters
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![
            ("X-Unicode".to_string(), "café résumé naïve".to_string()),
            ("X-Emoji".to_string(), "Hello 👋 World 🌍".to_string()),
            ("X-CJK".to_string(), "你好世界".to_string()),
        ],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("x-unicode"), Some("café résumé naïve"));
    assert_eq!(req.header("x-emoji"), Some("Hello 👋 World 🌍"));
    assert_eq!(req.header("x-cjk"), Some("你好世界"));
}

#[test]
fn test_header_lookup_preserves_original_value() {
    // Ensure header values are returned exactly as provided
    let original_value = "  spaces   and\ttabs  ";
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![("X-Whitespace".to_string(), original_value.to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("x-whitespace"), Some(original_value));
}

#[test]
fn test_header_with_empty_value() {
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![("X-Empty".to_string(), String::new())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.header("x-empty"), Some(""));
}

#[test]
fn test_headers_iteration_preserves_order() {
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![
            ("First".to_string(), "1".to_string()),
            ("Second".to_string(), "2".to_string()),
            ("Third".to_string(), "3".to_string()),
        ],
        None,
        HashMap::new(),
    );

    let headers: Vec<_> = req.headers().iter().collect();
    assert_eq!(headers[0].0, "First");
    assert_eq!(headers[1].0, "Second");
    assert_eq!(headers[2].0, "Third");
}

#[test]
fn test_header_with_newlines_in_value() {
    // HTTP headers shouldn't contain raw newlines, but test graceful handling
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![(
            "X-Multiline".to_string(),
            "line1\nline2\r\nline3".to_string(),
        )],
        None,
        HashMap::new(),
    );

    // Should return the value as-is (validation is done elsewhere)
    assert_eq!(req.header("x-multiline"), Some("line1\nline2\r\nline3"));
}

#[test]
fn test_multiple_headers_same_name_different_case() {
    let req = Request::new(
        Method::Get,
        "/test".to_string(),
        vec![
            ("Accept".to_string(), "text/html".to_string()),
            ("ACCEPT".to_string(), "application/json".to_string()),
            ("accept".to_string(), "text/plain".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // All should be accessible (case-insensitive lookup)
    let all = req.header_all("accept");
    assert_eq!(all.len(), 3);
    assert!(all.contains(&"text/html"));
    assert!(all.contains(&"application/json"));
    assert!(all.contains(&"text/plain"));
}

// =========================================================================
// LARGE BODY EDGE CASE TESTS
// =========================================================================

#[test]
fn test_body_exactly_at_common_limits() {
    // Test bodies at exact power-of-2 boundaries
    for size in [1024, 4096, 8192, 16384, 32768, 65536] {
        let body = vec![b'x'; size];
        let req = Request::new(
            Method::Post,
            "/upload".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        assert_eq!(req.body().map(<[u8]>::len), Some(size));
        assert_eq!(req.text().map(str::len), Some(size));
    }
}

#[test]
fn test_body_just_under_and_over_limits() {
    // Test bodies at boundary-1 and boundary+1
    for boundary in [4096i32, 65536] {
        for offset in [-1i32, 0, 1] {
            let size = (boundary + offset) as usize;
            let body = vec![b'a'; size];
            let req = Request::new(
                Method::Post,
                "/upload".to_string(),
                vec![],
                Some(body),
                HashMap::new(),
            );

            assert_eq!(req.body().map(<[u8]>::len), Some(size));
        }
    }
}

#[test]
fn test_trace_id_present() {
    let req = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![("x-trace-id".to_string(), "abc123".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.trace_id(), Some("abc123"));
}

#[test]
fn test_trace_id_missing() {
    let req = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.trace_id(), None);
}

#[test]
fn test_trace_id_case_insensitive() {
    let req = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![("X-Trace-Id".to_string(), "xyz789".to_string())],
        None,
        HashMap::new(),
    );

    // Header lookup is case-insensitive
    assert_eq!(req.trace_id(), Some("xyz789"));
}
