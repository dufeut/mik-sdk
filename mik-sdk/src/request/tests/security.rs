//! Security-related tests: size limits, truncation, CRLF injection, path traversal

use super::super::*;
use std::collections::HashMap;

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
fn test_path_traversal_null_byte_paths() {
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
