//! Form data parsing tests

use super::super::*;
use std::collections::HashMap;

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
