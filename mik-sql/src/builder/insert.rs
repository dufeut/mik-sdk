//! INSERT query builder.

use crate::dialect::{Dialect, Postgres, Sqlite};
use crate::validate::assert_valid_sql_identifier;

use super::types::{QueryResult, Value};

/// Builder for INSERT queries.
#[derive(Debug)]
#[must_use = "builder does nothing until .build() is called"]
pub struct InsertBuilder<D: Dialect> {
    dialect: D,
    table: String,
    columns: Vec<String>,
    values: Vec<Vec<Value>>,
    returning: Vec<String>,
}

impl<D: Dialect> InsertBuilder<D> {
    /// Create a new insert builder.
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
            columns: Vec::new(),
            values: Vec::new(),
            returning: Vec::new(),
        }
    }

    /// Set the columns for insertion.
    ///
    /// # Panics
    ///
    /// Panics if any column name is not a valid SQL identifier.
    pub fn columns(mut self, columns: &[&str]) -> Self {
        for col in columns {
            assert_valid_sql_identifier(col, "column");
        }
        self.columns = columns.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Add a row of values.
    pub fn values(mut self, values: Vec<Value>) -> Self {
        self.values.push(values);
        self
    }

    /// Add multiple rows of values.
    pub fn values_many(mut self, rows: Vec<Vec<Value>>) -> Self {
        self.values.extend(rows);
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

    /// Build the INSERT query.
    pub fn build(self) -> QueryResult {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_idx = 1usize;

        // INSERT INTO table (columns)
        sql.push_str(&format!(
            "INSERT INTO {} ({})",
            self.table,
            self.columns.join(", ")
        ));

        // VALUES (...)
        let mut value_groups = Vec::new();
        for row in &self.values {
            let placeholders: Vec<String> = row
                .iter()
                .map(|v| {
                    let p = self.dialect.param(param_idx);
                    params.push(v.clone());
                    param_idx += 1;
                    p
                })
                .collect();
            value_groups.push(format!("({})", placeholders.join(", ")));
        }
        sql.push_str(&format!(" VALUES {}", value_groups.join(", ")));

        // RETURNING clause
        if !self.returning.is_empty() {
            sql.push_str(&format!(" RETURNING {}", self.returning.join(", ")));
        }

        QueryResult { sql, params }
    }
}

/// Create an INSERT builder for Postgres.
pub fn insert(table: impl Into<String>) -> InsertBuilder<Postgres> {
    InsertBuilder::new(Postgres, table)
}

/// Create an INSERT builder for `SQLite`.
pub fn insert_sqlite(table: impl Into<String>) -> InsertBuilder<Sqlite> {
    InsertBuilder::new(Sqlite, table)
}
