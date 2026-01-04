//! Cookie parsing tests

use super::super::*;
use std::collections::HashMap;

#[test]
fn test_cookie_basic() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("cookie".to_string(), "session=abc123".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("session", ""), "abc123");
    assert_eq!(req.cookie_or("missing", "default"), "default");
}

#[test]
fn test_cookie_multiple() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "cookie".to_string(),
            "session=abc123; user=john; theme=dark".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("session", ""), "abc123");
    assert_eq!(req.cookie_or("user", ""), "john");
    assert_eq!(req.cookie_or("theme", ""), "dark");
}

#[test]
fn test_cookies_all() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "cookie".to_string(),
            "session=abc123; user=john".to_string(),
        )],
        None,
        HashMap::new(),
    );

    let cookies = req.cookies();
    assert_eq!(cookies.len(), 2);
    assert!(cookies.iter().any(|(n, v)| n == "session" && v == "abc123"));
    assert!(cookies.iter().any(|(n, v)| n == "user" && v == "john"));
}

#[test]
fn test_cookie_empty_header() {
    let req = Request::new(Method::Get, "/".to_string(), vec![], None, HashMap::new());

    assert_eq!(req.cookie_or("session", "default"), "default");
    assert!(req.cookies().is_empty());
}

#[test]
fn test_cookie_empty_value() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("cookie".to_string(), "empty=; normal=value".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("empty", "default"), "");
    assert_eq!(req.cookie_or("normal", "default"), "value");
}

#[test]
fn test_cookie_with_spaces() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "cookie".to_string(),
            "  session = abc123 ;  user=john  ".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("session", ""), "abc123");
    assert_eq!(req.cookie_or("user", ""), "john");
}

#[test]
fn test_cookie_value_with_equals() {
    // Cookie value can contain = signs
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("cookie".to_string(), "data=a=b=c".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("data", ""), "a=b=c");
}

#[test]
fn test_cookie_caching() {
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("cookie".to_string(), "session=abc123".to_string())],
        None,
        HashMap::new(),
    );

    // First access parses
    let first = req.cookie_or("session", "");
    // Second access uses cache
    let second = req.cookie_or("session", "");

    assert_eq!(first, "abc123");
    assert_eq!(second, "abc123");
}

#[test]
fn test_cookie_case_sensitivity() {
    // Cookie header lookup is case-insensitive (header name)
    // but cookie names themselves are case-sensitive
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![("Cookie".to_string(), "Session=abc; session=xyz".to_string())],
        None,
        HashMap::new(),
    );

    assert_eq!(req.cookie_or("Session", ""), "abc");
    assert_eq!(req.cookie_or("session", ""), "xyz");
}

#[test]
fn test_cookie_invalid_pairs_skipped() {
    // Invalid pairs (no =) are skipped
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![(
            "cookie".to_string(),
            "valid=value; invalid; also_valid=123".to_string(),
        )],
        None,
        HashMap::new(),
    );

    let cookies = req.cookies();
    assert_eq!(cookies.len(), 2);
    assert_eq!(req.cookie_or("valid", ""), "value");
    assert_eq!(req.cookie_or("also_valid", ""), "123");
    assert_eq!(req.cookie_or("invalid", "default"), "default");
}
