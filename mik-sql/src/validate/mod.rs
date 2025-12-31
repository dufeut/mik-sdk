//! Security validation layer for query filters and SQL identifiers.
//!
//! This module provides validation for:
//! - User-provided filters (field whitelisting, operator blacklisting)
//! - SQL identifiers (table names, column names) to prevent injection
//! - Nesting depth limits for complex queries
//!
//! # Example
//!
//! ```
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
//!     Filter::new("org_id", Operator::Eq, Value::Int(123)),
//!     Filter::new("deleted_at", Operator::Eq, Value::Null),
//! ];
//!
//! // User-provided filters (validated)
//! let user = vec![
//!     Filter::new("status", Operator::Eq, Value::String("active".into())),
//! ];
//!
//! // Merge with validation
//! let filters = merge_filters(trusted, user, &validator).unwrap();
//! assert_eq!(filters.len(), 3);
//! ```

mod column;
mod expression;
mod filter;

// Re-export all public items
pub use column::{assert_valid_sql_identifier, is_valid_sql_identifier};
pub use expression::{assert_valid_sql_expression, is_valid_sql_expression};
pub use filter::{FilterValidator, ValidationError, merge_filters};
