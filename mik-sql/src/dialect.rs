//! SQL dialect implementations for Postgres and `SQLite`.
//!
//! Each dialect handles the specific syntax differences between databases.

use crate::Value;

/// SQL dialect trait for database-specific syntax.
pub trait Dialect: Clone + Copy {
    /// Format a parameter placeholder (e.g., `$1` for Postgres, `?1` for `SQLite`).
    fn param(&self, idx: usize) -> String;

    /// Format a boolean literal.
    fn bool_lit(&self, val: bool) -> &'static str;

    /// Format the regex operator and pattern.
    /// Returns (operator, `should_transform_pattern`).
    fn regex_op(&self) -> &'static str;

    /// Format an IN clause with multiple values.
    /// Returns the SQL fragment (e.g., `= ANY($1)` or `IN (?1, ?2)`).
    fn in_clause(&self, field: &str, values: &[Value], start_idx: usize) -> (String, Vec<Value>);

    /// Format a NOT IN clause.
    fn not_in_clause(
        &self,
        field: &str,
        values: &[Value],
        start_idx: usize,
    ) -> (String, Vec<Value>);

    /// Whether ILIKE is supported natively.
    fn supports_ilike(&self) -> bool;

    /// Format a STARTS WITH clause (e.g., `LIKE $1 || '%'` or `LIKE ?1 || '%'`).
    fn starts_with_clause(&self, field: &str, idx: usize) -> String;

    /// Format an ENDS WITH clause (e.g., `LIKE '%' || $1` or `LIKE '%' || ?1`).
    fn ends_with_clause(&self, field: &str, idx: usize) -> String;

    /// Format a CONTAINS clause (e.g., `LIKE '%' || $1 || '%'` or `LIKE '%' || ?1 || '%'`).
    fn contains_clause(&self, field: &str, idx: usize) -> String;
}

/// Postgres dialect.
#[derive(Debug, Clone, Copy, Default)]
pub struct Postgres;

impl Dialect for Postgres {
    #[inline]
    fn param(&self, idx: usize) -> String {
        format!("${idx}")
    }

    #[inline]
    fn bool_lit(&self, val: bool) -> &'static str {
        if val { "TRUE" } else { "FALSE" }
    }

    #[inline]
    fn regex_op(&self) -> &'static str {
        "~"
    }

    fn in_clause(&self, field: &str, values: &[Value], start_idx: usize) -> (String, Vec<Value>) {
        // Postgres: field = ANY($1) with array parameter
        let sql = format!("{field} = ANY(${start_idx})");
        (sql, vec![Value::Array(values.to_vec())])
    }

    fn not_in_clause(
        &self,
        field: &str,
        values: &[Value],
        start_idx: usize,
    ) -> (String, Vec<Value>) {
        let sql = format!("{field} != ALL(${start_idx})");
        (sql, vec![Value::Array(values.to_vec())])
    }

    #[inline]
    fn supports_ilike(&self) -> bool {
        true
    }

    #[inline]
    fn starts_with_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE ${idx} || '%'")
    }

    #[inline]
    fn ends_with_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE '%' || ${idx}")
    }

    #[inline]
    fn contains_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE '%' || ${idx} || '%'")
    }
}

/// `SQLite` dialect.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sqlite;

impl Dialect for Sqlite {
    #[inline]
    fn param(&self, idx: usize) -> String {
        format!("?{idx}")
    }

    #[inline]
    fn bool_lit(&self, val: bool) -> &'static str {
        if val { "1" } else { "0" }
    }

    #[inline]
    fn regex_op(&self) -> &'static str {
        // SQLite doesn't have native regex, fall back to LIKE
        "LIKE"
    }

    fn in_clause(&self, field: &str, values: &[Value], start_idx: usize) -> (String, Vec<Value>) {
        // SQLite: field IN (?1, ?2, ?3) with expanded parameters
        let placeholders: Vec<String> = (0..values.len())
            .map(|i| format!("?{}", start_idx + i))
            .collect();
        let sql = format!("{} IN ({})", field, placeholders.join(", "));
        (sql, values.to_vec())
    }

    fn not_in_clause(
        &self,
        field: &str,
        values: &[Value],
        start_idx: usize,
    ) -> (String, Vec<Value>) {
        let placeholders: Vec<String> = (0..values.len())
            .map(|i| format!("?{}", start_idx + i))
            .collect();
        let sql = format!("{} NOT IN ({})", field, placeholders.join(", "));
        (sql, values.to_vec())
    }

    #[inline]
    fn supports_ilike(&self) -> bool {
        // SQLite LIKE is case-insensitive for ASCII by default
        false
    }

    #[inline]
    fn starts_with_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE ?{idx} || '%'")
    }

    #[inline]
    fn ends_with_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE '%' || ?{idx}")
    }

    #[inline]
    fn contains_clause(&self, field: &str, idx: usize) -> String {
        format!("{field} LIKE '%' || ?{idx} || '%'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_params() {
        let pg = Postgres;
        assert_eq!(pg.param(1), "$1");
        assert_eq!(pg.param(10), "$10");
    }

    #[test]
    fn test_sqlite_params() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.param(1), "?1");
        assert_eq!(sqlite.param(10), "?10");
    }

    #[test]
    fn test_postgres_bool() {
        let pg = Postgres;
        assert_eq!(pg.bool_lit(true), "TRUE");
        assert_eq!(pg.bool_lit(false), "FALSE");
    }

    #[test]
    fn test_sqlite_bool() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.bool_lit(true), "1");
        assert_eq!(sqlite.bool_lit(false), "0");
    }

    #[test]
    fn test_postgres_in_clause() {
        let pg = Postgres;
        let values = vec![Value::String("a".into()), Value::String("b".into())];
        let (sql, params) = pg.in_clause("status", &values, 1);

        assert_eq!(sql, "status = ANY($1)");
        assert_eq!(params.len(), 1); // Single array param
    }

    #[test]
    fn test_sqlite_in_clause() {
        let sqlite = Sqlite;
        let values = vec![Value::String("a".into()), Value::String("b".into())];
        let (sql, params) = sqlite.in_clause("status", &values, 1);

        assert_eq!(sql, "status IN (?1, ?2)");
        assert_eq!(params.len(), 2); // Expanded params
    }

    // --- Additional tests for regex_op ---

    #[test]
    fn test_postgres_regex_op() {
        let pg = Postgres;
        assert_eq!(pg.regex_op(), "~");
    }

    #[test]
    fn test_sqlite_regex_op() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.regex_op(), "LIKE");
    }

    // --- Tests for supports_ilike ---

    #[test]
    fn test_postgres_supports_ilike() {
        let pg = Postgres;
        assert!(pg.supports_ilike());
    }

    #[test]
    fn test_sqlite_supports_ilike() {
        let sqlite = Sqlite;
        assert!(!sqlite.supports_ilike());
    }

    // --- Tests for not_in_clause ---

    #[test]
    fn test_postgres_not_in_clause() {
        let pg = Postgres;
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let (sql, params) = pg.not_in_clause("id", &values, 1);

        assert_eq!(sql, "id != ALL($1)");
        assert_eq!(params.len(), 1);
        match &params[0] {
            Value::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("Expected array parameter"),
        }
    }

    #[test]
    fn test_sqlite_not_in_clause() {
        let sqlite = Sqlite;
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let (sql, params) = sqlite.not_in_clause("id", &values, 1);

        assert_eq!(sql, "id NOT IN (?1, ?2, ?3)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_sqlite_not_in_clause_with_offset() {
        let sqlite = Sqlite;
        let values = vec![Value::String("x".into()), Value::String("y".into())];
        let (sql, params) = sqlite.not_in_clause("name", &values, 5);

        assert_eq!(sql, "name NOT IN (?5, ?6)");
        assert_eq!(params.len(), 2);
    }

    // --- Tests for starts_with_clause ---

    #[test]
    fn test_postgres_starts_with_clause() {
        let pg = Postgres;
        assert_eq!(pg.starts_with_clause("name", 1), "name LIKE $1 || '%'");
        assert_eq!(pg.starts_with_clause("title", 5), "title LIKE $5 || '%'");
    }

    #[test]
    fn test_sqlite_starts_with_clause() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.starts_with_clause("name", 1), "name LIKE ?1 || '%'");
        assert_eq!(
            sqlite.starts_with_clause("title", 5),
            "title LIKE ?5 || '%'"
        );
    }

    // --- Tests for ends_with_clause ---

    #[test]
    fn test_postgres_ends_with_clause() {
        let pg = Postgres;
        assert_eq!(pg.ends_with_clause("name", 1), "name LIKE '%' || $1");
        assert_eq!(pg.ends_with_clause("email", 3), "email LIKE '%' || $3");
    }

    #[test]
    fn test_sqlite_ends_with_clause() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.ends_with_clause("name", 1), "name LIKE '%' || ?1");
        assert_eq!(sqlite.ends_with_clause("email", 3), "email LIKE '%' || ?3");
    }

    // --- Tests for contains_clause ---

    #[test]
    fn test_postgres_contains_clause() {
        let pg = Postgres;
        assert_eq!(pg.contains_clause("name", 1), "name LIKE '%' || $1 || '%'");
        assert_eq!(
            pg.contains_clause("description", 2),
            "description LIKE '%' || $2 || '%'"
        );
    }

    #[test]
    fn test_sqlite_contains_clause() {
        let sqlite = Sqlite;
        assert_eq!(
            sqlite.contains_clause("name", 1),
            "name LIKE '%' || ?1 || '%'"
        );
        assert_eq!(
            sqlite.contains_clause("description", 2),
            "description LIKE '%' || ?2 || '%'"
        );
    }

    // --- Tests for in_clause with different value types ---

    #[test]
    fn test_postgres_in_clause_with_ints() {
        let pg = Postgres;
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let (sql, params) = pg.in_clause("id", &values, 2);

        assert_eq!(sql, "id = ANY($2)");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_sqlite_in_clause_with_ints() {
        let sqlite = Sqlite;
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let (sql, params) = sqlite.in_clause("id", &values, 2);

        assert_eq!(sql, "id IN (?2, ?3, ?4)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_sqlite_in_clause_single_value() {
        let sqlite = Sqlite;
        let values = vec![Value::Int(42)];
        let (sql, params) = sqlite.in_clause("id", &values, 1);

        assert_eq!(sql, "id IN (?1)");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_postgres_in_clause_single_value() {
        let pg = Postgres;
        let values = vec![Value::String("only".into())];
        let (sql, params) = pg.in_clause("name", &values, 1);

        assert_eq!(sql, "name = ANY($1)");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_sqlite_in_clause_empty() {
        let sqlite = Sqlite;
        let values: Vec<Value> = vec![];
        let (sql, _params) = sqlite.in_clause("id", &values, 1);

        assert_eq!(sql, "id IN ()");
    }

    #[test]
    fn test_postgres_in_clause_empty() {
        let pg = Postgres;
        let values: Vec<Value> = vec![];
        let (sql, params) = pg.in_clause("id", &values, 1);

        assert_eq!(sql, "id = ANY($1)");
        match &params[0] {
            Value::Array(arr) => assert!(arr.is_empty()),
            _ => panic!("Expected empty array"),
        }
    }

    // --- Tests for Default trait ---

    #[test]
    fn test_postgres_default() {
        let pg = Postgres;
        assert_eq!(pg.param(1), "$1");
    }

    #[test]
    fn test_sqlite_default() {
        let sqlite = Sqlite;
        assert_eq!(sqlite.param(1), "?1");
    }

    // --- Tests for Clone and Copy ---

    #[test]
    fn test_postgres_clone_copy() {
        let pg = Postgres;
        let pg_clone = pg;
        let pg_copy = pg;
        assert_eq!(pg_clone.param(1), pg_copy.param(1));
    }

    #[test]
    fn test_sqlite_clone_copy() {
        let sqlite = Sqlite;
        let sqlite_clone = sqlite;
        let sqlite_copy = sqlite;
        assert_eq!(sqlite_clone.param(1), sqlite_copy.param(1));
    }

    // --- Tests with various start indices ---

    #[test]
    fn test_postgres_in_clause_high_index() {
        let pg = Postgres;
        let values = vec![Value::Int(1)];
        let (sql, _) = pg.in_clause("id", &values, 100);
        assert_eq!(sql, "id = ANY($100)");
    }

    #[test]
    fn test_sqlite_in_clause_high_index() {
        let sqlite = Sqlite;
        let values = vec![Value::Int(1), Value::Int(2)];
        let (sql, _) = sqlite.in_clause("id", &values, 100);
        assert_eq!(sql, "id IN (?100, ?101)");
    }

    #[test]
    fn test_sqlite_not_in_clause_high_index() {
        let sqlite = Sqlite;
        let values = vec![Value::Int(1), Value::Int(2)];
        let (sql, _) = sqlite.not_in_clause("id", &values, 50);
        assert_eq!(sql, "id NOT IN (?50, ?51)");
    }

    #[test]
    fn test_postgres_not_in_clause_high_index() {
        let pg = Postgres;
        let values = vec![Value::Int(1)];
        let (sql, _) = pg.not_in_clause("id", &values, 99);
        assert_eq!(sql, "id != ALL($99)");
    }

    // --- Tests for Debug trait ---

    #[test]
    fn test_postgres_debug() {
        let pg = Postgres;
        let debug_str = format!("{pg:?}");
        assert_eq!(debug_str, "Postgres");
    }

    #[test]
    fn test_sqlite_debug() {
        let sqlite = Sqlite;
        let debug_str = format!("{sqlite:?}");
        assert_eq!(debug_str, "Sqlite");
    }

    // --- Tests for Default trait via Default::default() ---

    #[test]
    #[allow(clippy::default_trait_access)]
    fn test_postgres_default_trait() {
        let pg: Postgres = Default::default();
        assert_eq!(pg.param(1), "$1");
    }

    #[test]
    #[allow(clippy::default_trait_access)]
    fn test_sqlite_default_trait() {
        let sqlite: Sqlite = Default::default();
        assert_eq!(sqlite.param(1), "?1");
    }

    // --- Tests for Clone trait via explicit .clone() ---

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_postgres_clone_explicit() {
        let pg = Postgres;
        let pg_cloned = pg.clone();
        assert_eq!(pg_cloned.param(1), "$1");
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_sqlite_clone_explicit() {
        let sqlite = Sqlite;
        let sqlite_cloned = sqlite.clone();
        assert_eq!(sqlite_cloned.param(1), "?1");
    }

    // --- Tests for empty not_in_clause ---

    #[test]
    fn test_sqlite_not_in_clause_empty() {
        let sqlite = Sqlite;
        let values: Vec<Value> = vec![];
        let (sql, params) = sqlite.not_in_clause("id", &values, 1);

        assert_eq!(sql, "id NOT IN ()");
        assert!(params.is_empty());
    }

    #[test]
    fn test_postgres_not_in_clause_empty() {
        let pg = Postgres;
        let values: Vec<Value> = vec![];
        let (sql, params) = pg.not_in_clause("id", &values, 1);

        assert_eq!(sql, "id != ALL($1)");
        assert_eq!(params.len(), 1);
        match &params[0] {
            Value::Array(arr) => assert!(arr.is_empty()),
            _ => panic!("Expected empty array"),
        }
    }
}
