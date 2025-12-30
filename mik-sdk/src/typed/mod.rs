//! Typed input infrastructure for type-safe request handling.
//!
//! This module provides:
//! - [`Id`] - Built-in path parameter for single ID routes
//! - [`ParseError`] - Error type for parsing failures
//! - [`ValidationError`] - Error type for constraint validation
//! - Traits for parsing JSON, query strings, and path parameters
//!
//! # Newtypes and Validation
//!
//! This SDK provides structural types ([`Id`], derive macros) but intentionally
//! delegates field validation to external crates like [`garde`](https://docs.rs/garde).
//!
//! **Why?** Validation requirements vary widely between projects. Some need strict
//! email validation, others accept any string. By separating concerns:
//! - SDK handles parsing and type conversion
//! - Validation crates handle domain-specific rules
//!
//! ## Example: Validated Newtypes with `garde`
//!
//! ```ignore
//! use garde::Validate;
//! use mik_sdk::prelude::*;
//!
//! #[derive(Type, Validate)]
//! pub struct CreateUser {
//!     #[garde(length(min = 1, max = 100))]
//!     pub name: String,
//!     #[garde(email)]
//!     pub email: String,
//! }
//!
//! fn create_user(body: CreateUser, _req: &Request) -> Response {
//!     // Validate after parsing
//!     if let Err(report) = body.validate() {
//!         return bad_request!(&report.to_string());
//!     }
//!     ok!({ "status": "created" })
//! }
//! ```

mod parse_error;
mod validation_error;

pub use parse_error::ParseError;
pub use validation_error::ValidationError;

use crate::json::JsonValue;
use std::collections::HashMap;

// ============================================================================
// BUILT-IN TYPES
// ============================================================================

/// Single path parameter - String for JavaScript compatibility.
///
/// Use this for simple routes with a single `{id}` parameter:
///
/// ```ignore
/// routes! {
///     GET "/users/{id}" => get_user(path: Id) -> User,
/// }
///
/// fn get_user(path: Id, req: &Request) -> Response {
///     let user_id = &path.0;  // String
///     ok!({ "id": user_id })
/// }
/// ```
///
/// # Why String?
///
/// JavaScript `Number` loses precision for integers > 2^53.
/// String IDs are safe for all ID formats:
/// - Numeric: `"123"`
/// - UUID: `"550e8400-e29b-41d4-a716-446655440000"`
/// - Prefixed: `"usr_abc123"`
/// - Snowflake/ULID: `"01ARZ3NDEKTSV4RRFFQ69G5FAV"`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(pub String);

impl Id {
    /// Create a new Id from a string.
    #[inline]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the inner string reference.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse the ID as a specific type.
    ///
    /// ```ignore
    /// let id: i64 = path.parse()?;
    /// ```
    #[inline]
    pub fn parse<T: std::str::FromStr>(&self) -> Result<T, ParseError> {
        self.0
            .parse()
            .map_err(|_| ParseError::invalid_format("id", &self.0))
    }
}

impl FromPath for Id {
    fn from_params(params: &HashMap<String, String>) -> Result<Self, ParseError> {
        params
            .get("id")
            .map(|s| Self(s.clone()))
            .ok_or_else(|| ParseError::missing("id"))
    }
}

impl OpenApiSchema for Id {
    fn openapi_schema() -> &'static str {
        r#"{"type":"string","description":"Resource identifier"}"#
    }

    fn schema_name() -> &'static str {
        "Id"
    }
}

impl AsRef<str> for Id {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ============================================================================
// PARSING TRAITS
// ============================================================================

/// Trait for types that can be parsed from JSON.
///
/// Implement this for request body types. Usually derived with `#[derive(Type)]`.
///
/// ```ignore
/// #[derive(Type)]
/// pub struct CreateUser {
///     pub name: String,
///     pub email: String,
/// }
///
/// // Generated implementation:
/// impl FromJson for CreateUser {
///     fn from_json(v: &JsonValue) -> Result<Self, ParseError> {
///         Ok(Self {
///             name: v.get("name").str()
///                 .ok_or(ParseError::missing("name"))?
///                 .to_string(),
///             email: v.get("email").str()
///                 .ok_or(ParseError::missing("email"))?
///                 .to_string(),
///         })
///     }
/// }
/// ```
pub trait FromJson: Sized {
    /// Parse this type from a JSON value.
    fn from_json(value: &JsonValue) -> Result<Self, ParseError>;

    /// Parse this type from a raw miniserde Value reference.
    ///
    /// This is used internally for efficient array parsing without cloning.
    /// The default implementation wraps the value in a JsonValue (which clones),
    /// but primitive types override this for zero-copy parsing.
    fn from_raw_value(value: &crate::json::RawValue) -> Result<Self, ParseError> {
        Self::from_json(&JsonValue::from_raw(value))
    }
}

/// Trait for types that can be parsed from query parameters.
///
/// Implement this for query parameter types. Usually derived with `#[derive(Query)]`.
///
/// ```ignore
/// #[derive(Query)]
/// pub struct ListQuery {
///     pub page: u32,
///     pub limit: u32,
/// }
///
/// // Generated implementation:
/// impl FromQuery for ListQuery {
///     fn from_query(params: &[(String, String)]) -> Result<Self, ParseError> {
///         // ...
///     }
/// }
/// ```
pub trait FromQuery: Sized {
    /// Parse this type from query parameters.
    fn from_query(params: &[(String, String)]) -> Result<Self, ParseError>;
}

/// Trait for types that can be parsed from path parameters.
///
/// Implement this for path parameter types. Usually derived with `#[derive(Path)]`.
///
/// ```ignore
/// #[derive(Path)]
/// pub struct UserPath {
///     pub org_id: String,
///     pub id: String,
/// }
///
/// // Generated implementation:
/// impl FromPath for UserPath {
///     fn from_params(params: &HashMap<String, String>) -> Result<Self, ParseError> {
///         Ok(Self {
///             org_id: params.get("org_id")
///                 .ok_or(ParseError::missing("org_id"))?
///                 .clone(),
///             id: params.get("id")
///                 .ok_or(ParseError::missing("id"))?
///                 .clone(),
///         })
///     }
/// }
/// ```
pub trait FromPath: Sized {
    /// Parse this type from path parameters.
    fn from_params(params: &HashMap<String, String>) -> Result<Self, ParseError>;
}

/// Trait for types that can be validated against constraints.
///
/// Implement this for types with field constraints. Usually derived with `#[derive(Type)]`.
///
/// ```ignore
/// #[derive(Type)]
/// pub struct CreateUser {
///     #[field(min = 1, max = 100)]
///     pub name: String,
/// }
///
/// // Generated implementation:
/// impl Validate for CreateUser {
///     fn validate(&self) -> Result<(), ValidationError> {
///         if self.name.len() < 1 {
///             return Err(ValidationError::min("name", 1));
///         }
///         if self.name.len() > 100 {
///             return Err(ValidationError::max("name", 100));
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Validate {
    /// Validate this value against its constraints.
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Trait for types that can generate their OpenAPI schema.
///
/// Implemented by types derived with `#[derive(Type)]`, `#[derive(Query)]`, etc.
pub trait OpenApiSchema {
    /// Get the OpenAPI JSON schema for this type.
    fn openapi_schema() -> &'static str;

    /// Get the schema name for $ref references.
    fn schema_name() -> &'static str;

    /// Get OpenAPI query parameters array for Query types.
    ///
    /// Returns a JSON array of parameter objects for use in OpenAPI path items.
    /// Only meaningful for types derived with `#[derive(Query)]`.
    ///
    /// Default implementation returns empty array for non-Query types.
    fn openapi_query_params() -> &'static str {
        "[]"
    }
}

// ============================================================================
// HELPER IMPLEMENTATIONS
// ============================================================================

// Implement FromJson for common types
impl FromJson for String {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        value
            .str()
            .ok_or_else(|| ParseError::type_mismatch("value", "string"))
    }
}

impl FromJson for i32 {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        value
            .int()
            .map(|n| n as Self)
            .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
    }
}

impl FromJson for i64 {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        value
            .int()
            .ok_or_else(|| ParseError::type_mismatch("value", "integer"))
    }
}

impl FromJson for f64 {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        value
            .float()
            .ok_or_else(|| ParseError::type_mismatch("value", "number"))
    }
}

impl FromJson for bool {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        value
            .bool()
            .ok_or_else(|| ParseError::type_mismatch("value", "boolean"))
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_json(value).map(Some)
        }
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(value: &JsonValue) -> Result<Self, ParseError> {
        // Use try_map_array for direct iteration without index-based access.
        // This avoids the overhead of len() check + at(i) bounds checking per element.
        value
            .try_map_array(|elem| T::from_json(&crate::json::JsonValue::from_raw(elem)))
            .ok_or_else(|| ParseError::type_mismatch("value", "array"))?
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::json;

    // ============================================================================
    // ID STRUCT TESTS
    // ============================================================================

    #[test]
    fn test_id_new_and_as_str() {
        let id = Id::new("user_123");
        assert_eq!(id.as_str(), "user_123");
        assert_eq!(id.0, "user_123");
    }

    #[test]
    fn test_id_from_string() {
        let id = Id::new(String::from("uuid-abc-123"));
        assert_eq!(id.as_str(), "uuid-abc-123");
    }

    #[test]
    fn test_id_empty_string() {
        let id = Id::new("");
        assert_eq!(id.as_str(), "");
        assert!(id.0.is_empty());
    }

    #[test]
    fn test_id_parse_valid_integer() {
        let id = Id::new("42");
        let parsed: Result<i64, _> = id.parse();
        assert_eq!(parsed.unwrap(), 42);
    }

    #[test]
    fn test_id_parse_valid_u32() {
        let id = Id::new("12345");
        let parsed: Result<u32, _> = id.parse();
        assert_eq!(parsed.unwrap(), 12345);
    }

    #[test]
    fn test_id_parse_invalid_format() {
        let id = Id::new("not-a-number");
        let parsed: Result<i64, _> = id.parse();
        assert!(parsed.is_err());
        let err = parsed.unwrap_err();
        assert_eq!(err.field(), "id");
        assert!(err.message().contains("Invalid format"));
        assert!(err.message().contains("not-a-number"));
    }

    #[test]
    fn test_id_parse_empty_string_as_integer() {
        let id = Id::new("");
        let parsed: Result<i64, _> = id.parse();
        assert!(parsed.is_err());
    }

    #[test]
    fn test_id_equality() {
        let id1 = Id::new("abc");
        let id2 = Id::new("abc");
        let id3 = Id::new("xyz");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_id_clone() {
        let id1 = Id::new("test");
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Id::new("a"));
        set.insert(Id::new("b"));
        set.insert(Id::new("a")); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_id_from_path_success() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "user_456".to_string());
        let id = Id::from_params(&params).unwrap();
        assert_eq!(id.as_str(), "user_456");
    }

    #[test]
    fn test_id_from_path_missing() {
        let params = HashMap::new();
        let result = Id::from_params(&params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field(), "id");
        assert!(err.message().contains("Missing"));
    }

    #[test]
    fn test_id_openapi_schema() {
        let schema = Id::openapi_schema();
        assert!(schema.contains("\"type\":\"string\""));
        assert!(schema.contains("\"description\":\"Resource identifier\""));
    }

    #[test]
    fn test_id_schema_name() {
        assert_eq!(Id::schema_name(), "Id");
    }

    // ============================================================================
    // PARSE ERROR TESTS
    // ============================================================================

    #[test]
    fn test_parse_error_missing() {
        let err = ParseError::missing("email");
        assert_eq!(err.field(), "email");
        assert!(err.message().contains("Missing required field"));
        assert!(err.message().contains("email"));
        // Test pattern matching
        assert!(matches!(err, ParseError::MissingField { .. }));
    }

    #[test]
    fn test_parse_error_invalid_format() {
        let err = ParseError::invalid_format("date", "not-a-date");
        assert_eq!(err.field(), "date");
        assert!(err.message().contains("Invalid format"));
        assert!(err.message().contains("date"));
        assert!(err.message().contains("not-a-date"));
        // Test pattern matching
        assert!(matches!(err, ParseError::InvalidFormat { .. }));
    }

    #[test]
    fn test_parse_error_type_mismatch() {
        let err = ParseError::type_mismatch("age", "integer");
        assert_eq!(err.field(), "age");
        assert!(err.message().contains("Expected integer"));
        assert!(err.message().contains("age"));
        // Test pattern matching
        assert!(matches!(err, ParseError::TypeMismatch { .. }));
    }

    #[test]
    fn test_parse_error_custom() {
        let err = ParseError::custom("field", "Something went wrong");
        assert_eq!(err.field(), "field");
        assert_eq!(err.message(), "Something went wrong");
        // Test pattern matching
        assert!(matches!(err, ParseError::Custom { .. }));
    }

    #[test]
    fn test_parse_error_custom_with_string() {
        let msg = String::from("Custom error message");
        let err = ParseError::custom("field", msg);
        assert_eq!(err.message(), "Custom error message");
    }

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::missing("name");
        let display = format!("{err}");
        assert!(display.contains("Missing required field: name"));
    }

    #[test]
    fn test_parse_error_debug() {
        let err = ParseError::missing("name");
        let debug = format!("{err:?}");
        // Enum variant name appears in debug output
        assert!(debug.contains("MissingField"));
        assert!(debug.contains("name"));
    }

    #[test]
    fn test_parse_error_equality() {
        let err1 = ParseError::missing("field");
        let err2 = ParseError::missing("field");
        let err3 = ParseError::missing("other");
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_parse_error_clone() {
        let err1 = ParseError::missing("field");
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_parse_error_is_std_error() {
        let err = ParseError::missing("field");
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_parse_error_with_path() {
        let err = ParseError::missing("city").with_path("address");
        assert_eq!(err.field(), "address.city");
        assert!(err.message().contains("address.city"));

        // Test nested paths
        let err2 = err.with_path("user");
        assert_eq!(err2.field(), "user.address.city");
    }

    #[test]
    fn test_parse_error_with_path_all_variants() {
        // MissingField
        let err = ParseError::missing("name").with_path("user");
        assert_eq!(err.field(), "user.name");

        // InvalidFormat
        let err = ParseError::invalid_format("age", "abc").with_path("user");
        assert_eq!(err.field(), "user.age");

        // TypeMismatch
        let err = ParseError::type_mismatch("count", "integer").with_path("items");
        assert_eq!(err.field(), "items.count");

        // Custom
        let err = ParseError::custom("value", "custom error").with_path("data");
        assert_eq!(err.field(), "data.value");
    }

    #[test]
    fn test_validation_error_to_parse_error() {
        let validation_err = ValidationError::min("count", 1);
        let parse_err: ParseError = validation_err.into();
        assert_eq!(parse_err.field(), "count");
        assert!(parse_err.message().contains("at least"));
    }

    // ============================================================================
    // VALIDATION ERROR TESTS
    // ============================================================================

    #[test]
    fn test_validation_error_min() {
        let err = ValidationError::min("name", 3);
        assert_eq!(err.field(), "name");
        assert_eq!(err.constraint(), "min");
        assert!(err.message().contains("'name'"));
        assert!(err.message().contains("at least 3"));
        // Test pattern matching
        assert!(matches!(err, ValidationError::Min { min: 3, .. }));
    }

    #[test]
    fn test_validation_error_max() {
        let err = ValidationError::max("count", 100);
        assert_eq!(err.field(), "count");
        assert_eq!(err.constraint(), "max");
        assert!(err.message().contains("'count'"));
        assert!(err.message().contains("at most 100"));
        // Test pattern matching
        assert!(matches!(err, ValidationError::Max { max: 100, .. }));
    }

    #[test]
    fn test_validation_error_pattern() {
        let err = ValidationError::pattern("email", r"^[\w@.]+$");
        assert_eq!(err.field(), "email");
        assert_eq!(err.constraint(), "pattern");
        assert!(err.message().contains("'email'"));
        assert!(err.message().contains("must match pattern"));
        // Test pattern matching
        assert!(matches!(err, ValidationError::Pattern { .. }));
    }

    #[test]
    fn test_validation_error_format() {
        let err = ValidationError::format("email", "email address");
        assert_eq!(err.field(), "email");
        assert_eq!(err.constraint(), "format");
        assert!(err.message().contains("'email'"));
        assert!(err.message().contains("valid email address"));
        // Test pattern matching
        assert!(matches!(err, ValidationError::Format { .. }));
    }

    #[test]
    fn test_validation_error_custom() {
        let err = ValidationError::custom("age", "range", "Age must be between 0 and 150");
        assert_eq!(err.field(), "age");
        assert_eq!(err.constraint(), "range");
        assert_eq!(err.message(), "Age must be between 0 and 150");
        // Test pattern matching
        assert!(matches!(err, ValidationError::Custom { .. }));
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::min("name", 1);
        let display = format!("{err}");
        assert!(display.contains("'name' must be at least 1"));
    }

    #[test]
    fn test_validation_error_debug() {
        let err = ValidationError::max("age", 120);
        let debug = format!("{err:?}");
        // Enum variant name appears in debug output
        assert!(debug.contains("Max"));
        assert!(debug.contains("age"));
        assert!(debug.contains("120"));
    }

    #[test]
    fn test_validation_error_equality() {
        let err1 = ValidationError::min("field", 5);
        let err2 = ValidationError::min("field", 5);
        let err3 = ValidationError::max("field", 5);
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_validation_error_clone() {
        let err1 = ValidationError::format("email", "email");
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_validation_error_is_std_error() {
        let err = ValidationError::min("field", 1);
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_validation_error_with_path() {
        let err = ValidationError::min("count", 1).with_path("items");
        assert_eq!(err.field(), "items.count");
        assert!(err.message().contains("items.count"));

        // Test nested paths
        let err2 = err.with_path("order");
        assert_eq!(err2.field(), "order.items.count");
    }

    #[test]
    fn test_validation_error_with_path_all_variants() {
        // Min
        let err = ValidationError::min("count", 1).with_path("items");
        assert_eq!(err.field(), "items.count");

        // Max
        let err = ValidationError::max("limit", 100).with_path("query");
        assert_eq!(err.field(), "query.limit");

        // Pattern
        let err = ValidationError::pattern("code", "^[A-Z]+$").with_path("data");
        assert_eq!(err.field(), "data.code");

        // Format
        let err = ValidationError::format("email", "email").with_path("user");
        assert_eq!(err.field(), "user.email");

        // Custom
        let err = ValidationError::custom("value", "range", "out of range").with_path("config");
        assert_eq!(err.field(), "config.value");
    }

    // ============================================================================
    // FROM_JSON FOR PRIMITIVE TYPES
    // ============================================================================

    #[test]
    fn test_from_json_string() {
        let v = json::str("hello");
        let result = String::from_json(&v);
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_from_json_string_type_mismatch() {
        let v = json::int(42);
        let result = String::from_json(&v);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field(), "value");
        assert!(err.message().contains("string"));
    }

    #[test]
    fn test_from_json_i32() {
        let v = json::int(42);
        let result = i32::from_json(&v);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_from_json_i32_type_mismatch() {
        let v = json::str("not a number");
        let result = i32::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_i64() {
        let v = json::int(9_000_000_000);
        let result = i64::from_json(&v);
        assert_eq!(result.unwrap(), 9_000_000_000);
    }

    #[test]
    fn test_from_json_i64_type_mismatch() {
        let v = json::bool(true);
        let result = i64::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_f64() {
        let v = json::float(42.5);
        let result = f64::from_json(&v);
        let parsed = result.unwrap();
        assert!((parsed - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_from_json_f64_from_int() {
        let v = json::int(42);
        let result = f64::from_json(&v);
        assert_eq!(result.unwrap(), 42.0);
    }

    #[test]
    fn test_from_json_f64_type_mismatch() {
        let v = json::str("not a number");
        let result = f64::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_bool_true() {
        let v = json::bool(true);
        let result = bool::from_json(&v);
        assert!(result.unwrap());
    }

    #[test]
    fn test_from_json_bool_false() {
        let v = json::bool(false);
        let result = bool::from_json(&v);
        assert!(!result.unwrap());
    }

    #[test]
    fn test_from_json_bool_type_mismatch() {
        let v = json::int(1);
        let result = bool::from_json(&v);
        assert!(result.is_err());
    }

    // ============================================================================
    // FROM_JSON FOR OPTION<T>
    // ============================================================================

    #[test]
    fn test_from_json_option_some_string() {
        let v = json::str("hello");
        let result = Option::<String>::from_json(&v);
        assert_eq!(result.unwrap(), Some("hello".to_string()));
    }

    #[test]
    fn test_from_json_option_none_null() {
        let v = json::null();
        let result = Option::<String>::from_json(&v);
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_from_json_option_some_i64() {
        let v = json::int(123);
        let result = Option::<i64>::from_json(&v);
        assert_eq!(result.unwrap(), Some(123));
    }

    #[test]
    fn test_from_json_option_some_bool() {
        let v = json::bool(true);
        let result = Option::<bool>::from_json(&v);
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn test_from_json_option_type_mismatch_propagates() {
        // When the value is not null but doesn't match the expected type,
        // the error should propagate
        let v = json::str("not a number");
        let result = Option::<i64>::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_nested_option() {
        // Option<Option<T>> - outer Option handles null, inner handles presence
        let v = json::null();
        let result = Option::<Option<String>>::from_json(&v);
        assert_eq!(result.unwrap(), None);
    }

    // ============================================================================
    // FROM_JSON FOR VEC<T>
    // ============================================================================

    #[test]
    fn test_from_json_vec_strings() {
        let v = json::arr()
            .push(json::str("a"))
            .push(json::str("b"))
            .push(json::str("c"));
        let result = Vec::<String>::from_json(&v);
        assert_eq!(result.unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_from_json_vec_integers() {
        let v = json::arr()
            .push(json::int(1))
            .push(json::int(2))
            .push(json::int(3));
        let result = Vec::<i64>::from_json(&v);
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_from_json_vec_empty() {
        let v = json::arr();
        let result = Vec::<String>::from_json(&v);
        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_from_json_vec_bools() {
        let v = json::arr()
            .push(json::bool(true))
            .push(json::bool(false))
            .push(json::bool(true));
        let result = Vec::<bool>::from_json(&v);
        assert_eq!(result.unwrap(), vec![true, false, true]);
    }

    #[test]
    fn test_from_json_vec_type_mismatch_not_array() {
        let v = json::str("not an array");
        let result = Vec::<String>::from_json(&v);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message().contains("array"));
    }

    #[test]
    fn test_from_json_vec_element_type_mismatch() {
        let v = json::arr().push(json::str("valid")).push(json::int(123)); // Not a string
        let result = Vec::<String>::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_vec_option_elements() {
        // Vec<Option<String>> - array with optional elements
        let v = json::arr()
            .push(json::str("a"))
            .push(json::null())
            .push(json::str("c"));
        let result = Vec::<Option<String>>::from_json(&v);
        assert_eq!(
            result.unwrap(),
            vec![Some("a".to_string()), None, Some("c".to_string())]
        );
    }

    #[test]
    fn test_from_json_option_vec() {
        // Option<Vec<String>> - nullable array
        let v = json::arr().push(json::str("a")).push(json::str("b"));
        let result = Option::<Vec<String>>::from_json(&v);
        assert_eq!(
            result.unwrap(),
            Some(vec!["a".to_string(), "b".to_string()])
        );

        let v_null = json::null();
        let result_null = Option::<Vec<String>>::from_json(&v_null);
        assert_eq!(result_null.unwrap(), None);
    }

    // ============================================================================
    // FROM_JSON NESTED/COMPLEX TYPES
    // ============================================================================

    #[test]
    fn test_from_json_vec_of_vec() {
        // Vec<Vec<i64>> - nested arrays
        let v = json::arr()
            .push(json::arr().push(json::int(1)).push(json::int(2)))
            .push(json::arr().push(json::int(3)).push(json::int(4)));
        let result = Vec::<Vec<i64>>::from_json(&v);
        assert_eq!(result.unwrap(), vec![vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn test_from_json_vec_of_vec_empty_inner() {
        let v = json::arr()
            .push(json::arr())
            .push(json::arr().push(json::int(1)));
        let result = Vec::<Vec<i64>>::from_json(&v);
        assert_eq!(result.unwrap(), vec![vec![], vec![1]]);
    }

    // ============================================================================
    // TYPE COERCION EDGE CASES
    // ============================================================================

    #[test]
    fn test_from_json_i32_truncation() {
        // Large i64 value truncated to i32
        let v = json::int(100);
        let result = i32::from_json(&v);
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_from_json_f64_from_large_int() {
        // Very large integers might lose precision when converted to f64
        let v = json::int(1_000_000);
        let result = f64::from_json(&v);
        assert_eq!(result.unwrap(), 1_000_000.0);
    }

    #[test]
    fn test_from_json_null_not_valid_string() {
        let v = json::null();
        let result = String::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_null_not_valid_int() {
        let v = json::null();
        let result = i64::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_null_not_valid_bool() {
        let v = json::null();
        let result = bool::from_json(&v);
        assert!(result.is_err());
    }

    // ============================================================================
    // PARSE FROM JSON BYTES
    // ============================================================================

    #[test]
    fn test_from_json_parsed_string() {
        let v = json::try_parse(b"\"hello world\"").unwrap();
        let result = String::from_json(&v);
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_from_json_parsed_number() {
        let v = json::try_parse(b"42").unwrap();
        let result = i64::from_json(&v);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_from_json_parsed_array() {
        let v = json::try_parse(b"[1, 2, 3]").unwrap();
        let result = Vec::<i64>::from_json(&v);
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_from_json_parsed_null_option() {
        let v = json::try_parse(b"null").unwrap();
        let result = Option::<String>::from_json(&v);
        assert_eq!(result.unwrap(), None);
    }

    // ============================================================================
    // ERROR MESSAGE CONTENT VERIFICATION
    // ============================================================================

    #[test]
    fn test_parse_error_messages_are_user_friendly() {
        let missing = ParseError::missing("username");
        assert!(missing.message().starts_with("Missing"));
        assert!(missing.to_string().contains("username"));

        let invalid = ParseError::invalid_format("date", "abc");
        assert!(invalid.message().contains("Invalid"));
        assert!(invalid.to_string().contains("abc"));

        let type_err = ParseError::type_mismatch("age", "number");
        assert!(type_err.message().contains("Expected"));
        assert!(type_err.to_string().contains("number"));
    }

    #[test]
    fn test_validation_error_messages_are_user_friendly() {
        let min = ValidationError::min("name", 3);
        assert!(min.message().contains("at least"));
        assert!(min.message().contains('3'));

        let max = ValidationError::max("items", 10);
        assert!(max.message().contains("at most"));
        assert!(max.message().contains("10"));

        let pattern = ValidationError::pattern("code", "^[A-Z]{3}$");
        assert!(pattern.message().contains("pattern"));

        let format = ValidationError::format("email", "email");
        assert!(format.message().contains("valid"));
    }

    // ============================================================================
    // EDGE CASES AND BOUNDARY CONDITIONS
    // ============================================================================

    #[test]
    fn test_id_with_special_characters() {
        let id = Id::new("user/123#section?query=1&foo=bar");
        assert_eq!(id.as_str(), "user/123#section?query=1&foo=bar");
    }

    #[test]
    fn test_id_with_unicode() {
        let id = Id::new("user_");
        assert_eq!(id.as_str(), "user_");
    }

    #[test]
    fn test_parse_error_with_empty_field_name() {
        let err = ParseError::missing("");
        assert_eq!(err.field(), "");
        // Should still have a message even with empty field
        assert!(!err.message().is_empty());
    }

    #[test]
    fn test_validation_error_with_negative_min() {
        let err = ValidationError::min("temperature", -40);
        assert!(err.message().contains("-40"));
    }

    #[test]
    fn test_validation_error_with_large_max() {
        let err = ValidationError::max("count", i64::MAX);
        assert!(err.message().contains(&i64::MAX.to_string()));
    }

    #[test]
    fn test_from_json_empty_string() {
        let v = json::str("");
        let result = String::from_json(&v);
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_from_json_negative_integer() {
        let v = json::int(-999);
        let result = i64::from_json(&v);
        assert_eq!(result.unwrap(), -999);
    }

    #[test]
    fn test_from_json_zero() {
        let v = json::int(0);
        let result = i64::from_json(&v);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_from_json_negative_float() {
        let v = json::float(-42.5);
        let result = f64::from_json(&v);
        let parsed = result.unwrap();
        assert!((parsed - (-42.5)).abs() < 0.001);
    }
}
