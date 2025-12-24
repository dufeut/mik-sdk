//! Hygiene tests - ensure user code with `__` prefixed variables doesn't conflict with macros.
//!
//! These tests verify that the `__mik_sdk_` prefix prevents naming collisions.

#![allow(dead_code)]

use mik_sdk_macros::{ensure, error, guard, json, ok};

// Mock types for testing
mod json {
    pub fn obj() -> MockBuilder {
        MockBuilder
    }
    pub fn str(_s: &str) {}
    pub fn int(_i: i64) {}
    pub fn parse(_s: &str) -> Option<MockJson> {
        Some(MockJson)
    }

    pub struct MockBuilder;
    impl MockBuilder {
        pub fn set(self, _k: &str, _v: ()) -> Self {
            self
        }
        pub fn to_bytes(&self) -> Vec<u8> {
            vec![]
        }
    }

    pub struct MockJson;
}

mod handler {
    pub struct Response {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: Option<Vec<u8>>,
    }
}

// Mock request
struct MockRequest;
impl MockRequest {
    fn json_with<F, T>(&self, _f: F) -> Option<T>
    where
        F: Fn(&str) -> Option<T>,
    {
        None
    }
}

// Helper for ensure! macro
mod mik_sdk {
    pub fn __ensure_helper<T>(opt: Option<T>) -> Option<T> {
        opt
    }
}

/// Test that user code can use `__body` without conflicting with macro internals.
#[test]
fn test_user_can_use_double_underscore_body() {
    let __body = "user's variable";

    // This should compile - macro uses __mik_sdk_body internally
    let _result = ok!({ "test": "value" });

    // User's variable should still be accessible
    assert_eq!(__body, "user's variable");
}

/// Test that user code can use `__val` without conflicting.
#[test]
fn test_user_can_use_double_underscore_val() {
    fn returns_option() -> Option<i32> {
        Some(42)
    }

    fn handler() -> handler::Response {
        let __val = "user's val";

        // This should compile - macro uses __mik_sdk_val internally
        let _x = ensure!(returns_option(), 404, "not found");

        // User's variable should still be accessible after macro
        let _ = __val;

        handler::Response {
            status: 200,
            headers: vec![],
            body: None,
        }
    }

    let _ = handler();
}

/// Test that user code can use `__v` without conflicting.
#[test]
fn test_user_can_use_double_underscore_v() {
    fn handler() -> handler::Response {
        let __v = [1, 2, 3];

        guard!(true, 400, "condition failed");

        // User's variable should still be accessible
        assert_eq!(__v.len(), 3);

        handler::Response {
            status: 200,
            headers: vec![],
            body: None,
        }
    }

    let _ = handler();
}

/// Test that user code can use `__req` without conflicting.
#[test]
fn test_user_can_use_double_underscore_req() {
    let __req = MockRequest;

    // This would use __mik_sdk_val internally, not __req
    fn handler() -> handler::Response {
        let __req = "user's req variable";

        let _ = error! {
            status: 400,
            title: "Test"
        };

        // User's variable should still be accessible
        let _ = __req;

        handler::Response {
            status: 200,
            headers: vec![],
            body: None,
        }
    }

    // Outer __req should still work
    let _ = __req;
    let _ = handler();
}

/// Test that user code can use `__path` without conflicting.
#[test]
fn test_user_can_use_double_underscore_path() {
    let __path = "/users/123";

    let _json = json!({
        "path": "test"
    });

    // User's variable should still be accessible
    assert_eq!(__path, "/users/123");
}

/// Test that user code can use `__method` without conflicting.
#[test]
fn test_user_can_use_double_underscore_method() {
    let __method = "GET";

    let _ = ok!({ "method": "POST" });

    // User's variable should still be accessible
    assert_eq!(__method, "GET");
}

/// Test multiple `__` prefixed variables in same scope.
#[test]
fn test_multiple_double_underscore_variables() {
    fn handler() -> handler::Response {
        let __body = "user body";
        let __val = 42;
        let __v = [1, 2, 3];
        let __path = "/test";
        let __method = "POST";
        let __req = "request";
        let __params = std::collections::HashMap::<String, String>::new();

        // Use macros that internally use these names (with __mik_sdk_ prefix)
        guard!(true, 400, "test");

        let _ = ok!({ "test": "value" });

        // All user variables should still be accessible
        assert_eq!(__body, "user body");
        assert_eq!(__val, 42);
        assert_eq!(__v.len(), 3);
        assert_eq!(__path, "/test");
        assert_eq!(__method, "POST");
        assert_eq!(__req, "request");
        assert!(__params.is_empty());

        handler::Response {
            status: 200,
            headers: vec![],
            body: None,
        }
    }

    let _ = handler();
}
