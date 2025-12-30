//! Tests for the fetch! macro.

use mik_sdk::fetch;
use mik_sdk::http_client::{ClientRequest, Method};

#[test]
fn test_fetch_simple_get() {
    let req: ClientRequest = fetch!(GET "https://api.example.com/users");
    assert_eq!(req.method(), Method::Get);
    assert_eq!(req.url(), "https://api.example.com/users");
    assert!(req.headers().is_empty());
    assert!(req.body_bytes().is_none());
}

#[test]
fn test_fetch_simple_post() {
    let req: ClientRequest = fetch!(POST "https://api.example.com/users");
    assert_eq!(req.method(), Method::Post);
    assert_eq!(req.url(), "https://api.example.com/users");
}

#[test]
fn test_fetch_all_methods() {
    assert_eq!(fetch!(GET "http://x").method(), Method::Get);
    assert_eq!(fetch!(POST "http://x").method(), Method::Post);
    assert_eq!(fetch!(PUT "http://x").method(), Method::Put);
    assert_eq!(fetch!(DELETE "http://x").method(), Method::Delete);
    assert_eq!(fetch!(PATCH "http://x").method(), Method::Patch);
    assert_eq!(fetch!(HEAD "http://x").method(), Method::Head);
    assert_eq!(fetch!(OPTIONS "http://x").method(), Method::Options);
}

#[test]
fn test_fetch_with_headers() {
    let req: ClientRequest = fetch!(GET "https://api.example.com/data",
        headers: {
            "Authorization": "Bearer token123",
            "Accept": "application/json"
        }
    );

    assert_eq!(req.method(), Method::Get);
    assert_eq!(req.headers().len(), 2);

    // Check headers exist (order may vary)
    let headers: Vec<_> = req.headers().iter().collect();
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer token123")
    );
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "Accept" && v == "application/json")
    );
}

#[test]
fn test_fetch_with_dynamic_url() {
    let user_id = "123";
    let req: ClientRequest = fetch!(GET format!("https://api.example.com/users/{}", user_id));

    assert_eq!(req.url(), "https://api.example.com/users/123");
}

#[test]
fn test_fetch_with_dynamic_headers() {
    let token = "secret-token";
    let req: ClientRequest = fetch!(GET "https://api.example.com/protected",
        headers: {
            "Authorization": format!("Bearer {}", token)
        }
    );

    let auth_header = req.headers().iter().find(|(k, _)| k == "Authorization");
    assert!(auth_header.is_some());
    assert_eq!(auth_header.unwrap().1, "Bearer secret-token");
}

#[test]
fn test_fetch_with_raw_body() {
    let body_bytes = b"raw body content";
    let req: ClientRequest = fetch!(POST "https://api.example.com/upload",
        body: body_bytes
    );

    assert_eq!(req.method(), Method::Post);
    assert_eq!(req.body_bytes(), Some(b"raw body content".as_slice()));
}

#[test]
fn test_fetch_with_timeout() {
    let req: ClientRequest = fetch!(GET "https://slow-api.example.com/data",
        timeout: 5000
    );

    // timeout_ms converts to nanoseconds
    assert_eq!(req.timeout(), Some(5_000_000_000));
}

#[test]
fn test_fetch_with_all_options() {
    let token = "my-token";
    let body_data = b"some data";

    let req: ClientRequest = fetch!(PUT "https://api.example.com/resource",
        headers: {
            "Authorization": format!("Bearer {}", token),
            "X-Custom": "value"
        },
        body: body_data,
        timeout: 10000
    );

    assert_eq!(req.method(), Method::Put);
    assert_eq!(req.url(), "https://api.example.com/resource");
    assert_eq!(req.headers().len(), 2);
    assert_eq!(req.body_bytes(), Some(b"some data".as_slice()));
    assert_eq!(req.timeout(), Some(10_000_000_000));
}

#[test]
fn test_fetch_case_insensitive_method() {
    // Methods should work regardless of case
    let req1: ClientRequest = fetch!(get "https://example.com");
    let req2: ClientRequest = fetch!(Get "https://example.com");
    let req3: ClientRequest = fetch!(GET "https://example.com");

    assert_eq!(req1.method(), Method::Get);
    assert_eq!(req2.method(), Method::Get);
    assert_eq!(req3.method(), Method::Get);
}

#[test]
fn test_fetch_with_trailing_comma() {
    // Should handle trailing comma gracefully
    let req: ClientRequest = fetch!(GET "https://example.com",
        headers: { "Accept": "application/json" },
    );

    assert_eq!(req.headers().len(), 1);
}

#[test]
fn test_fetch_post_with_vec_body() {
    let body: Vec<u8> = vec![1, 2, 3, 4, 5];
    let req: ClientRequest = fetch!(POST "https://api.example.com/binary",
        body: &body
    );

    assert_eq!(req.body_bytes(), Some([1, 2, 3, 4, 5].as_slice()));
}
