//! DELETE query builder.

use crate::dialect::{Dialect, Postgres, Sqlite};
use crate::validate::assert_valid_sql_identifier;

use super::filter::{build_condition_impl, build_filter_expr_impl};
use super::types::{Filter, FilterExpr, Operator, QueryResult, Value};

/// Builder for DELETE queries.
#[derive(Debug)]
#[must_use = "builder does nothing until .build() is called"]
pub struct DeleteBuilder<D: Dialect> {
    dialect: D,
    table: String,
    filters: Vec<Filter>,
    filter_expr: Option<FilterExpr>,
    returning: Vec<String>,
}

impl<D: Dialect> DeleteBuilder<D> {
    /// Create a new delete builder.
    ///
    /// # Panics
    ///
    /// Panics if the table name is not a valid SQL identifier.
    pub fn new(dialect: D, table: impl Into<String>) -> Self {
        let table = table.into();
        assert_valid_sql_identifier(&table, "table");
        Self {
            dialect,
            table,
            filters: Vec::new(),
            filter_expr: None,
            returning: Vec::new(),
        }
    }

    /// Add a simple WHERE filter.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn filter(mut self, field: impl Into<String>, op: Operator, value: Value) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "filter field");
        self.filters.push(Filter { field, op, value });
        self
    }

    /// Set a compound filter expression (AND, OR, NOT).
    /// Use with `simple()`, `and()`, `or()`, `not()` helpers.
    pub fn filter_expr(mut self, expr: FilterExpr) -> Self {
        self.filter_expr = Some(expr);
        self
    }

    /// Add RETURNING clause (Postgres/SQLite 3.35+).
    ///
    /// # Panics
    ///
    /// Panics if any column name is not a valid SQL identifier.
    pub fn returning(mut self, columns: &[&str]) -> Self {
        for col in columns {
            assert_valid_sql_identifier(col, "returning column");
        }
        self.returning = columns.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Build the DELETE query.
    pub fn build(self) -> QueryResult {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_idx = 1usize;

        // DELETE FROM table
        sql.push_str(&format!("DELETE FROM {}", self.table));

        // WHERE clause - combine filter_expr and simple filters
        let has_filter_expr = self.filter_expr.is_some();
        let has_simple_filters = !self.filters.is_empty();

        if has_filter_expr || has_simple_filters {
            sql.push_str(" WHERE ");
            let mut all_conditions = Vec::new();

            if let Some(ref expr) = self.filter_expr {
                let (condition, new_params, new_idx) =
                    build_filter_expr_impl(&self.dialect, expr, param_idx);
                all_conditions.push(condition);
                params.extend(new_params);
                param_idx = new_idx;
            }

            for filter in &self.filters {
                let (condition, new_params, new_idx) =
                    build_condition_impl(&self.dialect, filter, param_idx);
                all_conditions.push(condition);
                params.extend(new_params);
                param_idx = new_idx;
            }

            sql.push_str(&all_conditions.join(" AND "));
        }

        // RETURNING clause
        if !self.returning.is_empty() {
            sql.push_str(&format!(" RETURNING {}", self.returning.join(", ")));
        }

        QueryResult { sql, params }
    }
}

/// Create a DELETE builder for Postgres.
pub fn delete(table: impl Into<String>) -> DeleteBuilder<Postgres> {
    DeleteBuilder::new(Postgres, table)
}

/// Create a DELETE builder for `SQLite`.
pub fn delete_sqlite(table: impl Into<String>) -> DeleteBuilder<Sqlite> {
    DeleteBuilder::new(Sqlite, table)
}
