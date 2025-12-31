//! Header access and case-insensitivity tests

use super::super::*;
use std::collections::HashMap;

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
fn test_all_http_methods() {
    assert_eq!(Method::Get.as_str(), "GET");
    assert_eq!(Method::Post.as_str(), "POST");
    assert_eq!(Method::Put.as_str(), "PUT");
    assert_eq!(Method::Patch.as_str(), "PATCH");
    assert_eq!(Method::Delete.as_str(), "DELETE");
    assert_eq!(Method::Head.as_str(), "HEAD");
    assert_eq!(Method::Options.as_str(), "OPTIONS");
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
