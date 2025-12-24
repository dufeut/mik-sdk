// =============================================================================
// CRATE-LEVEL QUALITY LINTS (following Tokio/Serde standards)
// =============================================================================
#![forbid(unsafe_code)]
#![deny(unused_must_use)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]
#![warn(unreachable_pub)]
// =============================================================================
// CLIPPY CONFIGURATION
// =============================================================================
// Pedantic lints that are too verbose to fix individually in a DSL-heavy crate
#![allow(clippy::doc_markdown)] // Code items in docs - extensive doc changes needed
#![allow(clippy::missing_errors_doc)] // # Errors sections - doc-heavy
#![allow(clippy::missing_panics_doc)] // # Panics sections - doc-heavy
#![allow(clippy::items_after_statements)] // const in functions - intentional for locality
#![allow(clippy::module_name_repetitions)] // Type names matching module - acceptable
#![allow(clippy::return_self_not_must_use)] // Builder pattern methods return Self by design
#![allow(clippy::must_use_candidate)] // Builder methods - fluent API doesn't need must_use
#![allow(clippy::match_same_arms)] // Intentional for clarity in some match expressions
#![allow(clippy::format_push_string)] // String building style preference
#![allow(clippy::cast_possible_truncation)] // Intentional in SQL context
#![allow(clippy::cast_sign_loss)] // Intentional in SQL context
#![allow(clippy::cast_possible_wrap)] // Intentional in SQL context

//! # mik-sql - SQL Query Builder with Mongo-style Filters
//!
//! A standalone SQL query builder with intuitive macro syntax for CRUD operations.
//! Supports Postgres and `SQLite` dialects.
//!
//! ## Quick Start
//!
//! ```ignore
//! use mik_sql::prelude::*;
//!
//! // SELECT - Read data
//! let (sql, params) = sql_read!(users {
//!     select: [id, name, email],
//!     filter: { active: true },
//!     order: name,
//!     limit: 10,
//! });
//! // → "SELECT id, name, email FROM users WHERE active = $1 ORDER BY name LIMIT 10"
//!
//! // INSERT - Create data
//! let (sql, params) = sql_create!(users {
//!     name: str(name),
//!     email: str(email),
//!     returning: [id],
//! });
//! // → "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id"
//!
//! // UPDATE - Update data
//! let (sql, params) = sql_update!(users {
//!     set: { name: str(new_name) },
//!     filter: { id: int(user_id) },
//! });
//! // → "UPDATE users SET name = $1 WHERE id = $2"
//!
//! // DELETE - Delete data
//! let (sql, params) = sql_delete!(users {
//!     filter: { id: int(user_id) },
//! });
//! // → "DELETE FROM users WHERE id = $1"
//! ```
//!
//! ## `SQLite` Dialect
//!
//! Add `sqlite` as first parameter for `SQLite` syntax (?1, ?2 instead of $1, $2):
//!
//! ```ignore
//! let (sql, params) = sql_read!(sqlite, users { ... });
//! ```
//!
//! ## Supported Operators
//!
//! | Operator | SQL | Example |
//! |----------|-----|---------|
//! | `$eq` | `=` | `"status": { "$eq": "active" }` |
//! | `$ne` | `!=` | `"status": { "$ne": "deleted" }` |
//! | `$gt` | `>` | `"age": { "$gt": 18 }` |
//! | `$gte` | `>=` | `"age": { "$gte": 21 }` |
//! | `$lt` | `<` | `"price": { "$lt": 100 }` |
//! | `$lte` | `<=` | `"price": { "$lte": 50 }` |
//! | `$in` | `IN` | `"status": { "$in": ["a", "b"] }` |
//! | `$nin` | `NOT IN` | `"status": { "$nin": ["x"] }` |
//! | `$like` | `LIKE` | `"name": { "$like": "%test%" }` |
//! | `$ilike` | `ILIKE` | `"name": { "$ilike": "%test%" }` |
//! | `$starts_with` | `LIKE $1 \|\| '%'` | `"name": { "$starts_with": "John" }` |
//! | `$ends_with` | `LIKE '%' \|\| $1` | `"email": { "$ends_with": "@example.com" }` |
//! | `$contains` | `LIKE '%' \|\| $1 \|\| '%'` | `"bio": { "$contains": "developer" }` |
//! | `$between` | `BETWEEN $1 AND $2` | `"age": { "$between": [18, 65] }` |
//!
//! ## Cursor Pagination
//!
//! ```ignore
//! use mik_sql::prelude::*;
//!
//! let (sql, params) = sql_read!(posts {
//!     select: [id, title, created_at],
//!     filter: { published: true },
//!     order: [-created_at, -id],
//!     after: req.query("after"),
//!     limit: 20,
//! });
//!
//! // Create cursor for next page
//! let next_cursor = Cursor::new()
//!     .string("created_at", &last_item.created_at)
//!     .int("id", last_item.id)
//!     .encode();
//! ```

mod builder;
mod dialect;
mod pagination;
mod validate;

pub use builder::{
    Aggregate, AggregateFunc, CompoundFilter, ComputedField, CursorDirection, DeleteBuilder,
    Filter, FilterExpr, InsertBuilder, LogicalOp, Operator, QueryBuilder, QueryResult, SortDir,
    SortField, UpdateBuilder, Value, and, delete, insert, not, or, simple, update,
};

// Internal functions used by macros - not part of public API
#[doc(hidden)]
pub use builder::{delete_sqlite, insert_sqlite, update_sqlite};
pub use dialect::{Dialect, Postgres, Sqlite};
pub use pagination::{Cursor, CursorError, IntoCursor, KeysetCondition, PageInfo};
pub use validate::{
    FilterValidator, ValidationError, assert_valid_sql_expression, assert_valid_sql_identifier,
    is_valid_sql_expression, is_valid_sql_identifier, merge_filters,
};

// Re-export macros from mik-sql-macros
pub use mik_sql_macros::{ids, sql_create, sql_delete, sql_read, sql_update};

/// Build a query for Postgres.
///
/// Convenience function that creates a `QueryBuilder` with Postgres dialect.
#[must_use]
pub fn postgres(table: &str) -> QueryBuilder<Postgres> {
    QueryBuilder::new(Postgres, table)
}

/// Build a query for `SQLite`.
///
/// Convenience function that creates a `QueryBuilder` with `SQLite` dialect.
#[must_use]
pub fn sqlite(table: &str) -> QueryBuilder<Sqlite> {
    QueryBuilder::new(Sqlite, table)
}

/// Prelude module for convenient imports.
///
/// ```ignore
/// use mik_sql::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        Aggregate, AggregateFunc, CompoundFilter, ComputedField, Cursor, CursorDirection,
        CursorError, DeleteBuilder, Dialect, Filter, FilterExpr, FilterValidator, InsertBuilder,
        IntoCursor, KeysetCondition, LogicalOp, Operator, PageInfo, Postgres, QueryBuilder,
        QueryResult, SortDir, SortField, Sqlite, UpdateBuilder, ValidationError, Value, and,
        delete, insert, merge_filters, not, or, postgres, simple, sqlite, update,
    };

    // Re-export macros
    pub use mik_sql_macros::{ids, sql_create, sql_delete, sql_read, sql_update};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let result = postgres("users").fields(&["id", "name", "email"]).build();

        assert_eq!(result.sql, "SELECT id, name, email FROM users");
        assert!(result.params.is_empty());
    }

    #[test]
    fn test_select_with_filter() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter("active", Operator::Eq, Value::Bool(true))
            .build();

        assert_eq!(result.sql, "SELECT id, name FROM users WHERE active = $1");
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_sqlite_dialect() {
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter("active", Operator::Eq, Value::Bool(true))
            .build();

        assert_eq!(result.sql, "SELECT id, name FROM users WHERE active = ?1");
    }

    #[test]
    fn test_cursor_pagination() {
        let cursor = Cursor::new().int("id", 100);

        let result = postgres("users")
            .fields(&["id", "name"])
            .sort("id", SortDir::Asc)
            .after_cursor(cursor.clone())
            .limit(20)
            .build();

        assert_eq!(
            result.sql,
            "SELECT id, name FROM users WHERE id > $1 ORDER BY id ASC LIMIT 20"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // EDGE CASE TESTS
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_multi_field_cursor_mixed_sort_directions() {
        // Edge case: Multi-field cursor with mixed ASC/DESC sort directions
        // DESC fields use < operator, ASC fields use > operator
        let cursor = Cursor::new()
            .string("created_at", "2024-01-15T10:00:00Z")
            .int("id", 42);

        let result = postgres("posts")
            .fields(&["id", "title", "created_at"])
            .sort("created_at", SortDir::Desc) // Newest first
            .sort("id", SortDir::Asc) // Then by ID ascending
            .after_cursor(cursor.clone())
            .limit(20)
            .build();

        // Multi-field keyset uses tuple comparison: (created_at, id) < ($1, $2)
        // For DESC primary, ASC secondary: items after cursor have (smaller created_at) OR (same created_at AND larger id)
        assert!(result.sql.contains("ORDER BY created_at DESC, id ASC"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_multi_field_cursor_all_desc() {
        // Edge case: All fields descending
        let cursor = Cursor::new()
            .string("created_at", "2024-01-15T10:00:00Z")
            .int("id", 42);

        let result = postgres("posts")
            .fields(&["id", "title"])
            .sort("created_at", SortDir::Desc)
            .sort("id", SortDir::Desc)
            .after_cursor(cursor.clone())
            .limit(10)
            .build();

        assert!(result.sql.contains("ORDER BY created_at DESC, id DESC"));
    }

    #[test]
    fn test_sqlite_between_operator() {
        // Edge case: SQLite BETWEEN handling with expanded parameters
        let result = sqlite("products")
            .fields(&["id", "name", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Float(10.0), Value::Float(100.0)]),
            )
            .build();

        // SQLite BETWEEN uses ?1 AND ?2 placeholders
        assert!(result.sql.contains("BETWEEN"));
        assert!(result.sql.contains("?1"));
        assert!(result.sql.contains("?2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_postgres_between_operator() {
        // Postgres BETWEEN for comparison
        let result = postgres("products")
            .fields(&["id", "name", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Int(10), Value::Int(100)]),
            )
            .build();

        assert!(result.sql.contains("BETWEEN"));
        assert!(result.sql.contains("$1"));
        assert!(result.sql.contains("$2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_compound_filter_nested() {
        use builder::{CompoundFilter, FilterExpr, simple};

        // Edge case: Nested compound filters (AND containing OR)
        let nested_or = CompoundFilter::or(vec![
            simple("status", Operator::Eq, Value::String("active".into())),
            simple("status", Operator::Eq, Value::String("pending".into())),
        ]);

        let result = postgres("orders")
            .fields(&["id", "status", "amount"])
            .filter_expr(FilterExpr::Compound(CompoundFilter::and(vec![
                simple("amount", Operator::Gte, Value::Int(100)),
                FilterExpr::Compound(nested_or),
            ])))
            .build();

        // Should produce: amount >= $1 AND (status = $2 OR status = $3)
        assert!(result.sql.contains("AND"));
        assert!(result.sql.contains("OR"));
        assert_eq!(result.params.len(), 3);
    }

    #[test]
    fn test_compound_filter_not() {
        use builder::{CompoundFilter, FilterExpr, simple};

        // Edge case: NOT compound filter
        let result = postgres("users")
            .fields(&["id", "name", "role"])
            .filter_expr(FilterExpr::Compound(CompoundFilter::not(simple(
                "role",
                Operator::Eq,
                Value::String("admin".into()),
            ))))
            .build();

        assert!(result.sql.contains("NOT"));
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_empty_cursor_ignored() {
        // Edge case: Empty cursor should not add any keyset conditions
        let cursor = Cursor::new();

        let result = postgres("users")
            .fields(&["id", "name"])
            .sort("id", SortDir::Asc)
            .after_cursor(cursor.clone())
            .limit(20)
            .build();

        // Should not have WHERE clause from empty cursor
        assert_eq!(
            result.sql,
            "SELECT id, name FROM users ORDER BY id ASC LIMIT 20"
        );
    }

    #[test]
    fn test_cursor_extra_fields_ignored() {
        // Edge case: Cursor contains fields that are NOT in the sort specification.
        // These extra fields should be silently ignored - only sort fields matter.
        let cursor = Cursor::new()
            .string("extra_field", "should_be_ignored")
            .int("another_extra", 999)
            .int("id", 42); // Only this matches sort

        let result = postgres("users")
            .fields(&["id", "name"])
            .sort("id", SortDir::Asc) // Only sort by id
            .after_cursor(cursor.clone())
            .limit(20)
            .build();

        // Should only use 'id' from cursor, ignore extra_field and another_extra
        assert!(result.sql.contains("id > $1"));
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_sqlite_in_clause_expansion() {
        // Edge case: SQLite IN clause expands to multiple placeholders
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter(
                "status",
                Operator::In,
                Value::Array(vec![
                    Value::String("active".into()),
                    Value::String("pending".into()),
                    Value::String("review".into()),
                ]),
            )
            .build();

        // SQLite: status IN (?1, ?2, ?3)
        assert!(result.sql.contains("IN (?1, ?2, ?3)"));
        assert_eq!(result.params.len(), 3);
    }

    #[test]
    fn test_postgres_in_clause_array() {
        // Postgres uses ANY with array parameter
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter(
                "status",
                Operator::In,
                Value::Array(vec![
                    Value::String("active".into()),
                    Value::String("pending".into()),
                ]),
            )
            .build();

        // Postgres: status = ANY($1)
        assert!(result.sql.contains("= ANY($1)"));
        assert_eq!(result.params.len(), 1); // Single array param
    }

    // =========================================================================
    // BETWEEN OPERATOR EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_between_with_exactly_two_values() {
        // BETWEEN with exactly 2 values should work
        let result = postgres("products")
            .fields(&["id", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Int(10), Value::Int(100)]),
            )
            .build();

        assert!(result.sql.contains("BETWEEN $1 AND $2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_between_with_one_value_fallback() {
        // BETWEEN with 1 value returns impossible condition (consistent behavior)
        let result = postgres("products")
            .fields(&["id", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Int(10)]),
            )
            .build();
        assert!(
            result.sql.contains("1=0"),
            "Should return impossible condition"
        );
    }

    #[test]
    fn test_between_with_three_values_fallback() {
        // BETWEEN with 3 values returns impossible condition (consistent behavior)
        let result = postgres("products")
            .fields(&["id", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Int(10), Value::Int(50), Value::Int(100)]),
            )
            .build();
        assert!(
            result.sql.contains("1=0"),
            "Should return impossible condition"
        );
    }

    #[test]
    fn test_between_with_empty_array_fallback() {
        // BETWEEN with empty array returns impossible condition (consistent behavior)
        let result = postgres("products")
            .fields(&["id", "price"])
            .filter("price", Operator::Between, Value::Array(vec![]))
            .build();
        assert!(
            result.sql.contains("1=0"),
            "Should return impossible condition"
        );
    }

    #[test]
    fn test_between_with_different_value_types() {
        // BETWEEN with different value types (strings for date range)
        let result = postgres("orders")
            .fields(&["id", "created_at"])
            .filter(
                "created_at",
                Operator::Between,
                Value::Array(vec![
                    Value::String("2024-01-01".into()),
                    Value::String("2024-12-31".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("BETWEEN $1 AND $2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_between_sqlite_dialect() {
        // BETWEEN with SQLite dialect
        let result = sqlite("products")
            .fields(&["id", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Float(9.99), Value::Float(99.99)]),
            )
            .build();

        assert!(result.sql.contains("BETWEEN ?1 AND ?2"));
        assert_eq!(result.params.len(), 2);
    }
}
