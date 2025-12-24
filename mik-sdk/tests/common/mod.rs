//! Shared test utilities and fixtures.
//!
//! This module provides common helpers for integration tests.

use mik_sdk::{Method, Request};
use std::collections::HashMap;

/// Create a GET request with the given path.
#[allow(dead_code)]
pub fn get_request(path: &str) -> Request {
    Request::new(Method::Get, path.to_string(), vec![], None, HashMap::new())
}

/// Create a POST request with a body.
#[allow(dead_code)]
pub fn post_request(path: &str, body: &[u8]) -> Request {
    Request::new(
        Method::Post,
        path.to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        Some(body.to_vec()),
        HashMap::new(),
    )
}

/// Create a request with custom headers.
#[allow(dead_code)]
pub fn request_with_headers(method: Method, path: &str, headers: Vec<(&str, &str)>) -> Request {
    let headers: Vec<(String, String)> = headers
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Request::new(method, path.to_string(), headers, None, HashMap::new())
}

/// Create a request with path parameters.
#[allow(dead_code)]
pub fn request_with_params(path: &str, params: Vec<(&str, &str)>) -> Request {
    let params: HashMap<String, String> = params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Request::new(Method::Get, path.to_string(), vec![], None, params)
}

/// Sample JSON for testing.
#[allow(dead_code)]
pub const SAMPLE_JSON: &[u8] = br#"{"name":"Alice","age":30,"active":true}"#;

/// Sample form data for testing.
#[allow(dead_code)]
pub const SAMPLE_FORM: &[u8] = b"name=Alice&email=alice%40example.com";
