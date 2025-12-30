#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
//! Tests for the derive macros (Type, Query, Path) and their generated code.
//!
//! These tests verify:
//! 1. Type derive generates `FromJson`, Validate, and `OpenApiSchema`
//! 2. Query derive generates `FromQuery`
//! 3. Path derive generates `FromPath`
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
                    message: format!("Missing required field: {field}"),
                }
            }

            pub fn invalid_format(field: &str, value: &str) -> Self {
                Self {
                    field: field.to_string(),
                    message: format!("Invalid format for '{field}': {value}"),
                }
            }

            pub fn type_mismatch(field: &str, expected: &str) -> Self {
                Self {
                    field: field.to_string(),
                    message: format!("Expected {expected} for field '{field}'"),
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
                    message: format!("'{field}' must be at least {min}"),
                }
            }

            pub fn max(field: &str, max: i64) -> Self {
                Self {
                    field: field.to_string(),
                    constraint: "max".to_string(),
                    message: format!("'{field}' must be at most {max}"),
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
                    .map(|n| n as Self)
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

        impl FromJson for f64 {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .float()
                    .ok_or_else(|| ParseError::type_mismatch("value", "float"))
            }
        }

        impl FromJson for u32 {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .int()
                    .map(|n| n as Self)
                    .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
            }
        }

        impl FromJson for u64 {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                value
                    .int()
                    .map(|n| n as Self)
                    .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
            }
        }

        impl<T: FromJson> FromJson for Vec<T> {
            fn from_json(value: &crate::mik_sdk::json::JsonValue) -> Result<Self, ParseError> {
                let len = value
                    .len()
                    .ok_or_else(|| ParseError::type_mismatch("value", "array"))?;
                let mut result = Self::with_capacity(len);
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
            pub const fn null() -> Self {
                Self {
                    data: JsonData::Null,
                }
            }

            pub const fn from_bool(b: bool) -> Self {
                Self {
                    data: JsonData::Bool(b),
                }
            }

            pub const fn from_int(n: i64) -> Self {
                Self {
                    data: JsonData::Int(n),
                }
            }

            pub const fn from_float(f: f64) -> Self {
                Self {
                    data: JsonData::Float(f),
                }
            }

            pub fn from_str(s: &str) -> Self {
                Self {
                    data: JsonData::String(s.to_string()),
                }
            }

            pub const fn from_array(arr: Vec<Self>) -> Self {
                Self {
                    data: JsonData::Array(arr),
                }
            }

            pub const fn from_object(obj: HashMap<String, Self>) -> Self {
                Self {
                    data: JsonData::Object(obj),
                }
            }

            pub fn get(&self, key: &str) -> Self {
                match &self.data {
                    JsonData::Object(obj) => obj.get(key).cloned().unwrap_or_else(Self::null),
                    _ => Self::null(),
                }
            }

            pub fn at(&self, index: usize) -> Self {
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

            pub const fn int(&self) -> Option<i64> {
                match &self.data {
                    JsonData::Int(n) => Some(*n),
                    _ => None,
                }
            }

            pub const fn float(&self) -> Option<f64> {
                match &self.data {
                    JsonData::Float(n) => Some(*n),
                    JsonData::Int(n) => Some(*n as f64),
                    _ => None,
                }
            }

            pub const fn bool(&self) -> Option<bool> {
                match &self.data {
                    JsonData::Bool(b) => Some(*b),
                    _ => None,
                }
            }

            pub const fn is_null(&self) -> bool {
                matches!(&self.data, JsonData::Null)
            }

            pub const fn len(&self) -> Option<usize> {
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
    let params = vec![("search".to_string(), String::new())];
    let query = <EmptyQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.search, Some(String::new()));
}

#[test]
fn test_path_derive_empty_string_param() {
    #[derive(Path)]
    struct EmptyPath {
        id: String,
    }

    let mut params = HashMap::new();
    params.insert("id".to_string(), String::new());

    // Empty string is valid (routing should prevent this, but parsing accepts it)
    let path = <EmptyPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.id, "");
}

// =============================================================================
// HTTP METHOD COVERAGE TESTS
// =============================================================================

#[test]
fn test_type_derive_with_float_field() {
    #[derive(Type)]
    struct MetricsData {
        score: f64,
    }

    let mut obj = HashMap::new();
    obj.insert(
        "score".to_string(),
        mik_sdk::json::JsonValue::from_float(98.6),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let metrics = <MetricsData as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert!((metrics.score - 98.6).abs() < 0.001);
}

#[test]
fn test_type_derive_with_u32_field() {
    #[derive(Type)]
    struct CountData {
        count: u32,
    }

    let mut obj = HashMap::new();
    obj.insert("count".to_string(), mik_sdk::json::JsonValue::from_int(42));
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <CountData as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.count, 42);
}

#[test]
fn test_type_derive_with_i64_field() {
    #[derive(Type)]
    struct BigNumber {
        value: i64,
    }

    let mut obj = HashMap::new();
    obj.insert(
        "value".to_string(),
        mik_sdk::json::JsonValue::from_int(9_223_372_036_854_775_807),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <BigNumber as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.value, 9_223_372_036_854_775_807);
}

#[test]
fn test_type_derive_with_u64_field() {
    #[derive(Type)]
    struct BigUnsigned {
        value: u64,
    }

    let mut obj = HashMap::new();
    obj.insert(
        "value".to_string(),
        mik_sdk::json::JsonValue::from_int(1_000_000),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <BigUnsigned as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.value, 1_000_000);
}

#[test]
fn test_type_derive_with_optional_vec() {
    #[derive(Type)]
    struct OptionalTags {
        tags: Option<Vec<String>>,
    }

    // With value
    let arr = vec![mik_sdk::json::JsonValue::from_str("tag1")];
    let mut obj = HashMap::new();
    obj.insert(
        "tags".to_string(),
        mik_sdk::json::JsonValue::from_array(arr),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <OptionalTags as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.tags, Some(vec!["tag1".to_string()]));

    // Without value (null)
    let mut obj2 = HashMap::new();
    obj2.insert("tags".to_string(), mik_sdk::json::JsonValue::null());
    let json2 = mik_sdk::json::JsonValue::from_object(obj2);

    let data2 = <OptionalTags as mik_sdk::typed::FromJson>::from_json(&json2).unwrap();
    assert_eq!(data2.tags, None);
}

#[test]
fn test_type_derive_nested_vec() {
    #[derive(Type)]
    struct Item {
        name: String,
    }

    #[derive(Type)]
    struct ItemList {
        items: Vec<Item>,
    }

    let item1_obj = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            mik_sdk::json::JsonValue::from_str("item1"),
        );
        mik_sdk::json::JsonValue::from_object(m)
    };
    let item2_obj = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            mik_sdk::json::JsonValue::from_str("item2"),
        );
        mik_sdk::json::JsonValue::from_object(m)
    };

    let mut obj = HashMap::new();
    obj.insert(
        "items".to_string(),
        mik_sdk::json::JsonValue::from_array(vec![item1_obj, item2_obj]),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <ItemList as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.items.len(), 2);
    assert_eq!(data.items[0].name, "item1");
    assert_eq!(data.items[1].name, "item2");
}

#[test]
fn test_query_derive_with_bool_field() {
    #[derive(Query)]
    struct BoolQuery {
        active: Option<bool>,
    }

    // Test true
    let params = vec![("active".to_string(), "true".to_string())];
    let query = <BoolQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.active, Some(true));

    // Test false
    let params = vec![("active".to_string(), "false".to_string())];
    let query = <BoolQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.active, Some(false));

    // Test missing
    let params: Vec<(String, String)> = vec![];
    let query = <BoolQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.active, None);
}

#[test]
fn test_query_derive_with_i32_field() {
    #[derive(Query)]
    struct OffsetQuery {
        #[field(default = 0)]
        offset: i32,
    }

    // Positive value
    let params = vec![("offset".to_string(), "100".to_string())];
    let query = <OffsetQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.offset, 100);

    // Negative value
    let params = vec![("offset".to_string(), "-50".to_string())];
    let query = <OffsetQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.offset, -50);

    // Default value
    let params: Vec<(String, String)> = vec![];
    let query = <OffsetQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.offset, 0);
}

#[test]
fn test_query_derive_with_i64_field() {
    #[derive(Query)]
    struct TimestampQuery {
        since: Option<i64>,
    }

    let params = vec![("since".to_string(), "1704067200000".to_string())];
    let query = <TimestampQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.since, Some(1_704_067_200_000));
}

#[test]
fn test_query_derive_with_u64_field() {
    #[derive(Query)]
    struct BigOffsetQuery {
        cursor: Option<u64>,
    }

    let params = vec![("cursor".to_string(), "18446744073709551615".to_string())];
    let query = <BigOffsetQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    // Note: This may overflow depending on implementation
    assert!(query.cursor.is_some());
}

#[test]
fn test_path_derive_with_multiple_segments() {
    #[derive(Path)]
    struct DeepPath {
        org: String,
        team: String,
        user: String,
    }

    let mut params = HashMap::new();
    params.insert("org".to_string(), "acme".to_string());
    params.insert("team".to_string(), "engineering".to_string());
    params.insert("user".to_string(), "alice".to_string());

    let path = <DeepPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.org, "acme");
    assert_eq!(path.team, "engineering");
    assert_eq!(path.user, "alice");
}

#[test]
fn test_path_derive_with_special_characters() {
    #[derive(Path)]
    struct SlugPath {
        slug: String,
    }

    let mut params = HashMap::new();
    params.insert("slug".to_string(), "hello-world_2024".to_string());

    let path = <SlugPath as mik_sdk::typed::FromPath>::from_params(&params).unwrap();
    assert_eq!(path.slug, "hello-world_2024");
}

#[test]
fn test_type_derive_min_only_validation() {
    #[derive(Type)]
    struct MinOnly {
        #[field(min = 5)]
        value: i32,
    }

    // Valid (above min)
    let v = MinOnly { value: 10 };
    assert!(<MinOnly as mik_sdk::typed::Validate>::validate(&v).is_ok());

    // At boundary
    let v = MinOnly { value: 5 };
    assert!(<MinOnly as mik_sdk::typed::Validate>::validate(&v).is_ok());

    // Below min
    let v = MinOnly { value: 4 };
    assert!(<MinOnly as mik_sdk::typed::Validate>::validate(&v).is_err());
}

#[test]
fn test_type_derive_max_only_validation() {
    #[derive(Type)]
    struct MaxOnly {
        #[field(max = 100)]
        value: i32,
    }

    // Valid (below max)
    let v = MaxOnly { value: 50 };
    assert!(<MaxOnly as mik_sdk::typed::Validate>::validate(&v).is_ok());

    // At boundary
    let v = MaxOnly { value: 100 };
    assert!(<MaxOnly as mik_sdk::typed::Validate>::validate(&v).is_ok());

    // Above max
    let v = MaxOnly { value: 101 };
    assert!(<MaxOnly as mik_sdk::typed::Validate>::validate(&v).is_err());
}

#[test]
fn test_type_derive_multiple_validated_fields() {
    #[derive(Type)]
    struct MultiValidated {
        #[field(min = 1, max = 10)]
        a: i32,
        #[field(min = 0, max = 5)]
        b: i32,
    }

    // Both valid
    let v = MultiValidated { a: 5, b: 3 };
    assert!(<MultiValidated as mik_sdk::typed::Validate>::validate(&v).is_ok());

    // First invalid
    let v = MultiValidated { a: 0, b: 3 };
    assert!(<MultiValidated as mik_sdk::typed::Validate>::validate(&v).is_err());

    // Second invalid
    let v = MultiValidated { a: 5, b: 6 };
    assert!(<MultiValidated as mik_sdk::typed::Validate>::validate(&v).is_err());
}

#[test]
fn test_openapi_schema_with_optional_fields() {
    #[derive(Type)]
    struct OptionalFieldsType {
        required_field: String,
        optional_field: Option<String>,
    }

    let schema = <OptionalFieldsType as mik_sdk::typed::OpenApiSchema>::openapi_schema();
    assert!(schema.contains("required_field"));
    assert!(schema.contains("optional_field"));
    assert!(schema.contains("\"type\":\"object\""));
}

#[test]
fn test_openapi_schema_with_array_fields() {
    #[derive(Type)]
    struct ArrayFieldsType {
        items: Vec<String>,
        numbers: Vec<i32>,
    }

    let schema = <ArrayFieldsType as mik_sdk::typed::OpenApiSchema>::openapi_schema();
    assert!(schema.contains("items"));
    assert!(schema.contains("numbers"));
    assert!(schema.contains("\"type\":\"array\""));
}

#[test]
fn test_query_openapi_params() {
    #[derive(Query)]
    struct PaginationParams {
        #[field(default = 1)]
        page: u32,
        #[field(default = 20)]
        limit: u32,
        search: Option<String>,
    }

    let params = <PaginationParams as mik_sdk::typed::OpenApiSchema>::openapi_query_params();
    assert!(params.contains("page"));
    assert!(params.contains("limit"));
    assert!(params.contains("search"));
}

#[test]
fn test_path_openapi_schema() {
    #[derive(Path)]
    struct ResourceIdPath {
        resource_type: String,
        resource_id: String,
    }

    let schema = <ResourceIdPath as mik_sdk::typed::OpenApiSchema>::openapi_schema();
    assert!(schema.contains("resource_type"));
    assert!(schema.contains("resource_id"));
}

#[test]
fn test_type_derive_deeply_nested() {
    #[derive(Type)]
    struct Inner {
        value: String,
    }

    #[derive(Type)]
    struct Middle {
        inner: Inner,
    }

    #[derive(Type)]
    struct Outer {
        middle: Middle,
    }

    let inner_obj = {
        let mut m = HashMap::new();
        m.insert(
            "value".to_string(),
            mik_sdk::json::JsonValue::from_str("deep"),
        );
        mik_sdk::json::JsonValue::from_object(m)
    };

    let middle_obj = {
        let mut m = HashMap::new();
        m.insert("inner".to_string(), inner_obj);
        mik_sdk::json::JsonValue::from_object(m)
    };

    let outer_obj = {
        let mut m = HashMap::new();
        m.insert("middle".to_string(), middle_obj);
        mik_sdk::json::JsonValue::from_object(m)
    };

    let data = <Outer as mik_sdk::typed::FromJson>::from_json(&outer_obj).unwrap();
    assert_eq!(data.middle.inner.value, "deep");
}

#[test]
fn test_type_derive_with_vec_of_nested() {
    #[derive(Type)]
    struct Tag {
        name: String,
    }

    #[derive(Type)]
    struct Article {
        title: String,
        tags: Vec<Tag>,
    }

    let tag1 = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            mik_sdk::json::JsonValue::from_str("rust"),
        );
        mik_sdk::json::JsonValue::from_object(m)
    };
    let tag2 = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            mik_sdk::json::JsonValue::from_str("wasm"),
        );
        mik_sdk::json::JsonValue::from_object(m)
    };

    let mut obj = HashMap::new();
    obj.insert(
        "title".to_string(),
        mik_sdk::json::JsonValue::from_str("My Article"),
    );
    obj.insert(
        "tags".to_string(),
        mik_sdk::json::JsonValue::from_array(vec![tag1, tag2]),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let article = <Article as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(article.title, "My Article");
    assert_eq!(article.tags.len(), 2);
    assert_eq!(article.tags[0].name, "rust");
    assert_eq!(article.tags[1].name, "wasm");
}

#[test]
fn test_type_derive_all_optional_struct() {
    #[derive(Type)]
    struct AllOptional {
        a: Option<String>,
        b: Option<i32>,
        c: Option<bool>,
    }

    // All missing
    let json = mik_sdk::json::JsonValue::from_object(HashMap::new());
    let data = <AllOptional as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.a, None);
    assert_eq!(data.b, None);
    assert_eq!(data.c, None);

    // All present
    let mut obj = HashMap::new();
    obj.insert("a".to_string(), mik_sdk::json::JsonValue::from_str("test"));
    obj.insert("b".to_string(), mik_sdk::json::JsonValue::from_int(42));
    obj.insert("c".to_string(), mik_sdk::json::JsonValue::from_bool(true));
    let json = mik_sdk::json::JsonValue::from_object(obj);
    let data = <AllOptional as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.a, Some("test".to_string()));
    assert_eq!(data.b, Some(42));
    assert_eq!(data.c, Some(true));
}

#[test]
fn test_query_derive_multiple_values_same_key() {
    #[derive(Query)]
    struct MultiQuery {
        tag: Option<String>,
    }

    // When multiple values for same key, take last (overwrite behavior)
    let params = vec![
        ("tag".to_string(), "first".to_string()),
        ("tag".to_string(), "second".to_string()),
    ];
    let query = <MultiQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert_eq!(query.tag, Some("second".to_string()));
}

#[test]
fn test_type_derive_validation_error_contains_field_name() {
    #[derive(Type)]
    struct ValidatedField {
        #[field(min = 10)]
        age: i32,
    }

    let v = ValidatedField { age: 5 };
    let result = <ValidatedField as mik_sdk::typed::Validate>::validate(&v);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "age");
    assert_eq!(err.constraint, "min");
}

#[test]
fn test_path_derive_partial_params_fails() {
    #[derive(Path, Debug)]
    struct TwoParams {
        id: String,
        name: String,
    }

    // Only one param provided
    let mut params = HashMap::new();
    params.insert("id".to_string(), "123".to_string());

    let result = <TwoParams as mik_sdk::typed::FromPath>::from_params(&params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "name");
}

#[test]
fn test_type_derive_mixed_required_optional() {
    #[derive(Type)]
    struct MixedFields {
        required: String,
        optional1: Option<String>,
        also_required: i32,
        optional2: Option<bool>,
    }

    // With all required and some optional
    let mut obj = HashMap::new();
    obj.insert(
        "required".to_string(),
        mik_sdk::json::JsonValue::from_str("yes"),
    );
    obj.insert(
        "also_required".to_string(),
        mik_sdk::json::JsonValue::from_int(42),
    );
    obj.insert(
        "optional1".to_string(),
        mik_sdk::json::JsonValue::from_str("present"),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <MixedFields as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.required, "yes");
    assert_eq!(data.also_required, 42);
    assert_eq!(data.optional1, Some("present".to_string()));
    assert_eq!(data.optional2, None);
}

#[test]
fn test_type_with_int_vec() {
    #[derive(Type)]
    struct IntVecType {
        numbers: Vec<i32>,
    }

    let arr = vec![
        mik_sdk::json::JsonValue::from_int(1),
        mik_sdk::json::JsonValue::from_int(2),
        mik_sdk::json::JsonValue::from_int(3),
    ];
    let mut obj = HashMap::new();
    obj.insert(
        "numbers".to_string(),
        mik_sdk::json::JsonValue::from_array(arr),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <IntVecType as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.numbers, vec![1, 2, 3]);
}

#[test]
fn test_type_with_bool_vec() {
    #[derive(Type)]
    struct BoolVecType {
        flags: Vec<bool>,
    }

    let arr = vec![
        mik_sdk::json::JsonValue::from_bool(true),
        mik_sdk::json::JsonValue::from_bool(false),
        mik_sdk::json::JsonValue::from_bool(true),
    ];
    let mut obj = HashMap::new();
    obj.insert(
        "flags".to_string(),
        mik_sdk::json::JsonValue::from_array(arr),
    );
    let json = mik_sdk::json::JsonValue::from_object(obj);

    let data = <BoolVecType as mik_sdk::typed::FromJson>::from_json(&json).unwrap();
    assert_eq!(data.flags, vec![true, false, true]);
}

#[test]
fn test_query_bool_default() {
    #[derive(Query)]
    struct BoolDefaultQuery {
        #[field(default = true)]
        enabled: bool,
    }

    // Without value uses default
    let params: Vec<(String, String)> = vec![];
    let query = <BoolDefaultQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert!(query.enabled);

    // With value overrides default
    let params = vec![("enabled".to_string(), "false".to_string())];
    let query = <BoolDefaultQuery as mik_sdk::typed::FromQuery>::from_query(&params).unwrap();
    assert!(!query.enabled);
}
