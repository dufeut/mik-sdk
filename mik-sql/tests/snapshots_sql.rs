//! Snapshot tests for SQL query generation.
//!
//! These tests use insta to capture the generated SQL and detect
//! unexpected changes in query output.
//!
//! Run with: cargo test -p mik-sql
//! Update snapshots: cargo insta review

use insta::assert_snapshot;
use mik_sql::{Cursor, Operator, SortDir, Value, postgres, sqlite};

// =============================================================================
// SELECT Query Snapshots
// =============================================================================

#[test]
fn snapshot_select_simple() {
    let result = postgres("users").fields(&["id", "name", "email"]).build();
    assert_snapshot!("select_simple", result.sql);
}

#[test]
fn snapshot_select_with_filter() {
    let result = postgres("users")
        .fields(&["id", "name"])
        .filter("active", Operator::Eq, Value::Bool(true))
        .filter("role", Operator::Eq, Value::String("admin".to_string()))
        .build();
    assert_snapshot!("select_with_filter", result.sql);
}

#[test]
fn snapshot_select_with_sort() {
    let result = postgres("posts")
        .fields(&["id", "title", "created_at"])
        .sort("created_at", SortDir::Desc)
        .sort("id", SortDir::Asc)
        .build();
    assert_snapshot!("select_with_sort", result.sql);
}

#[test]
fn snapshot_select_with_pagination() {
    let result = postgres("products")
        .fields(&["id", "name", "price"])
        .filter(
            "category",
            Operator::Eq,
            Value::String("electronics".to_string()),
        )
        .sort("price", SortDir::Asc)
        .limit_offset(20, 40)
        .build();
    assert_snapshot!("select_with_pagination", result.sql);
}

// =============================================================================
// Cursor Pagination Snapshots
// =============================================================================

#[test]
fn snapshot_cursor_single_field() {
    let cursor = Cursor::new().int("id", 100);

    let result = postgres("posts")
        .fields(&["id", "title"])
        .sort("id", SortDir::Asc)
        .after_cursor(cursor.clone())
        .limit(20)
        .build();

    assert_snapshot!("cursor_single_field", result.sql);
}

#[test]
fn snapshot_cursor_multi_field() {
    let cursor = Cursor::new()
        .string("created_at", "2024-01-15T10:30:00Z")
        .int("id", 12345);

    let result = postgres("posts")
        .fields(&["id", "title", "created_at"])
        .sort("created_at", SortDir::Desc)
        .sort("id", SortDir::Desc)
        .after_cursor(cursor.clone())
        .limit(20)
        .build();

    assert_snapshot!("cursor_multi_field", result.sql);
}

// =============================================================================
// SQLite Dialect Snapshots
// =============================================================================

#[test]
fn snapshot_sqlite_select() {
    let result = sqlite("users")
        .fields(&["id", "name"])
        .filter("active", Operator::Eq, Value::Bool(true))
        .limit(10)
        .build();
    assert_snapshot!("sqlite_select", result.sql);
}

// =============================================================================
// Filter Operator Snapshots
// =============================================================================

#[test]
fn snapshot_filter_gt() {
    let result = postgres("products")
        .fields(&["id", "price"])
        .filter("price", Operator::Gt, Value::Float(50.0))
        .build();
    assert_snapshot!("filter_gt", result.sql);
}

#[test]
fn snapshot_filter_like() {
    let result = postgres("products")
        .fields(&["id", "name"])
        .filter(
            "name",
            Operator::Like,
            Value::String("%widget%".to_string()),
        )
        .build();
    assert_snapshot!("filter_like", result.sql);
}

#[test]
fn snapshot_filter_ne() {
    let result = postgres("users")
        .fields(&["id", "status"])
        .filter("status", Operator::Ne, Value::String("deleted".to_string()))
        .build();
    assert_snapshot!("filter_ne", result.sql);
}
