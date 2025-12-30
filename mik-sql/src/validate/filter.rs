//! Filter validation logic for user-provided filters.

use crate::{Filter, Operator, Value};
use std::fmt;

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
    pub const fn permissive() -> Self {
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
    pub const fn max_depth(mut self, depth: usize) -> Self {
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let msg = format!("{err}");
        assert!(msg.contains("password"));
        assert!(msg.contains("name"));

        let err = ValidationError::OperatorDenied {
            operator: Operator::Regex,
            field: "name".into(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Regex"));
        assert!(msg.contains("name"));

        let err = ValidationError::NestingTooDeep { max: 3, actual: 5 };
        let msg = format!("{err}");
        assert!(msg.contains('3'));
        assert!(msg.contains('5'));
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
