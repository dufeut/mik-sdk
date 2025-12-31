//! UPDATE query builder.

use crate::dialect::{Dialect, Postgres, Sqlite};
use crate::validate::assert_valid_sql_identifier;

use super::filter::{build_condition_impl, build_filter_expr_impl};
use super::types::{Filter, FilterExpr, Operator, QueryResult, Value};

/// Builder for UPDATE queries.
#[derive(Debug)]
#[must_use = "builder does nothing until .build() is called"]
pub struct UpdateBuilder<D: Dialect> {
    dialect: D,
    table: String,
    sets: Vec<(String, Value)>,
    filters: Vec<Filter>,
    filter_expr: Option<FilterExpr>,
    returning: Vec<String>,
}

impl<D: Dialect> UpdateBuilder<D> {
    /// Create a new update builder.
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
            sets: Vec::new(),
            filters: Vec::new(),
            filter_expr: None,
            returning: Vec::new(),
        }
    }

    /// Set a column to a value.
    ///
    /// # Panics
    ///
    /// Panics if the column name is not a valid SQL identifier.
    pub fn set(mut self, column: impl Into<String>, value: Value) -> Self {
        let column = column.into();
        assert_valid_sql_identifier(&column, "column");
        self.sets.push((column, value));
        self
    }

    /// Set multiple columns at once.
    ///
    /// # Panics
    ///
    /// Panics if any column name is not a valid SQL identifier.
    pub fn set_many(mut self, pairs: Vec<(&str, Value)>) -> Self {
        for (col, val) in pairs {
            assert_valid_sql_identifier(col, "column");
            self.sets.push((col.to_string(), val));
        }
        self
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

    /// Add RETURNING clause (Postgres).
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

    /// Build the UPDATE query.
    pub fn build(self) -> QueryResult {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_idx = 1usize;

        // UPDATE table SET col = val, ...
        sql.push_str(&format!("UPDATE {} SET ", self.table));

        let set_parts: Vec<String> = self
            .sets
            .iter()
            .map(|(col, val)| {
                let p = self.dialect.param(param_idx);
                params.push(val.clone());
                param_idx += 1;
                format!("{col} = {p}")
            })
            .collect();
        sql.push_str(&set_parts.join(", "));

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

/// Create an UPDATE builder for Postgres.
pub fn update(table: impl Into<String>) -> UpdateBuilder<Postgres> {
    UpdateBuilder::new(Postgres, table)
}

/// Create an UPDATE builder for `SQLite`.
pub fn update_sqlite(table: impl Into<String>) -> UpdateBuilder<Sqlite> {
    UpdateBuilder::new(Sqlite, table)
}
