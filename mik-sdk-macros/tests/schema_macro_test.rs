//! Tests for the derive macros (Type, Query, Path) and their generated code.
//!
//! These tests verify:
//! 1. Type derive generates FromJson, Validate, and OpenApiSchema
//! 2. Query derive generates FromQuery
//! 3. Path derive generates FromPath
//! 4. Field attributes work correctly

#![allow(dead_code)]

use mik_sdk_macros::{Path, Query, Type};
use std::collections::HashMap;

// Mock the mik_sdk types needed by generated code
#[allow(dead_code)]
mod mik_sdk {
    pub mod typed {
        use std::collections::HashMap;

        #[derive(Debug, Clone)]
        pub struct ParseError {
            pub field: String,
            pub message: String,
        }

        impl ParseError {
            pub fn missing(field: &str) -> Self {
                Self {
                    field: field.to_string(),
                    message: format!("Missing required field: {}", field),
                }
            }

            pub fn invalid_format(field: &str, value: &str) -> Self {
                Self {
                    field: field.to_string(),
                    message: format!("Invalid format for '{}': {}", field, value),
                }
            }

            pub fn type_mismatch(field: &str, expected: &str) -> Self {
                Self {
                    field: field.to_string(),
                    message: format!("Expected {} for field '{}'", expected, field),
                }
            }
        }

        #[derive(Debug, Clone)]
        pub struct ValidationError {
            pub field: String,
            pub constraint: String,
            pub message: String,
        }

        impl ValidationError {
            pub fn min(field: &str, min: i64) -> Self {
                Self {
                    field: field.to_string(),
                    constraint: "min".to_string(),
                    message: format!("'{}' must be at least {}", field, min),
                }
            }

            pub fn max(field: &str, max: i64) -> Self {
                Self {
                    field: field.to_string(),
                    constraint: "max".to_string(),
                    message: format!("'{}' must be at most {}", field, max),
                }
            }
        }

        pub trait FromJson: Sized {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError>;
        }

        pub trait FromQuery: Sized {
            fn from_query(params: &[(String, String)]) -> Result<Self, ParseError>;
        }

        pub trait FromPath: Sized {
            fn from_params(params: &HashMap<String, String>) -> Result<Self, ParseError>;
        }

        pub trait Validate {
            fn validate(&self) -> Result<(), ValidationError>;
        }

        pub trait OpenApiSchema {
            fn openapi_schema() -> &'static str;
            fn schema_name() -> &'static str;
            fn openapi_query_params() -> &'static str {
                "[]"
            }
        }

        // Implement FromJson for primitives
        impl FromJson for String {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .str()
                    .ok_or_else(|| ParseError::type_mismatch("value", "string"))
            }
        }

        impl FromJson for i32 {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .int()
                    .map(|n| n as i32)
                    .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
            }
        }

        impl FromJson for i64 {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .int()
                    .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
            }
        }

        impl FromJson for bool {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .bool()
                    .ok_or_else(|| ParseError::type_mismatch("value", "boolean"))
            }
        }

        impl<T: FromJson> FromJson for Vec<T> {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                let len = value
                    .len()
                    .ok_or_else(|| ParseError::type_mismatch("value", "array"))?;
                let mut result = Vec::with_capacity(len);
                for i in 0..len {
                    let item = value.at(i);
                    result.push(T::from_json(&item)?);
                }
                Ok(result)
            }
        }

        impl<T: FromJson> FromJson for Option<T> {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                if value.is_null() {
                    Ok(None)
                } else {
                    T::from_json(value).map(Some)
                }
            }
        }
    }

    pub mod json {
        use std::collections::HashMap;

        #[derive(Clone)]
        pub struct JsonValue {
            data: JsonData,
        }

        #[derive(Clone)]
        enum JsonData {
            Null,
            Bool(bool),
            Int(i64),
            Float(f64),
            String(String),
            Array(Vec<JsonValue>),
            Object(HashMap<String, JsonValue>),
        }

        impl JsonValue {
            pub fn null() -> Self {
                Self {
                    data: JsonData::Null,
                }
            }

            pub fn from_bool(b: bool) -> Self {
                Self {
                    data: JsonData::Bool(b),
                }
            }

            pub fn from_int(n: i64) -> Self {
                Self {
                    data: JsonData::Int(n),
                }
            }

            pub fn from_str(s: &str) -> Self {
                Self {
                    data: JsonData::String(s.to_string()),
                }
            }

            pub fn from_array(arr: Vec<JsonValue>) -> Self {
                Self {
                    data: JsonData::Array(arr),
                }
            }

            pub fn from_object(obj: HashMap<String, JsonValue>) -> Self {
                Self {
                    data: JsonData::Object(obj),
                }
            }

            pub fn get(&self, key: &str) -> JsonValue {
                match &self.data {
                    JsonData::Object(obj) => obj.get(key).cloned().unwrap_or_else(Self::null),
                    _ => Self::null(),
                }
            }

            pub fn at(&self, index: usize) -> JsonValue {
                match &self.data {
                    JsonData::Array(arr) => arr.get(index).cloned().unwrap_or_else(Self::null),
                    _ => Self::null(),
                }
            }

            pub fn str(&self) -> Option<String> {
                match &self.data {
                    JsonData::String(s) => Some(s.clone()),
                    _ => None,
                }
            }

            pub fn int(&self) -> Option<i64> {
                match &self.data {
                    JsonData::Int(n) => Some(*n),
                    _ => None,
                }
            }

            pub fn float(&self) -> Option<f64> {
                match &self.data {
                    JsonData::Float(n) => Some(*n),
                    JsonData::Int(n) => Some(*n as f64),
                    _ => None,
                }
            }

            pub fn bool(&self) -> Option<bool> {
                match &self.data {
                    JsonData::Bool(b) => Some(*b),
                    _ => None,
                }
            }

            pub fn is_null(&self) -> bool {
                matches!(&self.data, JsonData::Null)
            }

            pub fn len(&self) -> Option<usize> {
                match &self.data {
                    JsonData::Array(arr) => Some(arr.len()),
                    _ => None,
                }
            }
        }
    }
}

// =============================================================================
// TYPE DERIVE TESTS
// =============================================================================

#[test]
fn test_type_derive_basic() {
    #[derive(Type)]
    struct User {
        name: String,
        age: i32,
    }

    // Test FromJson
    let mut obj = HashMap::new();
    obj.insert(
        "name".to_string(),
        mik_sdk::json::JsonValue::from_str("Alice"),
    );
    obj.insert("age".to_string(), mik_sdk::json::JsonValue::from_int(30));
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let user = <User as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);

    // Test OpenApiSchema
    let schema = <User as mik_sdk::typed::OpenApiSchema>::openapi_schema();
    assert!(schema.contains("object"));
    assert!(schema.contains("name"));
    assert!(schema.contains("age"));
}

#[test]
fn test_type_derive_optional_fields() {
    #[derive(Type)]
    struct Profile {
        name: String,
        bio: Option<String>,
    }

    // With optional field present
    let mut obj = HashMap::new();
    obj.insert(
        "name".to_string(),
        mik_sdk::json::JsonValue::from_str("Bob"),
    );
    obj.insert(
        "bio".to_string(),
        mik_sdk::json::JsonValue::from_str("Hello"),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let profile = <Profile as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(profile.name, "Bob");
    assert_eq!(profile.bio, Some("Hello".to_string()));

    // With optional field missing
    let mut obj = HashMap::new();
    obj.insert(
        "name".to_string(),
        mik_sdk::json::JsonValue::from_str("Bob"),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let profile = <Profile as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(profile.name, "Bob");
    assert_eq!(profile.bio, None);
}

#[test]
fn test_type_derive_missing_required() {
    #[derive(Type)]
    struct Required {
        name: String,
    }

    let json = mik_sdk::json::JsonValue::from_object(HashMap::new());
    let result = <Required as mik_sdk::typed::FromJson>::from_json(&json);
    assert!(result.is_err());
}

#[test]
fn test_type_derive_validation() {
    #[derive(Type)]
    struct Constrained {
        #[field(min = 1, max = 10)]
        value: i32,
    }

    // Valid value
    let c = Constrained { value: 5 };
    assert!(<Constrained as mik_sdk::typed::Validate>::validate(&c).is_ok());

    // Value too small
    let c = Constrained { value: 0 };
    assert!(<Constrained as mik_sdk::typed::Validate>::validate(&c).is_err());

    // Value too large
    let c = Constrained { value: 100 };
    assert!(<Constrained as mik_sdk::typed::Validate>::validate(&c).is_err());
}

#[test]
fn test_type_derive_string_validation() {
    #[derive(Type)]
    struct Username {
        #[field(min = 3, max = 20)]
        name: String,
    }

    // Valid length
    let u = Username {
        name: "alice".to_string(),
    };
    assert!(<Username as mik_sdk::typed::Validate>::validate(&u).is_ok());

    // Too short
    let u = Username {
        name: "ab".to_string(),
    };
    assert!(<Username as mik_sdk::typed::Validate>::validate(&u).is_err());

    // Too long
    let u = Username {
        name: "a".repeat(25),
    };
    assert!(<Username as mik_sdk::typed::Validate>::validate(&u).is_err());
}

#[test]
fn test_type_derive_vec_field() {
    #[derive(Type)]
    struct Tags {
        items: Vec<String>,
    }

    let arr = vec![
        mik_sdk::json::JsonValue::from_str("rust"),
        mik_sdk::json::JsonValue::from_str("wasm"),
    ];
    let mut obj = HashMap::new();
    obj.insert(
        "items".to_string(),
        mik_sdk::json::JsonValue::from_array(arr),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let tags = <Tags as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(tags.items.len(), 2);
    assert_eq!(tags.items[0], "rust");
    assert_eq!(tags.items[1], "wasm");
}

// =============================================================================
// QUERY DERIVE TESTS
// =============================================================================

#[test]
fn test_query_derive_basic() {
    #[derive(Query)]
    struct ListQuery {
        #[field(default = 1)]
        page: u32,
        #[field(default = 20)]
        limit: u32,
    }

    // With values
    let params = vec![
        ("page".to_string(), "5".to_string()),
        ("limit".to_string(), "50".to_string()),
    ];
    let query = <ListQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.page, 5);
    assert_eq!(query.limit, 50);

    // With defaults
    let params = vec![];
    let query = <ListQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.page, 1);
    assert_eq!(query.limit, 20);
}

#[test]
fn test_query_derive_optional() {
    #[derive(Query)]
    struct SearchQuery {
        search: Option<String>,
    }

    // With value
    let params = vec![("search".to_string(), "hello".to_string())];
    let query = <SearchQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.search, Some("hello".to_string()));

    // Without value
    let params = vec![];
    let query = <SearchQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.search, None);
}

// =============================================================================
// PATH DERIVE TESTS
// =============================================================================

#[test]
fn test_path_derive_basic() {
    #[derive(Path)]
    struct UserPath {
        id: String,
    }

    let mut params = HashMap::new();
    params.insert("id".to_string(), "123".to_string());

    let path = <UserPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.id, "123");
}

#[test]
fn test_path_derive_multiple() {
    #[derive(Path)]
    struct OrgUserPath {
        org_id: String,
        user_id: String,
    }

    let mut params = HashMap::new();
    params.insert("org_id".to_string(), "acme".to_string());
    params.insert("user_id".to_string(), "456".to_string());

    let path = <OrgUserPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.org_id, "acme");
    assert_eq!(path.user_id, "456");
}

#[test]
fn test_path_derive_missing() {
    #[derive(Path)]
    struct RequiredPath {
        id: String,
    }

    let params = HashMap::new();
    let result = <RequiredPath as mik_sdk::typed::FromPath>::from_params(&params);
    assert!(result.is_err());
}

// =============================================================================
// OPENAPI SCHEMA TESTS
// =============================================================================

#[test]
fn test_openapi_schema_content() {
    #[derive(Type)]
    struct TestSchema {
        name: String,
        count: i32,
        active: bool,
    }

    let schema = <TestSchema as mik_sdk::typed::OpenApiSchema>::openapi_schema();
    assert!(schema.contains("\"type\":\"object\""));
    assert!(schema.contains("\"properties\""));
    assert!(schema.contains("\"name\""));
    assert!(schema.contains("\"count\""));
    assert!(schema.contains("\"active\""));
}

#[test]
fn test_openapi_schema_name() {
    #[derive(Type)]
    struct MyType {
        field: String,
    }

    let name = <MyType as mik_sdk::typed::OpenApiSchema>::schema_name();
    assert_eq!(name, "MyType");
}

// =============================================================================
// TYPED INPUT VALIDATION EDGE CASE TESTS
// =============================================================================

#[test]
fn test_type_derive_nested_struct() {
    #[derive(Type)]
    struct Address {
        city: String,
    }

    #[derive(Type)]
    struct Person {
        name: String,
        address: Address,
    }

    // Test nested struct parsing
    let mut addr_obj = HashMap::new();
    addr_obj.insert(
        "city".to_string(),
        mik_sdk::json::JsonValue::from_str("NYC"),
    );

    let mut obj = HashMap::new();
    obj.insert(
        "name".to_string(),
        mik_sdk::json::JsonValue::from_str("Alice"),
    );
    obj.insert(
        "address".to_string(),
        mik_sdk::json::JsonValue::from_object(addr_obj),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let person = <Person as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(person.name, "Alice");
    assert_eq!(person.address.city, "NYC");
}

#[test]
fn test_type_derive_empty_vec() {
    #[derive(Type)]
    struct EmptyTags {
        tags: Vec<String>,
    }

    let mut obj = HashMap::new();
    obj.insert(
        "tags".to_string(),
        mik_sdk::json::JsonValue::from_array(vec![]),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let tags = <EmptyTags as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert!(tags.tags.is_empty());
}

#[test]
fn test_type_derive_type_mismatch_string_for_int() {
    #[derive(Type)]
    struct NeedsInt {
        count: i32,
    }

    let mut obj = HashMap::new();
    obj.insert(
        "count".to_string(),
        mik_sdk::json::JsonValue::from_str("not a number"),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let result = <NeedsInt as mik_sdk::typed::FromJson>::from_json(&json);
    assert!(result.is_err());
}

#[test]
fn test_type_derive_type_mismatch_int_for_string() {
    #[derive(Type)]
    struct NeedsString {
        name: String,
    }

    let mut obj = HashMap::new();
    obj.insert("name".to_string(), mik_sdk::json::JsonValue::from_int(42));
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let result = <NeedsString as mik_sdk::typed::FromJson>::from_json(&json);
    assert!(result.is_err());
}

#[test]
fn test_type_derive_null_for_required_field() {
    #[derive(Type)]
    struct RequiredField {
        value: String,
    }

    let mut obj = HashMap::new();
    obj.insert("value".to_string(), mik_sdk::json::JsonValue::null());
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let result = <RequiredField as mik_sdk::typed::FromJson>::from_json(&json);
    assert!(result.is_err());
}

#[test]
fn test_type_derive_validation_boundary_values() {
    #[derive(Type)]
    struct BoundaryTest {
        #[field(min = 0, max = 100)]
        value: i32,
    }

    // Exactly at min boundary
    let b = BoundaryTest { value: 0 };
    assert!(<BoundaryTest as mik_sdk::typed::Validate>::validate(&b).is_ok());

    // Exactly at max boundary
    let b = BoundaryTest { value: 100 };
    assert!(<BoundaryTest as mik_sdk::typed::Validate>::validate(&b).is_ok());

    // Just below min boundary
    let b = BoundaryTest { value: -1 };
    assert!(<BoundaryTest as mik_sdk::typed::Validate>::validate(&b).is_err());

    // Just above max boundary
    let b = BoundaryTest { value: 101 };
    assert!(<BoundaryTest as mik_sdk::typed::Validate>::validate(&b).is_err());
}

#[test]
fn test_query_derive_invalid_number_format() {
    #[derive(Query, Debug)]
    struct NumberQuery {
        #[field(default = 1)]
        page: u32,
    }

    // Invalid number format returns an error (default only applies when param is missing)
    let params = vec![("page".to_string(), "not_a_number".to_string())];
    let result = <NumberQuery as mik_sdk::typed::FromQuery>::from_query(&params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "page");
    // Error message now uses type_mismatch: "Expected integer for field 'page'"
    assert!(
        err.message.contains("Expected") && err.message.contains("integer"),
        "Expected type mismatch error, got: {}",
        err.message
    );

    // But missing param uses the default
    let empty_params: Vec<(String, String)> = vec![];
    let query = <NumberQuery as mik_sdk::typed::FromQuery>::from_query(&empty_params).unwrap();
    assert_eq!(query.page, 1);
}

#[test]
fn test_query_derive_empty_string_value() {
    #[derive(Query)]
    struct EmptyQuery {
        search: Option<String>,
    }

    // Empty string is still Some("")
    let params = vec![("search".to_string(), "".to_string())];
    let query = <EmptyQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.search, Some("".to_string()));
}

#[test]
fn test_path_derive_empty_string_param() {
    #[derive(Path)]
    struct EmptyPath {
        id: String,
    }

    let mut params = HashMap::new();
    params.insert("id".to_string(), "".to_string());

    // Empty string is valid (routing should prevent this, but parsing accepts it)
    let path = <EmptyPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.id, "");
}
