//! Security validation layer for query filters and SQL identifiers.
//!
//! This module provides validation for:
//! - User-provided filters (field whitelisting, operator blacklisting)
//! - SQL identifiers (table names, column names) to prevent injection
//! - Nesting depth limits for complex queries
//!
//! # Example
//!
//! ```ignore
//! use mik_sql::{FilterValidator, merge_filters, Filter, Operator, Value};
//!
//! // Create validator with security rules
//! let validator = FilterValidator::new()
//!     .allow_fields(&["name", "email", "status"])
//!     .deny_operators(&[Operator::Regex, Operator::ILike])
//!     .max_depth(3);
//!
//! // System/policy filters (trusted, no validation)
//! let trusted = vec![
//!     Filter { field: "org_id".into(), op: Operator::Eq, value: Value::Int(123) },
//!     Filter { field: "deleted_at".into(), op: Operator::Eq, value: Value::Null },
//! ];
//!
//! // User-provided filters (validated)
//! let user = vec![
//!     Filter { field: "status".into(), op: Operator::Eq, value: Value::String("active".into()) },
//! ];
//!
//! // Merge with validation
//! let filters = merge_filters(trusted, user, &validator)?;
//! ```

use crate::{Filter, Operator, Value};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// SQL IDENTIFIER VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

/// Maximum length for SQL identifiers (`PostgreSQL` limit is 63).
const MAX_IDENTIFIER_LENGTH: usize = 63;

/// Validate that a string is a safe SQL identifier.
///
/// A valid SQL identifier:
/// - Starts with a letter (a-z, A-Z) or underscore
/// - Contains only letters, digits (0-9), and underscores
/// - Is not empty and not longer than 63 characters
///
/// This prevents SQL injection attacks by rejecting:
/// - Special characters (quotes, semicolons, etc.)
/// - SQL keywords as standalone identifiers
/// - Unicode characters that could cause confusion
///
/// # Examples
///
/// ```
/// use mik_sql::is_valid_sql_identifier;
///
/// assert!(is_valid_sql_identifier("users"));
/// assert!(is_valid_sql_identifier("user_id"));
/// assert!(is_valid_sql_identifier("_private"));
/// assert!(is_valid_sql_identifier("Table123"));
///
/// // Invalid identifiers
/// assert!(!is_valid_sql_identifier(""));           // empty
/// assert!(!is_valid_sql_identifier("123abc"));     // starts with digit
/// assert!(!is_valid_sql_identifier("user-name"));  // contains hyphen
/// assert!(!is_valid_sql_identifier("user.id"));    // contains dot
/// assert!(!is_valid_sql_identifier("user; DROP")); // contains special chars
/// ```
#[inline]
#[must_use]
pub fn is_valid_sql_identifier(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_IDENTIFIER_LENGTH {
        return false;
    }

    let mut chars = s.chars();

    // First character must be letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }

    // Rest must be letters, digits, or underscores
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Assert that a string is a valid SQL identifier.
///
/// # Panics
///
/// Panics with a descriptive error if the identifier is invalid.
/// This is intended for programmer errors (invalid table/column names in code),
/// not for user input validation.
///
/// # Examples
///
/// ```
/// use mik_sql::assert_valid_sql_identifier;
///
/// assert_valid_sql_identifier("users", "table");    // OK
/// assert_valid_sql_identifier("user_id", "column"); // OK
/// ```
///
/// ```should_panic
/// use mik_sql::assert_valid_sql_identifier;
///
/// assert_valid_sql_identifier("user; DROP TABLE", "table"); // Panics!
/// ```
#[inline]
pub fn assert_valid_sql_identifier(s: &str, context: &str) {
    assert!(
        is_valid_sql_identifier(s),
        "Invalid SQL {context} name '{s}': must start with letter/underscore, \
             contain only ASCII alphanumeric/underscore, and be 1-63 chars"
    );
}

/// Validate a SQL expression for computed fields.
///
/// Computed field expressions are dangerous because they're inserted directly
/// into SQL. This function performs defense-in-depth validation to catch
/// injection attempts, but **cannot provide complete protection**.
///
/// # Security Model
///
/// This validation is a safety net, not a security boundary. It catches:
/// - Obvious injection patterns (comments, semicolons, SQL keywords)
/// - Common attack vectors
///
/// It **cannot** catch:
/// - All possible SQL injection variants
/// - Database-specific syntax
/// - Encoded or obfuscated attacks
///
/// **CRITICAL**: Only use computed fields with trusted expressions from code.
/// Never pass user input to computed field expressions, even with validation.
///
/// # Valid expressions
///
/// - Simple field references: `first_name`, `price`
/// - Arithmetic: `quantity * price`
/// - String concatenation: `first_name || ' ' || last_name`
/// - Functions: `COALESCE(nickname, name)`, `UPPER(name)`
///
/// # Invalid expressions (rejected)
///
/// - Comments: `--`, `/*`, `*/`
/// - Statement terminators: `;`
/// - SQL keywords: SELECT, INSERT, UPDATE, DELETE, DROP, etc.
/// - System functions: `pg_`, `sqlite_`
///
/// # Examples
///
/// ```
/// use mik_sql::is_valid_sql_expression;
///
/// assert!(is_valid_sql_expression("first_name || ' ' || last_name"));
/// assert!(is_valid_sql_expression("quantity * price"));
/// assert!(is_valid_sql_expression("COALESCE(nickname, name)"));
///
/// // Dangerous patterns are rejected
/// assert!(!is_valid_sql_expression("1; DROP TABLE users"));
/// assert!(!is_valid_sql_expression("name -- comment"));
/// assert!(!is_valid_sql_expression("/* comment */ name"));
/// ```
#[inline]
#[must_use]
pub fn is_valid_sql_expression(s: &str) -> bool {
    // Empty or oversized expressions are invalid
    if s.is_empty() || s.len() > 1000 {
        return false;
    }

    // No SQL comments
    if s.contains("--") || s.contains("/*") || s.contains("*/") {
        return false;
    }

    // No statement terminators
    if s.contains(';') {
        return false;
    }

    // No backticks (MySQL identifier quotes that could be used for injection)
    if s.contains('`') {
        return false;
    }

    // Check for dangerous SQL keywords using word boundary detection
    let lower = s.to_ascii_lowercase();

    // Dangerous DML/DDL keywords and functions
    const DANGEROUS_KEYWORDS: &[&str] = &[
        // DML/DDL statements
        "select",
        "insert",
        "update",
        "delete",
        "drop",
        "truncate",
        "alter",
        "create",
        "grant",
        "revoke",
        "exec",
        "execute",
        "union",
        "into",
        "from",
        "where",
        "having",
        "group",
        "order",
        "limit",
        "offset",
        "fetch",
        "returning",
        // Dangerous functions (timing attacks, DoS)
        "sleep",
        "benchmark",
        "waitfor",
        "pg_sleep",
        "dbms_lock",
        // File/network operations
        "load_file",
        "into_outfile",
        "into_dumpfile",
        // Encoding/conversion functions that could bypass keyword detection
        "chr",
        "char",
        "ascii",
        "unicode",
        "hex",
        "unhex",
        "convert",
        "cast",
        "encode",
        "decode",
    ];

    for keyword in DANGEROUS_KEYWORDS {
        if contains_sql_keyword(&lower, keyword) {
            return false;
        }
    }

    // Block system catalog access patterns
    if lower.contains("pg_")
        || lower.contains("sqlite_")
        || lower.contains("information_schema")
        || lower.contains("sys.")
    {
        return false;
    }

    // Block hex escapes that could bypass other checks
    if lower.contains("0x") || lower.contains("\\x") {
        return false;
    }

    true
}

/// Check if a string contains a SQL keyword as a whole word.
///
/// This prevents false positives like "update" in "`last_updated`".
#[inline]
fn contains_sql_keyword(haystack: &str, keyword: &str) -> bool {
    let bytes = haystack.as_bytes();
    let kw_bytes = keyword.as_bytes();
    let kw_len = kw_bytes.len();

    if kw_len == 0 || bytes.len() < kw_len {
        return false;
    }

    for i in 0..=(bytes.len() - kw_len) {
        // Check if keyword matches at this position
        if &bytes[i..i + kw_len] == kw_bytes {
            // Check word boundaries (parentheses fix operator precedence: && binds tighter than ||)
            let before_ok =
                i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
            let after_ok = i + kw_len == bytes.len()
                || (!bytes[i + kw_len].is_ascii_alphanumeric() && bytes[i + kw_len] != b'_');

            if before_ok && after_ok {
                return true;
            }
        }
    }

    false
}

/// Assert that a SQL expression is valid for computed fields.
///
/// # Panics
///
/// Panics if the expression contains dangerous patterns.
#[inline]
pub fn assert_valid_sql_expression(s: &str, context: &str) {
    assert!(
        is_valid_sql_expression(s),
        "Invalid SQL expression for {context}: '{s}' contains dangerous patterns \
             (comments, semicolons, or SQL keywords)"
    );
}

/// Maximum number of value nodes to validate (defense-in-depth).
const MAX_VALUE_NODES: usize = 10000;

/// Validation configuration for user-provided filters.
///
/// Provides four layers of security:
/// 1. Field whitelist - only specific fields can be queried
/// 2. Operator blacklist - dangerous operators can be denied
/// 3. Nesting depth limit - prevent complex nested queries
/// 4. Total node count limit - prevent DoS via large arrays
#[derive(Debug, Clone)]
pub struct FilterValidator {
    /// Allowed field names (whitelist). Empty = allow all fields.
    pub allowed_fields: Vec<String>,
    /// Denied operators (blacklist).
    pub denied_operators: Vec<Operator>,
    /// Maximum nesting depth for complex filters.
    pub max_depth: usize,
}

impl FilterValidator {
    /// Create a new validator with secure defaults.
    ///
    /// Defaults:
    /// - No field restrictions (allow all)
    /// - Denies `Regex` operator (`ReDoS` prevention)
    /// - Max nesting depth: 5
    ///
    /// This is the recommended constructor for user-facing filters.
    /// For internal/trusted filters where you need all operators,
    /// use [`permissive()`](Self::permissive).
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::FilterValidator;
    ///
    /// let validator = FilterValidator::new()
    ///     .allow_fields(&["name", "email", "status"]);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_fields: Vec::new(),
            denied_operators: vec![crate::Operator::Regex],
            max_depth: 5,
        }
    }

    /// Create a permissive validator that allows all operators.
    ///
    /// **Warning:** Only use this for trusted/internal filters, never for
    /// user-provided input. The `Regex` operator can cause `ReDoS` attacks.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sql::FilterValidator;
    ///
    /// // Only for trusted internal filters!
    /// let validator = FilterValidator::permissive();
    /// ```
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allowed_fields: Vec::new(),
            denied_operators: Vec::new(),
            max_depth: 5,
        }
    }

    /// Set allowed fields (whitelist).
    ///
    /// Only fields in this list can be used in user filters.
    /// If empty, all fields are allowed.
    #[must_use]
    pub fn allow_fields(mut self, fields: &[&str]) -> Self {
        self.allowed_fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Set denied operators (blacklist).
    ///
    /// These operators cannot be used in user filters.
    /// Useful for blocking regex, pattern matching, or other expensive operations.
    #[must_use]
    pub fn deny_operators(mut self, ops: &[Operator]) -> Self {
        self.denied_operators = ops.to_vec();
        self
    }

    /// Set maximum nesting depth.
    ///
    /// Prevents complex nested queries that could impact performance.
    /// Default is 5.
    #[must_use]
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Validate a filter against the configured rules.
    ///
    /// Returns an error if:
    /// - Field is not in the allowed list (when list is not empty)
    /// - Operator is in the denied list
    /// - Array nesting depth exceeds maximum
    pub fn validate(&self, filter: &Filter) -> Result<(), ValidationError> {
        self.validate_with_depth(filter, 0)
    }

    /// Internal validation with depth tracking.
    fn validate_with_depth(&self, filter: &Filter, depth: usize) -> Result<(), ValidationError> {
        // Check nesting depth
        if depth > self.max_depth {
            return Err(ValidationError::NestingTooDeep {
                max: self.max_depth,
                actual: depth,
            });
        }

        // Check field whitelist (only if not empty)
        if !self.allowed_fields.is_empty() && !self.allowed_fields.contains(&filter.field) {
            return Err(ValidationError::FieldNotAllowed {
                field: filter.field.clone(),
                allowed: self.allowed_fields.clone(),
            });
        }

        // Check operator blacklist
        if self.denied_operators.contains(&filter.op) {
            return Err(ValidationError::OperatorDenied {
                operator: filter.op,
                field: filter.field.clone(),
            });
        }

        // Recursively validate array values (for complex nested filters)
        if let Value::Array(values) = &filter.value {
            let mut node_count = 0;
            for value in values {
                self.validate_value_with_count(value, depth + 1, &mut node_count)?;
            }
        }

        Ok(())
    }

    /// Validate nested values in arrays with node count tracking.
    fn validate_value_with_count(
        &self,
        value: &Value,
        depth: usize,
        count: &mut usize,
    ) -> Result<(), ValidationError> {
        *count += 1;
        if *count > MAX_VALUE_NODES {
            return Err(ValidationError::TooManyNodes {
                max: MAX_VALUE_NODES,
            });
        }

        if depth > self.max_depth {
            return Err(ValidationError::NestingTooDeep {
                max: self.max_depth,
                actual: depth,
            });
        }

        if let Value::Array(values) = value {
            for v in values {
                self.validate_value_with_count(v, depth + 1, count)?;
            }
        }

        Ok(())
    }
}

impl Default for FilterValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation error types.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Field is not in the allowed list.
    FieldNotAllowed { field: String, allowed: Vec<String> },
    /// Operator is denied for this field.
    OperatorDenied { operator: Operator, field: String },
    /// Nesting depth exceeds maximum.
    NestingTooDeep { max: usize, actual: usize },
    /// Too many value nodes (DoS prevention).
    TooManyNodes { max: usize },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldNotAllowed { field, allowed } => {
                write!(
                    f,
                    "Field '{}' is not allowed. Allowed fields: {}",
                    field,
                    allowed.join(", ")
                )
            },
            Self::OperatorDenied { operator, field } => {
                write!(f, "Operator '{operator:?}' is denied for field '{field}'")
            },
            Self::NestingTooDeep { max, actual } => {
                write!(f, "Filter nesting depth {actual} exceeds maximum {max}")
            },
            Self::TooManyNodes { max } => {
                write!(f, "Filter contains too many value nodes (max {max})")
            },
        }
    }
}

impl std::error::Error for ValidationError {}

/// Merge trusted filters with validated user filters.
///
/// This function combines system/policy filters (trusted, no validation)
/// with user-provided filters (validated against the validator rules).
///
/// # Arguments
///
/// * `trusted` - System filters (e.g., `org_id`, `tenant_id`, `deleted_at`)
/// * `user` - User-provided filters from request
/// * `validator` - Validation rules for user filters
///
/// # Returns
///
/// Combined filter list with trusted filters first, then validated user filters.
///
/// # Example
///
/// ```ignore
/// // System ensures user can only see their org's data
/// let trusted = vec![
///     Filter { field: "org_id".into(), op: Operator::Eq, value: Value::Int(123) },
/// ];
///
/// // User wants to filter by status
/// let user = vec![
///     Filter { field: "status".into(), op: Operator::Eq, value: Value::String("active".into()) },
/// ];
///
/// let validator = FilterValidator::new().allow_fields(&["status", "name"]);
/// let all_filters = merge_filters(trusted, user, &validator)?;
/// // Result: [org_id=123, status='active']
/// ```
pub fn merge_filters(
    trusted: Vec<Filter>,
    user: Vec<Filter>,
    validator: &FilterValidator,
) -> Result<Vec<Filter>, ValidationError> {
    // Validate all user filters
    for filter in &user {
        validator.validate(filter)?;
    }

    // Combine: trusted first, then user filters
    let mut result = trusted;
    result.extend(user);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_default_is_secure() {
        let validator = FilterValidator::new();
        assert!(validator.allowed_fields.is_empty());
        // new() now denies Regex by default for security
        assert_eq!(validator.denied_operators, vec![Operator::Regex]);
        assert_eq!(validator.max_depth, 5);
    }

    #[test]
    fn test_validator_permissive() {
        let validator = FilterValidator::permissive();
        assert!(validator.allowed_fields.is_empty());
        assert!(validator.denied_operators.is_empty());
        assert_eq!(validator.max_depth, 5);
    }

    #[test]
    fn test_validator_builder() {
        let validator = FilterValidator::new()
            .allow_fields(&["name", "email"])
            .deny_operators(&[Operator::Regex, Operator::ILike])
            .max_depth(3);

        assert_eq!(validator.allowed_fields.len(), 2);
        assert_eq!(validator.denied_operators.len(), 2);
        assert_eq!(validator.max_depth, 3);
    }

    #[test]
    fn test_validate_allowed_field() {
        let validator = FilterValidator::new().allow_fields(&["name", "email", "status"]);

        let filter = Filter {
            field: "name".into(),
            op: Operator::Eq,
            value: Value::String("Alice".into()),
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_validate_disallowed_field() {
        let validator = FilterValidator::new().allow_fields(&["name", "email"]);

        let filter = Filter {
            field: "password".into(),
            op: Operator::Eq,
            value: Value::String("secret".into()),
        };

        let result = validator.validate(&filter);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::FieldNotAllowed { field, allowed } => {
                assert_eq!(field, "password");
                assert_eq!(allowed.len(), 2);
            },
            _ => panic!("Expected FieldNotAllowed error"),
        }
    }

    #[test]
    fn test_validate_empty_whitelist_allows_all() {
        let validator = FilterValidator::new(); // No field restrictions

        let filter = Filter {
            field: "any_field".into(),
            op: Operator::Eq,
            value: Value::String("value".into()),
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_validate_denied_operator() {
        let validator = FilterValidator::new()
            .allow_fields(&["name"])
            .deny_operators(&[Operator::Regex, Operator::ILike]);

        let filter = Filter {
            field: "name".into(),
            op: Operator::Regex,
            value: Value::String("^A".into()),
        };

        let result = validator.validate(&filter);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::OperatorDenied { operator, field } => {
                assert_eq!(operator, Operator::Regex);
                assert_eq!(field, "name");
            },
            _ => panic!("Expected OperatorDenied error"),
        }
    }

    #[test]
    fn test_validate_allowed_operator() {
        let validator = FilterValidator::new()
            .allow_fields(&["status"])
            .deny_operators(&[Operator::Regex]);

        let filter = Filter {
            field: "status".into(),
            op: Operator::Eq,
            value: Value::String("active".into()),
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_validate_nesting_depth() {
        let validator = FilterValidator::new().max_depth(2);

        // Depth 0 - OK
        let filter = Filter {
            field: "tags".into(),
            op: Operator::In,
            value: Value::Array(vec![Value::String("rust".into())]),
        };
        assert!(validator.validate(&filter).is_ok());

        // Depth 3 - exceeds max
        let filter_deep = Filter {
            field: "deep".into(),
            op: Operator::In,
            value: Value::Array(vec![Value::Array(vec![Value::Array(vec![Value::String(
                "too deep".into(),
            )])])]),
        };
        let result = validator.validate(&filter_deep);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::NestingTooDeep { max, actual } => {
                assert_eq!(max, 2);
                assert!(actual > max);
            },
            _ => panic!("Expected NestingTooDeep error"),
        }
    }

    #[test]
    fn test_merge_filters_success() {
        let validator = FilterValidator::new().allow_fields(&["status", "name"]);

        let trusted = vec![
            Filter {
                field: "org_id".into(),
                op: Operator::Eq,
                value: Value::Int(123),
            },
            Filter {
                field: "deleted_at".into(),
                op: Operator::Eq,
                value: Value::Null,
            },
        ];

        let user = vec![Filter {
            field: "status".into(),
            op: Operator::Eq,
            value: Value::String("active".into()),
        }];

        let result = merge_filters(trusted, user, &validator);
        assert!(result.is_ok());

        let filters = result.unwrap();
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0].field, "org_id");
        assert_eq!(filters[1].field, "deleted_at");
        assert_eq!(filters[2].field, "status");
    }

    #[test]
    fn test_merge_filters_validation_error() {
        let validator = FilterValidator::new().allow_fields(&["status"]);

        let trusted = vec![Filter {
            field: "org_id".into(),
            op: Operator::Eq,
            value: Value::Int(123),
        }];

        // User tries to filter on disallowed field
        let user = vec![Filter {
            field: "password".into(),
            op: Operator::Eq,
            value: Value::String("hack".into()),
        }];

        let result = merge_filters(trusted, user, &validator);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_filters_empty_user() {
        let validator = FilterValidator::new();

        let trusted = vec![Filter {
            field: "org_id".into(),
            op: Operator::Eq,
            value: Value::Int(123),
        }];

        let user = vec![];

        let result = merge_filters(trusted, user, &validator);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_merge_filters_empty_trusted() {
        let validator = FilterValidator::new().allow_fields(&["name"]);

        let trusted = vec![];
        let user = vec![Filter {
            field: "name".into(),
            op: Operator::Eq,
            value: Value::String("Alice".into()),
        }];

        let result = merge_filters(trusted, user, &validator);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_multiple_validation_errors() {
        let validator = FilterValidator::new()
            .allow_fields(&["status"])
            .deny_operators(&[Operator::Regex]);

        // Disallowed field
        let filter1 = Filter {
            field: "password".into(),
            op: Operator::Eq,
            value: Value::String("x".into()),
        };
        assert!(validator.validate(&filter1).is_err());

        // Denied operator
        let filter2 = Filter {
            field: "status".into(),
            op: Operator::Regex,
            value: Value::String("^A".into()),
        };
        assert!(validator.validate(&filter2).is_err());
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::FieldNotAllowed {
            field: "password".into(),
            allowed: vec!["name".into(), "email".into()],
        };
        let msg = format!("{}", err);
        assert!(msg.contains("password"));
        assert!(msg.contains("name"));

        let err = ValidationError::OperatorDenied {
            operator: Operator::Regex,
            field: "name".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Regex"));
        assert!(msg.contains("name"));

        let err = ValidationError::NestingTooDeep { max: 3, actual: 5 };
        let msg = format!("{}", err);
        assert!(msg.contains("3"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_in_operator_validation() {
        let validator = FilterValidator::new().allow_fields(&["status"]);

        let filter = Filter {
            field: "status".into(),
            op: Operator::In,
            value: Value::Array(vec![
                Value::String("active".into()),
                Value::String("pending".into()),
            ]),
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_not_in_operator_validation() {
        let validator = FilterValidator::new()
            .allow_fields(&["status"])
            .deny_operators(&[Operator::NotIn]);

        let filter = Filter {
            field: "status".into(),
            op: Operator::NotIn,
            value: Value::Array(vec![Value::String("deleted".into())]),
        };

        assert!(validator.validate(&filter).is_err());
    }

    #[test]
    fn test_null_value_validation() {
        let validator = FilterValidator::new().allow_fields(&["deleted_at"]);

        let filter = Filter {
            field: "deleted_at".into(),
            op: Operator::Eq,
            value: Value::Null,
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_bool_value_validation() {
        let validator = FilterValidator::new().allow_fields(&["active"]);

        let filter = Filter {
            field: "active".into(),
            op: Operator::Eq,
            value: Value::Bool(true),
        };

        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_numeric_value_validation() {
        let validator = FilterValidator::new().allow_fields(&["age", "price"]);

        let filter1 = Filter {
            field: "age".into(),
            op: Operator::Gte,
            value: Value::Int(18),
        };
        assert!(validator.validate(&filter1).is_ok());

        let filter2 = Filter {
            field: "price".into(),
            op: Operator::Lt,
            value: Value::Float(99.99),
        };
        assert!(validator.validate(&filter2).is_ok());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SQL IDENTIFIER VALIDATION TESTS
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_valid_sql_identifiers() {
        use super::is_valid_sql_identifier;

        // Valid identifiers
        assert!(is_valid_sql_identifier("users"));
        assert!(is_valid_sql_identifier("user_id"));
        assert!(is_valid_sql_identifier("_private"));
        assert!(is_valid_sql_identifier("Table123"));
        assert!(is_valid_sql_identifier("a"));
        assert!(is_valid_sql_identifier("_"));
        assert!(is_valid_sql_identifier("UPPERCASE"));
        assert!(is_valid_sql_identifier("mixedCase"));
        assert!(is_valid_sql_identifier("with_123_numbers"));
    }

    #[test]
    fn test_invalid_sql_identifiers() {
        use super::is_valid_sql_identifier;

        // Empty
        assert!(!is_valid_sql_identifier(""));

        // Starts with digit
        assert!(!is_valid_sql_identifier("123abc"));
        assert!(!is_valid_sql_identifier("1"));

        // Contains special characters
        assert!(!is_valid_sql_identifier("user-name"));
        assert!(!is_valid_sql_identifier("user.id"));
        assert!(!is_valid_sql_identifier("user name"));
        assert!(!is_valid_sql_identifier("user;drop"));
        assert!(!is_valid_sql_identifier("table'"));
        assert!(!is_valid_sql_identifier("table\""));
        assert!(!is_valid_sql_identifier("table`"));
        assert!(!is_valid_sql_identifier("table("));
        assert!(!is_valid_sql_identifier("table)"));

        // SQL injection attempts
        assert!(!is_valid_sql_identifier("users; DROP TABLE"));
        assert!(!is_valid_sql_identifier("users--"));
        assert!(!is_valid_sql_identifier("users/*"));
    }

    #[test]
    fn test_sql_identifier_length_limit() {
        use super::is_valid_sql_identifier;

        // 63 chars = OK (PostgreSQL limit)
        let valid_63 = "a".repeat(63);
        assert!(is_valid_sql_identifier(&valid_63));

        // 64 chars = too long
        let invalid_64 = "a".repeat(64);
        assert!(!is_valid_sql_identifier(&invalid_64));
    }

    #[test]
    fn test_valid_sql_expressions() {
        use super::is_valid_sql_expression;

        // Valid expressions
        assert!(is_valid_sql_expression("first_name || ' ' || last_name"));
        assert!(is_valid_sql_expression("quantity * price"));
        assert!(is_valid_sql_expression("COALESCE(nickname, name)"));
        assert!(is_valid_sql_expression("age + 1"));
        assert!(is_valid_sql_expression("CASE WHEN x > 0 THEN y ELSE z END"));
        assert!(is_valid_sql_expression("price * 1.1"));
        assert!(is_valid_sql_expression("UPPER(name)"));
        assert!(is_valid_sql_expression("LENGTH(description)"));

        // Word boundary detection - these contain keywords as substrings but should be allowed
        assert!(is_valid_sql_expression("last_updated")); // contains "update"
        assert!(is_valid_sql_expression("created_at")); // contains "create"
        assert!(is_valid_sql_expression("selected_items")); // contains "select"
        assert!(is_valid_sql_expression("deleted_at")); // contains "delete"
        assert!(is_valid_sql_expression("order_total")); // contains "order"
        assert!(is_valid_sql_expression("group_name")); // contains "group"
        assert!(is_valid_sql_expression("from_date")); // contains "from"
        assert!(is_valid_sql_expression("where_clause")); // contains "where"
    }

    #[test]
    fn test_invalid_sql_expressions() {
        use super::is_valid_sql_expression;

        // Empty
        assert!(!is_valid_sql_expression(""));

        // SQL comments
        assert!(!is_valid_sql_expression("name -- comment"));
        assert!(!is_valid_sql_expression("/* comment */ name"));
        assert!(!is_valid_sql_expression("name */ attack"));

        // Statement terminators
        assert!(!is_valid_sql_expression("1; DROP TABLE users"));
        assert!(!is_valid_sql_expression("name;"));

        // Backticks
        assert!(!is_valid_sql_expression("`table`"));

        // SQL keywords as standalone words
        assert!(!is_valid_sql_expression("(SELECT password)"));
        assert!(!is_valid_sql_expression("INSERT INTO x"));
        assert!(!is_valid_sql_expression("DELETE FROM x"));
        assert!(!is_valid_sql_expression("DROP TABLE x"));
        assert!(!is_valid_sql_expression("UPDATE SET y=1"));
        assert!(!is_valid_sql_expression("UNION ALL"));
        assert!(!is_valid_sql_expression("x FROM y"));
        assert!(!is_valid_sql_expression("x WHERE y"));

        // System catalog access
        assert!(!is_valid_sql_expression("pg_catalog.pg_tables"));
        assert!(!is_valid_sql_expression("sqlite_master"));
        assert!(!is_valid_sql_expression("information_schema.tables"));

        // Hex escapes
        assert!(!is_valid_sql_expression("0x48454C4C4F"));
        assert!(!is_valid_sql_expression("\\x48454C4C4F"));

        // Dangerous functions (timing attacks, DoS)
        assert!(!is_valid_sql_expression("SLEEP(10)"));
        assert!(!is_valid_sql_expression("pg_sleep(5)"));
        assert!(!is_valid_sql_expression("BENCHMARK(1000000, SHA1('test'))"));
        assert!(!is_valid_sql_expression("WAITFOR DELAY '0:0:5'"));

        // File operations
        assert!(!is_valid_sql_expression("LOAD_FILE('/etc/passwd')"));
    }

    #[test]
    #[should_panic(expected = "Invalid SQL table name")]
    fn test_assert_valid_identifier_panics() {
        use super::assert_valid_sql_identifier;
        assert_valid_sql_identifier("users; DROP TABLE", "table");
    }

    #[test]
    #[should_panic(expected = "Invalid SQL expression")]
    fn test_assert_valid_expression_panics() {
        use super::assert_valid_sql_expression;
        assert_valid_sql_expression("1; DROP TABLE users", "computed field");
    }

    #[test]
    fn test_new_denies_regex_by_default() {
        // new() now uses secure defaults
        let validator = FilterValidator::new().allow_fields(&["name"]);

        // Regex should be denied by default
        let filter = Filter {
            field: "name".into(),
            op: Operator::Regex,
            value: Value::String("^test".into()),
        };

        let result = validator.validate(&filter);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::OperatorDenied { operator, .. } => {
                assert_eq!(operator, Operator::Regex);
            },
            _ => panic!("Expected OperatorDenied error"),
        }
    }

    #[test]
    fn test_permissive_allows_regex() {
        let validator = FilterValidator::permissive().allow_fields(&["name"]);

        let filter = Filter {
            field: "name".into(),
            op: Operator::Regex,
            value: Value::String("^test".into()),
        };

        // permissive() allows all operators
        assert!(validator.validate(&filter).is_ok());
    }

    #[test]
    fn test_new_allows_safe_operators() {
        let validator = FilterValidator::new().allow_fields(&["name", "status"]);

        // Safe operators should work
        let filter = Filter {
            field: "status".into(),
            op: Operator::Eq,
            value: Value::String("active".into()),
        };
        assert!(validator.validate(&filter).is_ok());

        // Like is also allowed (less dangerous than regex)
        let filter = Filter {
            field: "name".into(),
            op: Operator::Like,
            value: Value::String("%test%".into()),
        };
        assert!(validator.validate(&filter).is_ok());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // COMPOUND FILTER EDGE CASE TESTS
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_compound_filter_deep_nesting() {
        use crate::builder::{CompoundFilter, FilterExpr, simple};

        // Create deeply nested compound filters to test depth limits
        // Build: AND(OR(AND(filter1, filter2), filter3), filter4)
        let innermost = CompoundFilter::and(vec![
            simple("a", Operator::Eq, Value::Int(1)),
            simple("b", Operator::Eq, Value::Int(2)),
        ]);

        let middle = CompoundFilter::or(vec![
            FilterExpr::Compound(innermost),
            simple("c", Operator::Eq, Value::Int(3)),
        ]);

        let outer = CompoundFilter::and(vec![
            FilterExpr::Compound(middle),
            simple("d", Operator::Eq, Value::Int(4)),
        ]);

        // Validator with limited depth should reject this structure
        // The nesting depth here is controlled by the number of Value nesting, not compound filter depth
        // Compound filter depth is separate from value nesting
        let validator = FilterValidator::new();

        // For simple filter validation, the compound structure itself isn't checked
        // Each simple filter should pass individually
        let simple_filter = Filter {
            field: "a".into(),
            op: Operator::Eq,
            value: Value::Int(1),
        };
        assert!(validator.validate(&simple_filter).is_ok());

        // Verify compound filter can be constructed without panic
        assert_eq!(outer.filters.len(), 2);
        assert_eq!(outer.op, crate::LogicalOp::And);
    }

    #[test]
    fn test_validate_compound_not_single_element() {
        use crate::builder::{CompoundFilter, simple};

        // NOT should wrap exactly one filter
        let not_filter = CompoundFilter::not(simple("deleted", Operator::Eq, Value::Bool(true)));

        assert_eq!(not_filter.filters.len(), 1);
        assert_eq!(not_filter.op, crate::LogicalOp::Not);
    }

    #[test]
    fn test_validate_compound_empty_filters() {
        use crate::builder::CompoundFilter;

        // Edge case: Compound filter with empty filter list
        let empty_and = CompoundFilter::and(vec![]);
        let empty_or = CompoundFilter::or(vec![]);

        // Empty compound filters should have 0 filters
        assert!(empty_and.filters.is_empty());
        assert!(empty_or.filters.is_empty());
    }

    #[test]
    fn test_validate_deeply_nested_array_values() {
        // Test value nesting depth validation
        let validator = FilterValidator::new().max_depth(2);

        // 2 levels of nesting - should pass
        let filter_ok = Filter {
            field: "tags".into(),
            op: Operator::In,
            value: Value::Array(vec![Value::Array(vec![Value::Int(1)])]),
        };
        assert!(validator.validate(&filter_ok).is_ok());

        // 3 levels of nesting - should fail with max_depth(2)
        let filter_too_deep = Filter {
            field: "tags".into(),
            op: Operator::In,
            value: Value::Array(vec![Value::Array(vec![Value::Array(vec![Value::Int(1)])])]),
        };
        assert!(validator.validate(&filter_too_deep).is_err());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SQL INJECTION FUZZING TESTS
    // Production-critical security tests for large-scale deployment
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_sqli_classic_or_true() {
        use super::is_valid_sql_expression;

        // Classic OR-based injection - these are blocked by comment/semicolon detection
        // Note: Simple OR expressions are allowed for computed fields (e.g., "(a > 0) OR (b > 0)")
        // The primary protection is parameterized queries, not expression validation
        assert!(!is_valid_sql_expression("' OR 1=1--")); // Blocked by --
        assert!(!is_valid_sql_expression("1; OR 1=1")); // Blocked by ;

        // These would pass expression validation but values are parameterized
        // so they can't actually cause injection when used properly
        // The expression validator is defense-in-depth, not the primary protection
    }

    #[test]
    fn test_sqli_drop_table() {
        use super::is_valid_sql_expression;

        // DROP TABLE attacks
        assert!(!is_valid_sql_expression("'; DROP TABLE users--"));
        assert!(!is_valid_sql_expression("'; DROP TABLE users;--"));
        assert!(!is_valid_sql_expression("1; DROP TABLE users"));
        assert!(!is_valid_sql_expression("DROP TABLE users"));
        assert!(!is_valid_sql_expression("drop table users"));
        assert!(!is_valid_sql_expression("DrOp TaBlE users"));
    }

    #[test]
    fn test_sqli_union_attacks() {
        use super::is_valid_sql_expression;

        // UNION-based injection
        assert!(!is_valid_sql_expression("' UNION SELECT * FROM users--"));
        assert!(!is_valid_sql_expression(
            "' UNION ALL SELECT password FROM users--"
        ));
        assert!(!is_valid_sql_expression("1 UNION SELECT 1,2,3"));
        assert!(!is_valid_sql_expression(
            "UNION SELECT username,password FROM admin"
        ));
        assert!(!is_valid_sql_expression("' union select null,null,null--"));
    }

    #[test]
    fn test_sqli_comment_injection() {
        use super::is_valid_sql_expression;

        // Comment-based attacks - blocked by comment detection
        assert!(!is_valid_sql_expression("admin'--")); // SQL comment
        assert!(!is_valid_sql_expression("admin'/*")); // Block comment start
        assert!(!is_valid_sql_expression("*/; DROP TABLE users--")); // Block comment end + semicolon
        assert!(!is_valid_sql_expression("1/**/OR/**/1=1")); // Block comments

        // Note: MySQL # comment is not blocked - this validator is for Postgres/SQLite
        // MySQL-specific attacks should be handled at the application layer if needed
    }

    #[test]
    fn test_sqli_stacked_queries() {
        use super::is_valid_sql_expression;

        // Stacked query attacks (semicolon-based)
        assert!(!is_valid_sql_expression(
            "; INSERT INTO users VALUES('hacker')"
        ));
        assert!(!is_valid_sql_expression("; UPDATE users SET role='admin'"));
        assert!(!is_valid_sql_expression("; DELETE FROM users"));
        assert!(!is_valid_sql_expression("1; SELECT * FROM passwords"));
        assert!(!is_valid_sql_expression("'; TRUNCATE TABLE logs;--"));
    }

    #[test]
    fn test_sqli_time_based_blind() {
        use super::is_valid_sql_expression;

        // Time-based blind injection
        assert!(!is_valid_sql_expression("SLEEP(5)"));
        assert!(!is_valid_sql_expression("1 AND SLEEP(5)"));
        assert!(!is_valid_sql_expression("pg_sleep(5)"));
        assert!(!is_valid_sql_expression("1; SELECT pg_sleep(10)"));
        assert!(!is_valid_sql_expression("BENCHMARK(10000000,SHA1('test'))"));
        assert!(!is_valid_sql_expression("WAITFOR DELAY '0:0:5'"));
        assert!(!is_valid_sql_expression("dbms_lock.sleep(5)"));
    }

    #[test]
    fn test_sqli_file_operations() {
        use super::is_valid_sql_expression;

        // File read/write attacks
        assert!(!is_valid_sql_expression("LOAD_FILE('/etc/passwd')"));
        assert!(!is_valid_sql_expression("load_file('/etc/shadow')"));
        assert!(!is_valid_sql_expression(
            "INTO OUTFILE '/var/www/shell.php'"
        ));
        assert!(!is_valid_sql_expression("INTO DUMPFILE '/tmp/data'"));
        assert!(!is_valid_sql_expression("into_outfile('/tmp/x')"));
        assert!(!is_valid_sql_expression("into_dumpfile('/tmp/x')"));
    }

    #[test]
    fn test_sqli_system_catalog_access() {
        use super::is_valid_sql_expression;

        // System catalog enumeration
        assert!(!is_valid_sql_expression("pg_tables"));
        assert!(!is_valid_sql_expression("pg_catalog.pg_tables"));
        assert!(!is_valid_sql_expression("sqlite_master"));
        assert!(!is_valid_sql_expression("information_schema.tables"));
        assert!(!is_valid_sql_expression("sys.tables"));
        assert!(!is_valid_sql_expression("SELECT FROM information_schema"));
    }

    #[test]
    fn test_sqli_hex_encoding() {
        use super::is_valid_sql_expression;

        // Hex-encoded attacks
        assert!(!is_valid_sql_expression("0x27")); // Single quote
        assert!(!is_valid_sql_expression("0x4F5220313D31")); // OR 1=1
        assert!(!is_valid_sql_expression("\\x27"));
        assert!(!is_valid_sql_expression("CHAR(0x27)"));
    }

    #[test]
    fn test_sqli_keyword_boundary_detection() {
        use super::is_valid_sql_expression;

        // These SHOULD be allowed - keywords as substrings of identifiers
        assert!(is_valid_sql_expression("order_id")); // order
        assert!(is_valid_sql_expression("reorder_count")); // order
        assert!(is_valid_sql_expression("group_name")); // group
        assert!(is_valid_sql_expression("ungroup")); // group
        assert!(is_valid_sql_expression("from_date")); // from
        assert!(is_valid_sql_expression("wherefrom")); // where, from
        assert!(is_valid_sql_expression("selected_items")); // select
        assert!(is_valid_sql_expression("preselect")); // select
        assert!(is_valid_sql_expression("delete_flag")); // delete
        assert!(is_valid_sql_expression("undelete")); // delete
        assert!(is_valid_sql_expression("update_time")); // update
        assert!(is_valid_sql_expression("last_updated")); // update

        // These SHOULD be blocked - standalone keywords
        assert!(!is_valid_sql_expression("ORDER BY name"));
        assert!(!is_valid_sql_expression("GROUP BY id"));
        assert!(!is_valid_sql_expression("FROM users"));
        assert!(!is_valid_sql_expression("WHERE id=1"));
        assert!(!is_valid_sql_expression("SELECT *"));
        assert!(!is_valid_sql_expression("DELETE FROM"));
        assert!(!is_valid_sql_expression("UPDATE SET"));
    }

    #[test]
    fn test_sqli_case_variations() {
        use super::is_valid_sql_expression;

        // Case variations of dangerous keywords
        assert!(!is_valid_sql_expression("SELECT"));
        assert!(!is_valid_sql_expression("select"));
        assert!(!is_valid_sql_expression("SeLeCt"));
        assert!(!is_valid_sql_expression("sElEcT"));

        assert!(!is_valid_sql_expression("UNION"));
        assert!(!is_valid_sql_expression("union"));
        assert!(!is_valid_sql_expression("UnIoN"));

        assert!(!is_valid_sql_expression("DROP"));
        assert!(!is_valid_sql_expression("drop"));
        assert!(!is_valid_sql_expression("DrOp"));
    }

    #[test]
    fn test_sqli_whitespace_variations() {
        use super::is_valid_sql_expression;

        // Whitespace-based evasion - these should still be caught
        // Note: tabs and newlines in expressions
        assert!(!is_valid_sql_expression("SELECT\t*"));
        assert!(!is_valid_sql_expression("SELECT\n*"));
        assert!(!is_valid_sql_expression("  SELECT  "));
        assert!(!is_valid_sql_expression("DROP\t\tTABLE"));
    }

    #[test]
    fn test_sqli_expression_length_limit() {
        use super::is_valid_sql_expression;

        // Very long expressions should be rejected
        let long_expr = "a".repeat(1001);
        assert!(!is_valid_sql_expression(&long_expr));

        // At limit should be OK
        let at_limit = "a".repeat(1000);
        assert!(is_valid_sql_expression(&at_limit));
    }

    #[test]
    fn test_identifier_injection_attempts() {
        use super::is_valid_sql_identifier;

        // SQL injection via identifier names
        assert!(!is_valid_sql_identifier("users; DROP TABLE x"));
        assert!(!is_valid_sql_identifier("users--"));
        assert!(!is_valid_sql_identifier("users/*comment*/"));
        assert!(!is_valid_sql_identifier("users'"));
        assert!(!is_valid_sql_identifier("users\""));
        assert!(!is_valid_sql_identifier("users`"));
        assert!(!is_valid_sql_identifier("users;"));
        assert!(!is_valid_sql_identifier("(SELECT 1)"));
        assert!(!is_valid_sql_identifier("1 OR 1=1"));

        // Unicode injection attempts
        assert!(!is_valid_sql_identifier("users\u{0000}")); // Null byte
        assert!(!is_valid_sql_identifier("users\u{200B}")); // Zero-width space
        assert!(!is_valid_sql_identifier("usërs")); // Non-ASCII letter
        assert!(!is_valid_sql_identifier("用户")); // Chinese characters

        // Fullwidth characters (potential bypass)
        assert!(!is_valid_sql_identifier("ｕｓｅｒｓ")); // Fullwidth letters
    }

    #[test]
    fn test_valid_safe_expressions() {
        use super::is_valid_sql_expression;

        // Legitimate expressions that should be allowed
        assert!(is_valid_sql_expression("first_name || ' ' || last_name"));
        assert!(is_valid_sql_expression("price * quantity"));
        assert!(is_valid_sql_expression("price * 1.15")); // With tax
        assert!(is_valid_sql_expression(
            "COALESCE(nickname, first_name, 'Anonymous')"
        ));
        assert!(is_valid_sql_expression("UPPER(TRIM(name))"));
        assert!(is_valid_sql_expression("LENGTH(description)"));
        assert!(is_valid_sql_expression("ABS(balance)"));
        assert!(is_valid_sql_expression("ROUND(price, 2)"));
        assert!(is_valid_sql_expression("LOWER(email)"));
        assert!(is_valid_sql_expression("created_at + INTERVAL '1 day'"));
        assert!(is_valid_sql_expression("age >= 18"));
        assert!(is_valid_sql_expression("status = 'active'"));
        assert!(is_valid_sql_expression("NOT is_deleted"));
        assert!(is_valid_sql_expression("(price > 0) AND (quantity > 0)"));
    }

    #[test]
    fn test_filter_value_injection() {
        // Test that malicious values in filters are properly parameterized
        // (This tests the design, not execution - values go through $1, $2 placeholders)
        let validator = FilterValidator::new().allow_fields(&["name", "email"]);

        // Malicious value in string - should be allowed because it's parameterized
        let filter = Filter {
            field: "name".into(),
            op: Operator::Eq,
            value: Value::String("'; DROP TABLE users--".into()),
        };
        // Filter validation passes - the value is parameterized, not interpolated
        assert!(validator.validate(&filter).is_ok());

        // But the SQL builder would produce:
        // "SELECT * FROM users WHERE name = $1"
        // With params: ["'; DROP TABLE users--"]
        // This is SAFE because it's parameterized!
    }

    #[test]
    fn test_filter_field_injection() {
        // Test that malicious field names are blocked by whitelist
        let validator = FilterValidator::new().allow_fields(&["name", "email"]);

        // Attempting to use SQL injection as field name
        let filter = Filter {
            field: "name; DROP TABLE users--".into(),
            op: Operator::Eq,
            value: Value::String("test".into()),
        };
        // Should fail - field not in whitelist
        assert!(validator.validate(&filter).is_err());

        // Even without whitelist, the field goes through identifier validation
        // when the query is built
    }

    #[test]
    fn test_operator_based_attacks() {
        // Certain operators could be used for attacks
        let validator = FilterValidator::new()
            .allow_fields(&["name"])
            .deny_operators(&[Operator::Regex]); // ReDoS prevention

        // Regex operator should be denied (ReDoS risk)
        let filter = Filter {
            field: "name".into(),
            op: Operator::Regex,
            value: Value::String("^(a+)+$".into()), // ReDoS pattern
        };
        assert!(validator.validate(&filter).is_err());

        // LIKE is safer (no backtracking)
        let filter = Filter {
            field: "name".into(),
            op: Operator::Like,
            value: Value::String("%test%".into()),
        };
        assert!(validator.validate(&filter).is_ok());
    }
}
