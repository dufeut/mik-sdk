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
}
