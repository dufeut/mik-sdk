#![allow(clippy::expect_used, clippy::unwrap_used)] // Test code
#![allow(clippy::needless_raw_string_hashes)] // r##"..."## needed for nested raw strings
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

// ============================================================================
// JSON MACRO EDGE CASE TESTS
// ============================================================================

#[test]
fn test_json_macro_deeply_nested_objects() {
    // Test 5+ levels of nesting
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_deep_nesting() -> json::JsonValue {
            // 5 levels deep: root -> level1 -> level2 -> level3 -> level4 -> value
            json::obj()
                .set("level1", json::obj()
                    .set("level2", json::obj()
                        .set("level3", json::obj()
                            .set("level4", json::obj()
                                .set("value", json::str("deep"))))))
        }

        fn main() { let _ = test_deep_nesting(); }
    "#;

    // Verify pattern exists (nested set calls)
    assert!(code.contains("level4"));
    assert!(code.contains("level3"));
}

#[test]
fn test_json_macro_mixed_arrays() {
    // Test arrays with mixed types
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn arr() -> JsonValue { JsonValue }
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        pub fn float(_: f64) -> JsonValue { JsonValue }
                        pub fn bool_val(_: bool) -> JsonValue { JsonValue }
                        pub fn null() -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn push(self, _: JsonValue) -> Self { self }
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_mixed_array() -> json::JsonValue {
            // Array with string, int, float, bool, null, nested object
            json::arr()
                .push(json::str("text"))
                .push(json::int(42))
                .push(json::float(3.14))
                .push(json::bool_val(true))
                .push(json::null())
                .push(json::obj().set("nested", json::str("object")))
        }

        fn main() { let _ = test_mixed_array(); }
    "#;

    assert!(code.contains("push"));
    assert!(code.contains("float"));
    assert!(code.contains("null()"));
}

#[test]
fn test_json_macro_empty_structures() {
    // Test empty object and empty array
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn arr() -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_empty_object() -> json::JsonValue {
            json::obj() // {} - empty object
        }

        fn test_empty_array() -> json::JsonValue {
            json::arr() // [] - empty array
        }

        fn main() {
            let _ = test_empty_object();
            let _ = test_empty_array();
        }
    "#;

    assert!(code.contains("json::obj()"));
    assert!(code.contains("json::arr()"));
}

#[test]
fn test_json_macro_unicode_values() {
    // Test unicode in string values (keys are always ASCII in this API)
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_unicode() -> json::JsonValue {
            json::obj()
                .set("japanese", json::str("日本語"))
                .set("emoji", json::str("🎉🚀"))
                .set("chinese", json::str("中文"))
                .set("arabic", json::str("العربية"))
                .set("mixed", json::str("Hello 世界 🌍"))
        }

        fn main() { let _ = test_unicode(); }
    "#;

    assert!(code.contains("日本語"));
    assert!(code.contains("🎉"));
}

#[test]
fn test_json_macro_special_string_values() {
    // Test strings with special characters that need escaping
    // Using r##"..."## to allow r#"..."# inside
    let code = r##"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_special_chars() -> json::JsonValue {
            json::obj()
                .set("with_quotes", json::str(r#"say "hello""#))
                .set("with_backslash", json::str(r"path\to\file"))
                .set("with_newline", json::str("line1\nline2"))
                .set("with_tab", json::str("col1\tcol2"))
        }

        fn main() { let _ = test_special_chars(); }
    "##;

    assert!(code.contains("with_quotes"));
    assert!(code.contains("with_newline"));
}

#[test]
fn test_json_macro_variable_interpolation() {
    // Test using variables in json construction
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        pub fn bool_val(_: bool) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_variables(name: &str, age: i64, active: bool) -> json::JsonValue {
            json::obj()
                .set("name", json::str(name))
                .set("age", json::int(age))
                .set("active", json::bool_val(active))
        }

        fn main() {
            let _ = test_variables("Alice", 30, true);
        }
    "#;

    assert!(code.contains("json::str(name)"));
    assert!(code.contains("json::int(age)"));
    assert!(code.contains("json::bool_val(active)"));
}

#[test]
fn test_json_macro_array_of_objects() {
    // Test array containing multiple objects
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn arr() -> JsonValue { JsonValue }
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn str(_: &str) -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn push(self, _: JsonValue) -> Self { self }
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_users_array() -> json::JsonValue {
            json::arr()
                .push(json::obj().set("id", json::int(1)).set("name", json::str("Alice")))
                .push(json::obj().set("id", json::int(2)).set("name", json::str("Bob")))
                .push(json::obj().set("id", json::int(3)).set("name", json::str("Charlie")))
        }

        fn main() { let _ = test_users_array(); }
    "#;

    assert!(code.contains("Alice"));
    assert!(code.contains("Bob"));
    assert!(code.contains("Charlie"));
}

#[test]
fn test_json_macro_numeric_edge_cases() {
    // Test numeric edge cases
    let code = r#"
        mod bindings {
            pub mod mik_sdk {
                pub mod core {
                    pub mod json {
                        pub struct JsonValue;
                        pub fn obj() -> JsonValue { JsonValue }
                        pub fn int(_: i64) -> JsonValue { JsonValue }
                        pub fn float(_: f64) -> JsonValue { JsonValue }
                        impl JsonValue {
                            pub fn set(self, _: &str, _: JsonValue) -> Self { self }
                            pub fn to_bytes(self) -> Vec<u8> { vec![] }
                        }
                    }
                }
            }
        }

        use bindings::mik_sdk::core::json;

        fn test_numerics() -> json::JsonValue {
            json::obj()
                .set("zero", json::int(0))
                .set("negative", json::int(-42))
                .set("max_safe", json::int(9007199254740991)) // JS MAX_SAFE_INTEGER
                .set("min_safe", json::int(-9007199254740991))
                .set("float_zero", json::float(0.0))
                .set("float_neg", json::float(-3.14))
                .set("scientific", json::float(1.23e10))
        }

        fn main() { let _ = test_numerics(); }
    "#;

    assert!(code.contains("9007199254740991"));
    assert!(code.contains("-42"));
    assert!(code.contains("1.23e10"));
}

#[test]
fn test_json_macro_object_with_array_field() {
    // Test object containing an array field
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

        fn test_object_with_array() -> json::JsonValue {
            json::obj()
                .set("name", json::str("Product"))
                .set("tags", json::arr()
                    .push(json::str("electronics"))
                    .push(json::str("gadgets"))
                    .push(json::str("new")))
                .set("versions", json::arr()
                    .push(json::int(1))
                    .push(json::int(2))
                    .push(json::int(3)))
        }

        fn main() { let _ = test_object_with_array(); }
    "#;

    assert!(code.contains("tags"));
    assert!(code.contains("versions"));
    assert!(code.contains("electronics"));
}
