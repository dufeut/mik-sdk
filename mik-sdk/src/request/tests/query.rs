//! Query parameter parsing tests

use super::super::*;
use std::collections::HashMap;

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
