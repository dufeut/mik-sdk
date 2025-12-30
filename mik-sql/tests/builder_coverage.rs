//! Comprehensive tests for builder module coverage.
//!
//! These tests target uncovered code paths in:
//! - `filter.rs`: compound filters, edge cases
//! - `update.rs`: `set_many`, filter combinations
//! - `delete.rs`: `filter_expr`, `SQLite` operations

use mik_sql::{
    Operator, Value, and, delete, delete_sqlite, insert, insert_sqlite, not, or, postgres, simple,
    sqlite, update, update_sqlite,
};

// =============================================================================
// FILTER MODULE TESTS (targeting filter.rs coverage)
// =============================================================================

mod filter_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Compound filters: single-item AND/OR (lines 37, 44 in filter.rs)
    // -------------------------------------------------------------------------

    #[test]
    fn test_and_single_condition() {
        // AND with exactly one condition should not add parentheses
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter_expr(and(vec![simple("active", Operator::Eq, Value::Bool(true))]))
            .build();

        // Single condition AND should just be the condition without (...)
        assert!(result.sql.contains("active = $1"), "SQL: {}", result.sql);
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_or_single_condition() {
        // OR with exactly one condition should not add parentheses
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter_expr(or(vec![simple(
                "role",
                Operator::Eq,
                Value::String("admin".into()),
            )]))
            .build();

        assert!(result.sql.contains("role = $1"), "SQL: {}", result.sql);
        assert_eq!(result.params.len(), 1);
    }

    // -------------------------------------------------------------------------
    // NOT operator (line 51 in filter.rs - unwrap_or_default)
    // -------------------------------------------------------------------------

    #[test]
    fn test_not_with_empty_filters() {
        // NOT with empty filters should produce NOT () with empty inner
        let result = postgres("users")
            .fields(&["id"])
            .filter_expr(not(and(vec![])))
            .build();

        // NOT with empty inner should produce NOT ()
        assert!(result.sql.contains("NOT"), "SQL: {}", result.sql);
    }

    #[test]
    fn test_not_simple_condition() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter_expr(not(simple("deleted", Operator::Eq, Value::Bool(true))))
            .build();

        assert!(result.sql.contains("NOT (deleted = $1)"));
        assert_eq!(result.params.len(), 1);
    }

    // -------------------------------------------------------------------------
    // Boolean operators with Ne (line 90-92)
    // -------------------------------------------------------------------------

    #[test]
    fn test_bool_ne_operator() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter("active", Operator::Ne, Value::Bool(false))
            .build();

        assert!(result.sql.contains("active != $1"));
        assert_eq!(result.params.len(), 1);
        assert_eq!(result.params[0], Value::Bool(false));
    }

    // -------------------------------------------------------------------------
    // Regex operator (lines 96-99)
    // -------------------------------------------------------------------------

    #[test]
    fn test_regex_operator_postgres() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter(
                "email",
                Operator::Regex,
                Value::String(".*@test\\.com$".into()),
            )
            .build();

        // Postgres regex uses ~
        assert!(result.sql.contains("email ~ $1"));
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_regex_operator_sqlite() {
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter(
                "email",
                Operator::Regex,
                Value::String(".*@test\\.com$".into()),
            )
            .build();

        // SQLite falls back to LIKE
        assert!(result.sql.contains("email LIKE ?1"));
        assert_eq!(result.params.len(), 1);
    }

    // -------------------------------------------------------------------------
    // ILike operator (lines 103-108)
    // -------------------------------------------------------------------------

    #[test]
    fn test_ilike_postgres() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter("name", Operator::ILike, Value::String("%john%".into()))
            .build();

        assert!(result.sql.contains("name ILIKE $1"));
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_ilike_sqlite_fallback() {
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter("name", Operator::ILike, Value::String("%john%".into()))
            .build();

        // SQLite doesn't support ILIKE, falls back to LIKE
        assert!(result.sql.contains("name LIKE ?1"));
        assert!(!result.sql.contains("ILIKE"));
        assert_eq!(result.params.len(), 1);
    }

    // -------------------------------------------------------------------------
    // String pattern operators (lines 113-123)
    // -------------------------------------------------------------------------

    #[test]
    fn test_starts_with_postgres() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter("name", Operator::StartsWith, Value::String("John".into()))
            .build();

        assert!(result.sql.contains("name LIKE $1 || '%'"));
    }

    #[test]
    fn test_starts_with_sqlite() {
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter("name", Operator::StartsWith, Value::String("John".into()))
            .build();

        assert!(result.sql.contains("name LIKE ?1 || '%'"));
    }

    #[test]
    fn test_ends_with_postgres() {
        let result = postgres("users")
            .fields(&["id", "email"])
            .filter(
                "email",
                Operator::EndsWith,
                Value::String("@example.com".into()),
            )
            .build();

        assert!(result.sql.contains("email LIKE '%' || $1"));
    }

    #[test]
    fn test_ends_with_sqlite() {
        let result = sqlite("users")
            .fields(&["id", "email"])
            .filter(
                "email",
                Operator::EndsWith,
                Value::String("@example.com".into()),
            )
            .build();

        assert!(result.sql.contains("email LIKE '%' || ?1"));
    }

    #[test]
    fn test_contains_postgres() {
        let result = postgres("users")
            .fields(&["id", "bio"])
            .filter("bio", Operator::Contains, Value::String("developer".into()))
            .build();

        assert!(result.sql.contains("bio LIKE '%' || $1 || '%'"));
    }

    #[test]
    fn test_contains_sqlite() {
        let result = sqlite("users")
            .fields(&["id", "bio"])
            .filter("bio", Operator::Contains, Value::String("developer".into()))
            .build();

        assert!(result.sql.contains("bio LIKE '%' || ?1 || '%'"));
    }

    // -------------------------------------------------------------------------
    // NOT IN operator (lines 79-82)
    // -------------------------------------------------------------------------

    #[test]
    fn test_not_in_postgres() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter(
                "status",
                Operator::NotIn,
                Value::Array(vec![
                    Value::String("banned".into()),
                    Value::String("deleted".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("status != ALL($1)"));
        assert_eq!(result.params.len(), 1);
    }

    #[test]
    fn test_not_in_sqlite() {
        let result = sqlite("users")
            .fields(&["id", "name"])
            .filter(
                "status",
                Operator::NotIn,
                Value::Array(vec![
                    Value::String("banned".into()),
                    Value::String("deleted".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("status NOT IN (?1, ?2)"));
        assert_eq!(result.params.len(), 2);
    }

    // -------------------------------------------------------------------------
    // Deeply nested compound filters
    // -------------------------------------------------------------------------

    #[test]
    fn test_deeply_nested_compound_filters() {
        // ((a OR b) AND (c OR d)) OR (e AND f)
        let expr = or(vec![
            and(vec![
                or(vec![
                    simple("a", Operator::Eq, Value::Int(1)),
                    simple("b", Operator::Eq, Value::Int(2)),
                ]),
                or(vec![
                    simple("c", Operator::Eq, Value::Int(3)),
                    simple("d", Operator::Eq, Value::Int(4)),
                ]),
            ]),
            and(vec![
                simple("e", Operator::Eq, Value::Int(5)),
                simple("f", Operator::Eq, Value::Int(6)),
            ]),
        ]);

        let result = postgres("data").fields(&["id"]).filter_expr(expr).build();

        assert!(result.sql.contains("WHERE"));
        assert_eq!(result.params.len(), 6);
    }

    #[test]
    fn test_not_with_compound_inner() {
        // NOT (a AND b)
        let expr = not(and(vec![
            simple("status", Operator::Eq, Value::String("deleted".into())),
            simple("archived", Operator::Eq, Value::Bool(true)),
        ]));

        let result = postgres("records")
            .fields(&["id"])
            .filter_expr(expr)
            .build();

        assert!(result.sql.contains("NOT ((status = $1 AND archived = $2))"));
        assert_eq!(result.params.len(), 2);
    }

    // -------------------------------------------------------------------------
    // NULL handling with Eq and Ne (lines 70-71)
    // -------------------------------------------------------------------------

    #[test]
    fn test_null_eq() {
        let result = postgres("users")
            .fields(&["id"])
            .filter("deleted_at", Operator::Eq, Value::Null)
            .build();

        assert!(result.sql.contains("deleted_at IS NULL"));
        assert!(result.params.is_empty());
    }

    #[test]
    fn test_null_ne() {
        let result = postgres("users")
            .fields(&["id"])
            .filter("deleted_at", Operator::Ne, Value::Null)
            .build();

        assert!(result.sql.contains("deleted_at IS NOT NULL"));
        assert!(result.params.is_empty());
    }

    // -------------------------------------------------------------------------
    // Standard comparison operators (lines 146-158)
    // -------------------------------------------------------------------------

    #[test]
    fn test_like_operator() {
        let result = postgres("users")
            .fields(&["id", "name"])
            .filter("name", Operator::Like, Value::String("%test%".into()))
            .build();

        assert!(result.sql.contains("name LIKE $1"));
    }

    #[test]
    fn test_all_comparison_operators() {
        let result = postgres("products")
            .fields(&["id"])
            .filter("price", Operator::Gt, Value::Float(10.0))
            .filter("price", Operator::Lt, Value::Float(100.0))
            .filter("quantity", Operator::Gte, Value::Int(1))
            .filter("quantity", Operator::Lte, Value::Int(1000))
            .build();

        assert!(result.sql.contains("price > $1"));
        assert!(result.sql.contains("price < $2"));
        assert!(result.sql.contains("quantity >= $3"));
        assert!(result.sql.contains("quantity <= $4"));
        assert_eq!(result.params.len(), 4);
    }
}

// =============================================================================
// UPDATE MODULE TESTS (targeting update.rs coverage)
// =============================================================================

mod update_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // set_many method (lines 56-62)
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_set_many() {
        let result = update("users")
            .set_many(vec![
                ("name", Value::String("Updated Name".into())),
                ("email", Value::String("new@example.com".into())),
                ("age", Value::Int(30)),
            ])
            .filter("id", Operator::Eq, Value::Int(42))
            .build();

        assert!(result.sql.contains("UPDATE users SET"));
        assert!(result.sql.contains("name = $1"));
        assert!(result.sql.contains("email = $2"));
        assert!(result.sql.contains("age = $3"));
        assert!(result.sql.contains("WHERE id = $4"));
        assert_eq!(result.params.len(), 4);
    }

    #[test]
    fn test_update_set_and_set_many_combined() {
        let result = update("users")
            .set("status", Value::String("active".into()))
            .set_many(vec![
                ("updated_at", Value::String("2025-01-01".into())),
                ("version", Value::Int(2)),
            ])
            .filter("id", Operator::Eq, Value::Int(1))
            .build();

        assert!(result.sql.contains("status = $1"));
        assert!(result.sql.contains("updated_at = $2"));
        assert!(result.sql.contains("version = $3"));
        assert_eq!(result.params.len(), 4);
    }

    // -------------------------------------------------------------------------
    // filter_expr with simple filters combined (lines 118-141)
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_filter_expr_only() {
        let result = update("users")
            .set("notified", Value::Bool(true))
            .filter_expr(or(vec![
                simple("role", Operator::Eq, Value::String("admin".into())),
                simple("role", Operator::Eq, Value::String("moderator".into())),
            ]))
            .build();

        assert!(result.sql.contains("WHERE (role = $2 OR role = $3)"));
        assert_eq!(result.params.len(), 3);
    }

    #[test]
    fn test_update_filter_expr_combined_with_simple_filters() {
        let result = update("users")
            .set("notified", Value::Bool(true))
            .filter_expr(or(vec![
                simple("role", Operator::Eq, Value::String("admin".into())),
                simple("role", Operator::Eq, Value::String("moderator".into())),
            ]))
            .filter("active", Operator::Eq, Value::Bool(true))
            .filter("deleted", Operator::Eq, Value::Bool(false))
            .build();

        // filter_expr conditions AND simple filters
        assert!(result.sql.contains("WHERE"));
        assert!(result.sql.contains("role = $2 OR role = $3"));
        assert!(result.sql.contains("active = $4"));
        assert!(result.sql.contains("deleted = $5"));
        assert_eq!(result.params.len(), 5);
    }

    // -------------------------------------------------------------------------
    // SQLite update operations
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_sqlite() {
        let result = update_sqlite("users")
            .set("name", Value::String("Updated".into()))
            .filter("id", Operator::Eq, Value::Int(1))
            .build();

        assert!(result.sql.contains("name = ?1"));
        assert!(result.sql.contains("id = ?2"));
        assert!(!result.sql.contains('$'));
    }

    #[test]
    fn test_update_sqlite_with_returning() {
        let result = update_sqlite("users")
            .set("status", Value::String("active".into()))
            .filter("id", Operator::Eq, Value::Int(1))
            .returning(&["id", "status", "updated_at"])
            .build();

        assert!(result.sql.contains("RETURNING id, status, updated_at"));
    }

    // -------------------------------------------------------------------------
    // RETURNING clause (lines 144-147)
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_returning() {
        let result = update("users")
            .set("email", Value::String("new@example.com".into()))
            .filter("id", Operator::Eq, Value::Int(42))
            .returning(&["id", "email", "updated_at"])
            .build();

        assert!(result.sql.contains("RETURNING id, email, updated_at"));
    }

    #[test]
    fn test_update_returning_single_column() {
        let result = update("users")
            .set("active", Value::Bool(false))
            .filter("id", Operator::Eq, Value::Int(1))
            .returning(&["id"])
            .build();

        assert!(result.sql.contains("RETURNING id"));
    }

    // -------------------------------------------------------------------------
    // No filters (no WHERE clause)
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_no_filters() {
        let result = update("settings")
            .set("value", Value::String("new_value".into()))
            .build();

        assert!(!result.sql.contains("WHERE"));
        assert_eq!(result.sql, "UPDATE settings SET value = $1");
    }

    // -------------------------------------------------------------------------
    // Edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_update_with_not_filter() {
        let result = update("users")
            .set("notified", Value::Bool(true))
            .filter_expr(not(simple(
                "role",
                Operator::Eq,
                Value::String("guest".into()),
            )))
            .build();

        assert!(result.sql.contains("NOT (role = $2)"));
    }
}

// =============================================================================
// DELETE MODULE TESTS (targeting delete.rs coverage)
// =============================================================================

mod delete_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // filter_expr method (lines 51-54)
    // -------------------------------------------------------------------------

    #[test]
    fn test_delete_filter_expr() {
        let result = delete("sessions")
            .filter_expr(or(vec![
                simple("expired", Operator::Eq, Value::Bool(true)),
                simple(
                    "created_at",
                    Operator::Lt,
                    Value::String("2024-01-01".into()),
                ),
            ]))
            .build();

        assert!(result.sql.contains("DELETE FROM sessions WHERE"));
        assert!(result.sql.contains("expired = $1 OR created_at < $2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_delete_filter_expr_combined_with_simple() {
        let result = delete("logs")
            .filter_expr(and(vec![
                simple("level", Operator::Eq, Value::String("debug".into())),
                simple("level", Operator::Eq, Value::String("trace".into())),
            ]))
            .filter("archived", Operator::Eq, Value::Bool(false))
            .build();

        // Both filter_expr and simple filters should be combined
        assert!(result.sql.contains("WHERE"));
        assert!(result.sql.contains("AND"));
        assert_eq!(result.params.len(), 3);
    }

    // -------------------------------------------------------------------------
    // SQLite delete operations
    // -------------------------------------------------------------------------

    #[test]
    fn test_delete_sqlite() {
        let result = delete_sqlite("sessions")
            .filter("user_id", Operator::Eq, Value::Int(42))
            .build();

        assert!(result.sql.contains("?1"));
        assert!(!result.sql.contains('$'));
        assert_eq!(result.sql, "DELETE FROM sessions WHERE user_id = ?1");
    }

    #[test]
    fn test_delete_sqlite_with_returning() {
        let result = delete_sqlite("sessions")
            .filter("user_id", Operator::Eq, Value::Int(42))
            .returning(&["id", "token"])
            .build();

        assert!(result.sql.contains("RETURNING id, token"));
    }

    #[test]
    fn test_delete_sqlite_compound_filter() {
        let result = delete_sqlite("logs")
            .filter_expr(or(vec![
                simple("level", Operator::Eq, Value::String("debug".into())),
                simple("level", Operator::Eq, Value::String("trace".into())),
            ]))
            .build();

        assert!(result.sql.contains("level = ?1 OR level = ?2"));
    }

    // -------------------------------------------------------------------------
    // RETURNING clause (lines 105-108)
    // -------------------------------------------------------------------------

    #[test]
    fn test_delete_returning() {
        let result = delete("users")
            .filter("id", Operator::Eq, Value::Int(42))
            .returning(&["id", "email", "name"])
            .build();

        assert!(result.sql.contains("RETURNING id, email, name"));
    }

    #[test]
    fn test_delete_returning_single() {
        let result = delete("logs")
            .filter("id", Operator::Eq, Value::Int(1))
            .returning(&["id"])
            .build();

        assert!(result.sql.contains("RETURNING id"));
    }

    // -------------------------------------------------------------------------
    // No filters (dangerous but valid)
    // -------------------------------------------------------------------------

    #[test]
    fn test_delete_no_filters() {
        let result = delete("temp_data").build();

        assert!(!result.sql.contains("WHERE"));
        assert_eq!(result.sql, "DELETE FROM temp_data");
        assert!(result.params.is_empty());
    }

    // -------------------------------------------------------------------------
    // Complex filter scenarios
    // -------------------------------------------------------------------------

    #[test]
    fn test_delete_with_not() {
        let result = delete("users")
            .filter_expr(not(simple(
                "status",
                Operator::Eq,
                Value::String("active".into()),
            )))
            .build();

        assert!(result.sql.contains("NOT (status = $1)"));
    }

    #[test]
    fn test_delete_multiple_simple_filters() {
        let result = delete("logs")
            .filter("level", Operator::Eq, Value::String("debug".into()))
            .filter(
                "created_at",
                Operator::Lt,
                Value::String("2024-01-01".into()),
            )
            .filter("archived", Operator::Eq, Value::Bool(true))
            .build();

        assert!(result.sql.contains("level = $1"));
        assert!(result.sql.contains("created_at < $2"));
        assert!(result.sql.contains("archived = $3"));
        assert_eq!(result.params.len(), 3);
    }

    #[test]
    fn test_delete_with_in_operator() {
        let result = delete("sessions")
            .filter(
                "status",
                Operator::In,
                Value::Array(vec![
                    Value::String("expired".into()),
                    Value::String("revoked".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("status = ANY($1)"));
    }

    #[test]
    fn test_delete_sqlite_with_in() {
        let result = delete_sqlite("sessions")
            .filter(
                "status",
                Operator::In,
                Value::Array(vec![
                    Value::String("expired".into()),
                    Value::String("revoked".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("status IN (?1, ?2)"));
        assert_eq!(result.params.len(), 2);
    }
}

// =============================================================================
// INSERT MODULE TESTS (for completeness)
// =============================================================================

mod insert_tests {
    use super::*;

    #[test]
    fn test_insert_sqlite() {
        let result = insert_sqlite("users")
            .columns(&["name", "email"])
            .values(vec![
                Value::String("Alice".into()),
                Value::String("alice@example.com".into()),
            ])
            .build();

        assert!(result.sql.contains("?1"));
        assert!(result.sql.contains("?2"));
        assert!(!result.sql.contains('$'));
    }

    #[test]
    fn test_insert_with_returning() {
        let result = insert("users")
            .columns(&["name"])
            .values(vec![Value::String("Bob".into())])
            .returning(&["id", "created_at"])
            .build();

        assert!(result.sql.contains("RETURNING id, created_at"));
    }

    #[test]
    fn test_insert_sqlite_with_returning() {
        let result = insert_sqlite("users")
            .columns(&["name"])
            .values(vec![Value::String("Bob".into())])
            .returning(&["id"])
            .build();

        assert!(result.sql.contains("RETURNING id"));
    }

    #[test]
    fn test_insert_values_many() {
        let result = insert("users")
            .columns(&["name", "email"])
            .values_many(vec![
                vec![
                    Value::String("Alice".into()),
                    Value::String("alice@example.com".into()),
                ],
                vec![
                    Value::String("Bob".into()),
                    Value::String("bob@example.com".into()),
                ],
            ])
            .build();

        // Should have multiple value groups
        assert!(result.sql.contains("VALUES ($1, $2), ($3, $4)"));
        assert_eq!(result.params.len(), 4);
    }
}

// =============================================================================
// ADDITIONAL FILTER EDGE CASES
// =============================================================================

mod additional_filter_tests {
    use super::*;

    // Test BETWEEN with various value types
    #[test]
    fn test_between_with_floats() {
        let result = postgres("products")
            .fields(&["id", "price"])
            .filter(
                "price",
                Operator::Between,
                Value::Array(vec![Value::Float(9.99), Value::Float(99.99)]),
            )
            .build();

        assert!(result.sql.contains("price BETWEEN $1 AND $2"));
        assert_eq!(result.params.len(), 2);
    }

    #[test]
    fn test_between_with_strings() {
        let result = postgres("events")
            .fields(&["id", "event_date"])
            .filter(
                "event_date",
                Operator::Between,
                Value::Array(vec![
                    Value::String("2024-01-01".into()),
                    Value::String("2024-12-31".into()),
                ]),
            )
            .build();

        assert!(result.sql.contains("event_date BETWEEN $1 AND $2"));
        assert_eq!(result.params.len(), 2);
    }

    // Test compound filters with mixed operators
    #[test]
    fn test_and_or_not_combined() {
        // (a OR b) AND NOT(c)
        let expr = and(vec![
            or(vec![
                simple("status", Operator::Eq, Value::String("active".into())),
                simple("status", Operator::Eq, Value::String("pending".into())),
            ]),
            not(simple("deleted", Operator::Eq, Value::Bool(true))),
        ]);

        let result = postgres("users").fields(&["id"]).filter_expr(expr).build();

        assert!(result.sql.contains("OR"));
        assert!(result.sql.contains("AND"));
        assert!(result.sql.contains("NOT"));
        assert_eq!(result.params.len(), 3);
    }

    // Test empty IN arrays
    #[test]
    fn test_in_empty_array_postgres() {
        let result = postgres("users")
            .fields(&["id"])
            .filter("status", Operator::In, Value::Array(vec![]))
            .build();

        // Empty array should still generate valid SQL
        assert!(result.sql.contains("= ANY($1)"));
    }

    #[test]
    fn test_in_empty_array_sqlite() {
        let result = sqlite("users")
            .fields(&["id"])
            .filter("status", Operator::In, Value::Array(vec![]))
            .build();

        // Empty IN list in SQLite
        assert!(result.sql.contains("IN ()"));
    }

    // Test multiple filter_expr calls (only last one should be used)
    #[test]
    fn test_multiple_filter_expr_calls() {
        let result = postgres("users")
            .fields(&["id"])
            .filter_expr(simple("a", Operator::Eq, Value::Int(1)))
            .filter_expr(simple("b", Operator::Eq, Value::Int(2)))
            .build();

        // Last filter_expr should override
        assert!(result.sql.contains("b = $1"));
        assert!(!result.sql.contains("a ="));
    }

    // Test filter with all value types
    #[test]
    fn test_filter_all_value_types() {
        let result = postgres("data")
            .fields(&["id"])
            .filter("str_field", Operator::Eq, Value::String("test".into()))
            .filter("int_field", Operator::Eq, Value::Int(42))
            .filter("float_field", Operator::Eq, Value::Float(3.15))
            .filter("bool_field", Operator::Eq, Value::Bool(true))
            .filter("null_field", Operator::Eq, Value::Null)
            .build();

        assert!(result.sql.contains("str_field = $1"));
        assert!(result.sql.contains("int_field = $2"));
        assert!(result.sql.contains("float_field = $3"));
        assert!(result.sql.contains("bool_field = $4"));
        assert!(result.sql.contains("null_field IS NULL"));
        // Note: NULL doesn't add a param
        assert_eq!(result.params.len(), 4);
    }
}

// =============================================================================
// EDGE CASE TESTS FOR PARAMETER INDEXING
// =============================================================================

mod param_indexing_tests {
    use super::*;

    #[test]
    fn test_update_many_params_correct_indexing() {
        let result = update("users")
            .set("field1", Value::String("a".into()))
            .set("field2", Value::String("b".into()))
            .set("field3", Value::String("c".into()))
            .set("field4", Value::String("d".into()))
            .filter_expr(and(vec![
                simple("x", Operator::Eq, Value::Int(1)),
                simple("y", Operator::Eq, Value::Int(2)),
            ]))
            .filter("z", Operator::Eq, Value::Int(3))
            .build();

        // Should have correct indexing: $1-$4 for SET, $5-$7 for WHERE
        assert!(result.sql.contains("field1 = $1"));
        assert!(result.sql.contains("field2 = $2"));
        assert!(result.sql.contains("field3 = $3"));
        assert!(result.sql.contains("field4 = $4"));
        assert!(result.sql.contains("x = $5"));
        assert!(result.sql.contains("y = $6"));
        assert!(result.sql.contains("z = $7"));
        assert_eq!(result.params.len(), 7);
    }

    #[test]
    fn test_delete_many_filters_correct_indexing() {
        let result = delete("data")
            .filter_expr(or(vec![
                simple("a", Operator::Eq, Value::Int(1)),
                simple("b", Operator::Eq, Value::Int(2)),
                simple("c", Operator::Eq, Value::Int(3)),
            ]))
            .filter("d", Operator::Eq, Value::Int(4))
            .filter("e", Operator::Eq, Value::Int(5))
            .build();

        assert!(result.sql.contains("a = $1"));
        assert!(result.sql.contains("b = $2"));
        assert!(result.sql.contains("c = $3"));
        assert!(result.sql.contains("d = $4"));
        assert!(result.sql.contains("e = $5"));
        assert_eq!(result.params.len(), 5);
    }
}
