//! Runtime JSON parsing for Mongo-style filters.
//!
//! Parse user-provided JSON into `FilterExpr` at runtime, using the same
//! Mongo-style syntax as the compile-time macros.
//!
//! # Quick Start
//!
//! ```
//! use mik_sql::prelude::*;
//!
//! // Parse from JSON string
//! let filter = parse_filter(r#"{"name": "Alice", "age": {"$gte": 18}}"#).unwrap();
//! ```
//!
//! # Usage with mik-sdk Request
//!
//! ```ignore
//! use mik_sdk::prelude::*;
//! use mik_sql::{parse_filter, sql_read};
//!
//! fn search(query: Pagination, req: &Request) -> Response {
//!     // Extract body as text, early return if missing
//!     let body = ensure!(req.text(), 400, "Filter body required");
//!
//!     // Parse as filter, early return if invalid
//!     let filter = ensure!(parse_filter(body), 400, "Invalid filter");
//!
//!     // Merge with trusted filter, validate against whitelist
//!     let (sql, params) = ensure!(sql_read!(users {
//!         select: [id, name, email],
//!         filter: { active: true },
//!         merge: filter,
//!         allow: [name, email, status],
//!         page: query.page,
//!         limit: query.limit,
//!     }), 400, "Invalid filter field");
//!
//!     // Execute query...
//!     ok!({ "sql": sql })
//! }
//! ```
//!
//! # Supported Syntax
//!
//! | Syntax | Example | SQL |
//! |--------|---------|-----|
//! | Implicit `$eq` | `{"name": "Alice"}` | `name = 'Alice'` |
//! | Explicit operator | `{"age": {"$gte": 18}}` | `age >= 18` |
//! | Multiple fields | `{"a": 1, "b": 2}` | `a = 1 AND b = 2` |
//! | `$and` | `{"$and": [{...}, {...}]}` | `(...) AND (...)` |
//! | `$or` | `{"$or": [{...}, {...}]}` | `(...) OR (...)` |
//! | `$not` | `{"$not": {...}}` | `NOT (...)` |
//! | `$in` | `{"status": {"$in": ["a", "b"]}}` | `status IN ('a', 'b')` |
//! | `$between` | `{"age": {"$between": [18, 65]}}` | `age BETWEEN 18 AND 65` |

use super::types::{CompoundFilter, Filter, FilterExpr, Operator, Value};
use miniserde::json::{Number, Value as JsonValue};
use std::fmt;

/// Error type for JSON filter parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// Invalid JSON syntax or encoding.
    InvalidJson,
    /// Unknown operator (e.g., `$foo`).
    UnknownOperator(String),
    /// Expected an object but got something else.
    ExpectedObject,
    /// Expected an array but got something else.
    ExpectedArray,
    /// Expected a value but got something else.
    ExpectedValue,
    /// Field name is empty.
    EmptyFieldName,
    /// Filter object is empty.
    EmptyFilter,
    /// Invalid operator value type.
    InvalidOperatorValue {
        /// The operator that had the wrong value type.
        op: String,
        /// Description of what was expected.
        expected: &'static str,
    },
    /// $not requires exactly one condition.
    NotRequiresOneCondition,
}

/// Parse a Mongo-style filter from a JSON string.
///
/// This is a convenience function that calls [`FilterExpr::parse`].
///
/// # Example
///
/// ```
/// use mik_sql::prelude::*;
///
/// // Simple filter
/// let filter = parse_filter(r#"{"active": true}"#).unwrap();
///
/// // Complex filter with operators
/// let filter = parse_filter(r#"{
///     "status": {"$in": ["active", "pending"]},
///     "age": {"$gte": 18}
/// }"#).unwrap();
///
/// // Logical operators
/// let filter = parse_filter(r#"{
///     "$or": [
///         {"role": "admin"},
///         {"role": "moderator"}
///     ]
/// }"#).unwrap();
/// ```
///
/// # Errors
///
/// Returns `ParseError` if the JSON is invalid.
pub fn parse_filter(json_str: &str) -> Result<FilterExpr, ParseError> {
    FilterExpr::parse(json_str)
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => write!(f, "Invalid JSON syntax or encoding"),
            Self::UnknownOperator(op) => write!(f, "Unknown operator '{op}'"),
            Self::ExpectedObject => write!(f, "Expected JSON object"),
            Self::ExpectedArray => write!(f, "Expected JSON array"),
            Self::ExpectedValue => write!(f, "Expected a value"),
            Self::EmptyFieldName => write!(f, "Field name cannot be empty"),
            Self::EmptyFilter => write!(f, "Filter object cannot be empty"),
            Self::InvalidOperatorValue { op, expected } => {
                write!(f, "Operator '{op}' expects {expected}")
            },
            Self::NotRequiresOneCondition => {
                write!(f, "$not requires exactly one condition")
            },
        }
    }
}

impl std::error::Error for ParseError {}

impl Operator {
    /// Parse from Mongo-style operator string (e.g., "$eq", "$gte").
    ///
    /// Accepts both with and without the `$` prefix.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::Operator;
    ///
    /// assert_eq!(Operator::from_mongo("$eq"), Some(Operator::Eq));
    /// assert_eq!(Operator::from_mongo("gte"), Some(Operator::Gte));
    /// assert_eq!(Operator::from_mongo("$unknown"), None);
    /// ```
    #[must_use]
    pub fn from_mongo(s: &str) -> Option<Self> {
        // Strip leading $ if present
        let s = s.strip_prefix('$').unwrap_or(s);

        match s {
            "eq" => Some(Self::Eq),
            "ne" => Some(Self::Ne),
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            "in" => Some(Self::In),
            "nin" => Some(Self::NotIn),
            "like" => Some(Self::Like),
            "ilike" => Some(Self::ILike),
            "regex" => Some(Self::Regex),
            "startsWith" | "starts_with" => Some(Self::StartsWith),
            "endsWith" | "ends_with" => Some(Self::EndsWith),
            "contains" => Some(Self::Contains),
            "between" => Some(Self::Between),
            _ => None,
        }
    }
}

impl Value {
    /// Convert from miniserde JSON value.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::Value;
    /// use miniserde::json::{Value as JsonValue, Number};
    ///
    /// let json = JsonValue::String("hello".to_string());
    /// assert_eq!(Value::from_json(&json), Some(Value::String("hello".to_string())));
    ///
    /// let json = JsonValue::Number(Number::I64(42));
    /// assert_eq!(Value::from_json(&json), Some(Value::Int(42)));
    /// ```
    #[must_use]
    pub fn from_json(json: &JsonValue) -> Option<Self> {
        match json {
            JsonValue::Null => Some(Self::Null),
            JsonValue::Bool(b) => Some(Self::Bool(*b)),
            JsonValue::Number(n) => match n {
                Number::I64(i) => Some(Self::Int(*i)),
                Number::U64(u) => i64::try_from(*u).ok().map(Self::Int),
                Number::F64(f) => Some(Self::Float(*f)),
            },
            JsonValue::String(s) => Some(Self::String(s.clone())),
            JsonValue::Array(arr) => {
                let values: Option<Vec<Self>> = arr.iter().map(Self::from_json).collect();
                values.map(Self::Array)
            },
            JsonValue::Object(_) => None, // Objects are not valid filter values
        }
    }
}

impl FilterExpr {
    /// Parse a Mongo-style filter from a JSON string.
    ///
    /// This is the recommended way to parse user-provided filters.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::FilterExpr;
    ///
    /// let filter = FilterExpr::parse(r#"{"name": "Alice", "active": true}"#).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the JSON is invalid or malformed.
    pub fn parse(json_str: &str) -> Result<Self, ParseError> {
        let json: JsonValue =
            miniserde::json::from_str(json_str).map_err(|_| ParseError::InvalidJson)?;
        Self::from_json(&json)
    }

    /// Parse a Mongo-style filter from JSON bytes.
    ///
    /// Useful when working with raw request bodies.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::FilterExpr;
    ///
    /// let bytes = br#"{"status": {"$in": ["active", "pending"]}}"#;
    /// let filter = FilterExpr::parse_bytes(bytes).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the bytes are not valid UTF-8 or valid JSON.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let s = std::str::from_utf8(bytes).map_err(|_| ParseError::InvalidJson)?;
        Self::parse(s)
    }

    /// Parse a Mongo-style filter from a parsed JSON value.
    ///
    /// Use this when you already have a parsed `miniserde::json::Value`.
    /// For most cases, prefer [`parse`](Self::parse) or [`parse_bytes`](Self::parse_bytes).
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the JSON structure is invalid.
    pub fn from_json(json: &JsonValue) -> Result<Self, ParseError> {
        let obj = match json {
            JsonValue::Object(o) => o,
            _ => return Err(ParseError::ExpectedObject),
        };

        if obj.is_empty() {
            return Err(ParseError::EmptyFilter);
        }

        let mut filters = Vec::new();

        for (key, value) in obj {
            if key.is_empty() {
                return Err(ParseError::EmptyFieldName);
            }

            // Check for logical operators
            if key.starts_with('$') {
                match key.as_str() {
                    "$and" => {
                        let exprs = parse_filter_array(value)?;
                        filters.push(Self::Compound(CompoundFilter::and(exprs)));
                    },
                    "$or" => {
                        let exprs = parse_filter_array(value)?;
                        filters.push(Self::Compound(CompoundFilter::or(exprs)));
                    },
                    "$not" => {
                        let inner = Self::from_json(value)?;
                        filters.push(Self::Compound(CompoundFilter::not(inner)));
                    },
                    _ => return Err(ParseError::UnknownOperator(key.clone())),
                }
            } else {
                // Field filter
                let filter = parse_field_filter(key, value)?;
                filters.push(filter);
            }
        }

        // Combine multiple filters with implicit AND
        Ok(match filters.len() {
            0 => return Err(ParseError::EmptyFilter),
            1 => filters.remove(0),
            _ => Self::Compound(CompoundFilter::and(filters)),
        })
    }
}

/// Parse an array of filter expressions (for $and/$or).
fn parse_filter_array(json: &JsonValue) -> Result<Vec<FilterExpr>, ParseError> {
    let arr = match json {
        JsonValue::Array(a) => a,
        _ => return Err(ParseError::ExpectedArray),
    };

    arr.iter().map(FilterExpr::from_json).collect()
}

/// Parse a field filter: `{"$op": value}` or just `value` (implicit $eq).
fn parse_field_filter(field: &str, value: &JsonValue) -> Result<FilterExpr, ParseError> {
    // Check for operator syntax: {"$eq": value}
    if let JsonValue::Object(obj) = value {
        if let Some((op_key, op_value)) = obj.iter().next()
            && op_key.starts_with('$')
        {
            let op = Operator::from_mongo(op_key)
                .ok_or_else(|| ParseError::UnknownOperator(op_key.clone()))?;

            let val = parse_operator_value(op, op_value)?;

            return Ok(FilterExpr::Simple(Filter {
                field: field.to_string(),
                op,
                value: val,
            }));
        }
        // Not an operator object, treat as error
        return Err(ParseError::ExpectedValue);
    }

    // Implicit $eq
    let val = Value::from_json(value).ok_or(ParseError::ExpectedValue)?;
    Ok(FilterExpr::Simple(Filter {
        field: field.to_string(),
        op: Operator::Eq,
        value: val,
    }))
}

/// Parse the value for an operator, with type validation.
fn parse_operator_value(op: Operator, value: &JsonValue) -> Result<Value, ParseError> {
    match op {
        // Array operators require arrays
        Operator::In | Operator::NotIn => match value {
            JsonValue::Array(arr) => {
                let values: Option<Vec<Value>> = arr.iter().map(Value::from_json).collect();
                values
                    .map(Value::Array)
                    .ok_or_else(|| ParseError::InvalidOperatorValue {
                        op: format!("${op:?}").to_lowercase(),
                        expected: "array of values",
                    })
            },
            _ => Err(ParseError::InvalidOperatorValue {
                op: "$in/$nin".to_string(),
                expected: "array",
            }),
        },

        // Between requires array of exactly 2 values
        Operator::Between => match value {
            JsonValue::Array(arr) if arr.len() == 2 => {
                let values: Option<Vec<Value>> = arr.iter().map(Value::from_json).collect();
                values
                    .map(Value::Array)
                    .ok_or_else(|| ParseError::InvalidOperatorValue {
                        op: "$between".to_string(),
                        expected: "array of 2 values",
                    })
            },
            JsonValue::Array(_) => Err(ParseError::InvalidOperatorValue {
                op: "$between".to_string(),
                expected: "array of exactly 2 values",
            }),
            _ => Err(ParseError::InvalidOperatorValue {
                op: "$between".to_string(),
                expected: "array of 2 values",
            }),
        },

        // All other operators accept scalar values
        _ => Value::from_json(value).ok_or(ParseError::ExpectedValue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogicalOp;
    use miniserde::json::{self, Array as JsonArray};

    // =========================================================================
    // Operator::from_mongo tests
    // =========================================================================

    #[test]
    fn test_operator_from_mongo_with_prefix() {
        assert_eq!(Operator::from_mongo("$eq"), Some(Operator::Eq));
        assert_eq!(Operator::from_mongo("$ne"), Some(Operator::Ne));
        assert_eq!(Operator::from_mongo("$gt"), Some(Operator::Gt));
        assert_eq!(Operator::from_mongo("$gte"), Some(Operator::Gte));
        assert_eq!(Operator::from_mongo("$lt"), Some(Operator::Lt));
        assert_eq!(Operator::from_mongo("$lte"), Some(Operator::Lte));
        assert_eq!(Operator::from_mongo("$in"), Some(Operator::In));
        assert_eq!(Operator::from_mongo("$nin"), Some(Operator::NotIn));
        assert_eq!(Operator::from_mongo("$like"), Some(Operator::Like));
        assert_eq!(Operator::from_mongo("$ilike"), Some(Operator::ILike));
        assert_eq!(Operator::from_mongo("$regex"), Some(Operator::Regex));
        assert_eq!(Operator::from_mongo("$between"), Some(Operator::Between));
    }

    #[test]
    fn test_operator_from_mongo_without_prefix() {
        assert_eq!(Operator::from_mongo("eq"), Some(Operator::Eq));
        assert_eq!(Operator::from_mongo("gte"), Some(Operator::Gte));
    }

    #[test]
    fn test_operator_from_mongo_camel_case() {
        assert_eq!(
            Operator::from_mongo("$startsWith"),
            Some(Operator::StartsWith)
        );
        assert_eq!(
            Operator::from_mongo("$starts_with"),
            Some(Operator::StartsWith)
        );
        assert_eq!(Operator::from_mongo("$endsWith"), Some(Operator::EndsWith));
        assert_eq!(Operator::from_mongo("$ends_with"), Some(Operator::EndsWith));
    }

    #[test]
    fn test_operator_from_mongo_unknown() {
        assert_eq!(Operator::from_mongo("$unknown"), None);
        assert_eq!(Operator::from_mongo("$foo"), None);
    }

    // =========================================================================
    // Value::from_json tests
    // =========================================================================

    #[test]
    fn test_value_from_json_primitives() {
        assert_eq!(Value::from_json(&JsonValue::Null), Some(Value::Null));
        assert_eq!(
            Value::from_json(&JsonValue::Bool(true)),
            Some(Value::Bool(true))
        );
        assert_eq!(
            Value::from_json(&JsonValue::Number(Number::I64(42))),
            Some(Value::Int(42))
        );
        assert_eq!(
            Value::from_json(&JsonValue::Number(Number::F64(2.5))),
            Some(Value::Float(2.5))
        );
        assert_eq!(
            Value::from_json(&JsonValue::String("hello".into())),
            Some(Value::String("hello".into()))
        );
    }

    #[test]
    fn test_value_from_json_array() {
        let mut arr = JsonArray::new();
        arr.push(JsonValue::String("a".into()));
        arr.push(JsonValue::String("b".into()));
        let json_arr = JsonValue::Array(arr);
        assert_eq!(
            Value::from_json(&json_arr),
            Some(Value::Array(vec![
                Value::String("a".into()),
                Value::String("b".into()),
            ]))
        );
    }

    // =========================================================================
    // FilterExpr::from_json tests
    // =========================================================================

    #[test]
    fn test_simple_equality() {
        let json: JsonValue = json::from_str(r#"{"name": "Alice"}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Simple(Filter {
                ref field,
                op: Operator::Eq,
                value: Value::String(ref s),
            }) if field == "name" && s == "Alice"
        ));
    }

    #[test]
    fn test_explicit_operator() {
        let json: JsonValue = json::from_str(r#"{"age": {"$gte": 18}}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Simple(Filter {
                ref field,
                op: Operator::Gte,
                value: Value::Int(18),
            }) if field == "age"
        ));
    }

    #[test]
    fn test_multiple_fields_implicit_and() {
        let json: JsonValue = json::from_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::And,
                ..
            })
        ));
    }

    #[test]
    fn test_explicit_and() {
        let json: JsonValue =
            json::from_str(r#"{"$and": [{"name": "Alice"}, {"age": 30}]}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::And,
                ..
            })
        ));
    }

    #[test]
    fn test_explicit_or() {
        let json: JsonValue =
            json::from_str(r#"{"$or": [{"status": "active"}, {"status": "pending"}]}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::Or,
                ..
            })
        ));
    }

    #[test]
    fn test_explicit_not() {
        let json: JsonValue = json::from_str(r#"{"$not": {"deleted": true}}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::Not,
                ..
            })
        ));
    }

    #[test]
    fn test_in_operator() {
        let json: JsonValue = json::from_str(r#"{"status": {"$in": ["a", "b", "c"]}}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Simple(Filter {
                op: Operator::In,
                value: Value::Array(_),
                ..
            })
        ));
    }

    #[test]
    fn test_between_operator() {
        let json: JsonValue = json::from_str(r#"{"age": {"$between": [18, 65]}}"#).unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Simple(Filter {
                op: Operator::Between,
                value: Value::Array(ref arr),
                ..
            }) if arr.len() == 2
        ));
    }

    #[test]
    fn test_nested_logical() {
        let json: JsonValue = json::from_str(
            r#"{"$and": [{"active": true}, {"$or": [{"role": "admin"}, {"role": "mod"}]}]}"#,
        )
        .unwrap();
        let filter = FilterExpr::from_json(&json).unwrap();

        assert!(matches!(
            filter,
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::And,
                ..
            })
        ));
    }

    // =========================================================================
    // Error cases
    // =========================================================================

    #[test]
    fn test_error_not_object() {
        let json: JsonValue = json::from_str(r"[1, 2, 3]").unwrap();
        assert!(matches!(
            FilterExpr::from_json(&json),
            Err(ParseError::ExpectedObject)
        ));
    }

    #[test]
    fn test_error_empty_filter() {
        let json: JsonValue = json::from_str(r"{}").unwrap();
        assert!(matches!(
            FilterExpr::from_json(&json),
            Err(ParseError::EmptyFilter)
        ));
    }

    #[test]
    fn test_error_unknown_operator() {
        let json: JsonValue = json::from_str(r#"{"field": {"$foo": 1}}"#).unwrap();
        assert!(matches!(
            FilterExpr::from_json(&json),
            Err(ParseError::UnknownOperator(_))
        ));
    }

    #[test]
    fn test_error_between_wrong_count() {
        let json: JsonValue = json::from_str(r#"{"age": {"$between": [18]}}"#).unwrap();
        assert!(matches!(
            FilterExpr::from_json(&json),
            Err(ParseError::InvalidOperatorValue { .. })
        ));
    }

    #[test]
    fn test_error_in_not_array() {
        let json: JsonValue = json::from_str(r#"{"status": {"$in": "not-array"}}"#).unwrap();
        assert!(matches!(
            FilterExpr::from_json(&json),
            Err(ParseError::InvalidOperatorValue { .. })
        ));
    }
}
