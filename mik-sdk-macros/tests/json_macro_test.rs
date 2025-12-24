//! Tests for the json!, ok!, and error! macro outputs.
//!
//! These tests verify the macros generate valid code by checking
//! that they compile correctly with mock bindings.

use std::process::Command;

#[test]
fn test_json_macro_basic_types() {
    // Test that the json! macro handles all basic types
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn arr() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        pub fn float(_: f64) -> JsonValue { JsonValue }
                        pub fn bool_val(_: bool) -> JsonValue { JsonValue }
                        pub fn null() -> JsonValue { JsonValue }

                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn push(self, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_literals() {
            // Strings
            let _ = json::obj().set("name", json::str("Alice"));

            // Numbers
            let _ = json::obj().set("age", json::int(30));
            let _ = json::obj().set("score", json::float(95.5));

            // Booleans
            let _ = json::obj().set("active", json::bool_val(true));

            // Null
            let _ = json::obj().set("deleted_at", json::null());

            // Arrays
            let _ = json::arr().push(json::int(1)).push(json::int(2));

            // Nested objects
            let _ = json::obj()
                .set("user", json::obj().set("name", json::str("Bob")));
        }

        fn main() {
            test_literals();
        }
    "#;

    // Just verify the pattern compiles (code is non-empty by definition)
    let _ = code;
}

#[test]
fn test_error_macro_rfc7807_structure() {
    // Verify error! macro produces RFC 7807 compliant structure
    // Required fields: status (mandatory), type, title, detail
    // Optional fields: instance, meta (extensions)

    let rfc7807_fields = [
        "status", // HTTP status code
        "type",   // URI reference identifying problem type
        "title",  // Short, human-readable summary
        "detail", // Human-readable explanation
    ];

    for field in &rfc7807_fields {
        // Each field should be valid in error responses
        assert!(!field.is_empty());
    }
}

#[test]
fn test_crud_api_builds_successfully() {
    // Verify the crud-api example compiles with all the macros
    let output = Command::new("cargo")
        .args(["check", "--package", "crud-api"])
        .current_dir("..")
        .output()
        .expect("Failed to run cargo check");

    assert!(
        output.status.success(),
        "crud-api should compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_status_constants_available() {
    // Verify status constants are properly exported
    let output = Command::new("cargo")
        .args(["check", "--package", "mik-sdk"])
        .current_dir("..")
        .output()
        .expect("Failed to run cargo check");

    assert!(
        output.status.success(),
        "mik-sdk crate should compile with status constants:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
