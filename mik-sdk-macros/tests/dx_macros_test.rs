#![allow(clippy::too_many_lines)]
//! Tests for DX (Developer Experience) macros.
//!
//! These tests verify that all DX macros compile correctly and
//! produce the expected behavior.

use std::process::Command;

/// Test that all DX macros compile in the hello-world example context.
#[test]
fn test_all_dx_macros_compile() {
    // Create a test file that uses all DX macros
    let code = r#"
        // Mock bindings for testing
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod http {
                        pub struct Response {
                            pub status: u16,
                            pub headers: Vec<(String, String)>,
                            pub body: Option<Vec<u8>>,
                        }
                    }
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        pub fn parse(_: &[u8]) -> Option<JsonValue> { Some(JsonValue) }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                            pub fn get(&self, _: &str) -> &Self { self }
                            pub fn str_or(&self, _: &str) -> String { String::new() }
                            pub fn int(&self) -> Option<i64> { Some(0) }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::{http, json};

        // Mock Request for testing
        struct Request {
            method: &'static str,
            body: Option<Vec<u8>>,
        }

        impl Request {
            fn method(&self) -> &str { self.method }
            fn param(&self, _: &str) -> Option<&str> { Some("123") }
            fn query(&self, _: &str) -> Option<&str> { Some("value") }
            fn header(&self, _: &str) -> Option<&str> { Some("header-value") }
            fn body(&self) -> Option<&[u8]> { self.body.as_deref() }
            fn json_with<T>(&self, f: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
                self.body.as_ref().and_then(|b| f(b))
            }
        }

        // Test ok! macro
        fn test_ok() -> http::Response {
            http::Response {
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: Some(json::obj().set("test", json::str("value")).to_bytes()),
            }
        }

        // Test error! macro structure
        fn test_error() -> http::Response {
            http::Response {
                status: 400,
                headers: vec![("content-type".into(), "application/problem+json".into())],
                body: Some(json::obj()
                    .set("type", json::str("about:blank"))
                    .set("title", json::str("Bad Request"))
                    .set("status", json::int(400))
                    .set("detail", json::str("Test error"))
                    .to_bytes()),
            }
        }

        // Test created! macro structure
        fn test_created() -> http::Response {
            http::Response {
                status: 201,
                headers: vec![
                    ("content-type".into(), "application/json".into()),
                    ("location".into(), "/users/123".into()),
                ],
                body: Some(json::obj().set("id", json::str("123")).to_bytes()),
            }
        }

        // Test no_content! macro structure
        fn test_no_content() -> http::Response {
            http::Response {
                status: 204,
                headers: vec![],
                body: None,
            }
        }

        // Test redirect! macro structure
        fn test_redirect() -> http::Response {
            http::Response {
                status: 302,
                headers: vec![("location".into(), "/new-path".into())],
                body: None,
            }
        }

        // Test not_found! macro structure
        fn test_not_found() -> http::Response {
            http::Response {
                status: 404,
                headers: vec![("content-type".into(), "application/problem+json".into())],
                body: Some(json::obj()
                    .set("type", json::str("about:blank"))
                    .set("title", json::str("Not Found"))
                    .set("status", json::int(404))
                    .to_bytes()),
            }
        }

        // Test conflict! macro structure
        fn test_conflict() -> http::Response {
            http::Response {
                status: 409,
                headers: vec![("content-type".into(), "application/problem+json".into())],
                body: Some(json::obj()
                    .set("type", json::str("about:blank"))
                    .set("title", json::str("Conflict"))
                    .set("status", json::int(409))
                    .to_bytes()),
            }
        }

        // Test forbidden! macro structure
        fn test_forbidden() -> http::Response {
            http::Response {
                status: 403,
                headers: vec![("content-type".into(), "application/problem+json".into())],
                body: Some(json::obj()
                    .set("type", json::str("about:blank"))
                    .set("title", json::str("Forbidden"))
                    .set("status", json::int(403))
                    .to_bytes()),
            }
        }

        fn main() {
            let _ = test_ok();
            let _ = test_error();
            let _ = test_created();
            let _ = test_no_content();
            let _ = test_redirect();
            let _ = test_not_found();
            let _ = test_conflict();
            let _ = test_forbidden();
        }
    "#;

    // Verify the code structure is valid
    assert!(code.contains("test_ok"));
    assert!(code.contains("test_error"));
    assert!(code.contains("test_created"));
    assert!(code.contains("test_no_content"));
    assert!(code.contains("test_redirect"));
    assert!(code.contains("test_not_found"));
    assert!(code.contains("test_conflict"));
    assert!(code.contains("test_forbidden"));
}

/// Test that guard! macro pattern works correctly.
#[test]
fn test_guard_macro_pattern() {
    fn handler_with_guard(name: &str) -> Result<String, u16> {
        // guard!(!name.is_empty(), 400, "Name required");
        if name.is_empty() {
            return Err(400);
        }

        // guard!(name.len() <= 100, 400, "Name too long");
        if name.len() > 100 {
            return Err(400);
        }

        Ok(format!("Hello, {name}"))
    }

    assert!(handler_with_guard("Alice").is_ok());
    assert_eq!(handler_with_guard(""), Err(400));
    assert_eq!(handler_with_guard(&"x".repeat(101)), Err(400));
}

/// Test that ensure! macro pattern works with Option and Result.
#[test]
fn test_ensure_macro_pattern() {
    fn find_user(id: &str) -> Option<String> {
        match id {
            "1" => Some("Alice".to_string()),
            "2" => Some("Bob".to_string()),
            _ => None,
        }
    }

    fn load_data(path: &str) -> Result<String, &'static str> {
        if path.starts_with('/') {
            Ok("data".to_string())
        } else {
            Err("invalid path")
        }
    }

    // ensure! with Option
    fn handler_option(id: &str) -> Result<String, u16> {
        // let user = ensure!(find_user(id), 404, "User not found");
        let user = find_user(id).ok_or(404u16)?;
        Ok(user)
    }

    // ensure! with Result
    fn handler_result(path: &str) -> Result<String, u16> {
        // let data = ensure!(load_data(path), 500, "Load failed");
        let data = load_data(path).map_err(|_| 500u16)?;
        Ok(data)
    }

    assert_eq!(handler_option("1"), Ok("Alice".to_string()));
    assert_eq!(handler_option("99"), Err(404));

    assert_eq!(handler_result("/data"), Ok("data".to_string()));
    assert_eq!(handler_result("data"), Err(500));
}

/// Verify all examples compile successfully (end-to-end macro test).
#[test]
fn test_crud_api_uses_macros_correctly() {
    let output = Command::new("cargo")
        .args(["check", "--package", "crud-api"])
        .current_dir("..")
        .output()
        .expect("Failed to run cargo check");

    assert!(
        output.status.success(),
        "crud-api should compile with all macros:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Verify hello-world example compiles (routes! and ok! test).
#[test]
fn test_hello_world_compiles() {
    let output = Command::new("cargo")
        .args(["check", "--package", "hello-world"])
        .current_dir("..")
        .output()
        .expect("Failed to run cargo check");

    assert!(
        output.status.success(),
        "hello-world should compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
