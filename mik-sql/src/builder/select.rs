//! SELECT query builder.

use crate::dialect::Dialect;
use crate::pagination::{Cursor, IntoCursor};
use crate::validate::{assert_valid_sql_expression, assert_valid_sql_identifier};

use super::filter::{build_condition_impl, build_filter_expr_impl};
use super::types::{
    Aggregate, CompoundFilter, ComputedField, CursorDirection, Filter, FilterExpr, Operator,
    QueryResult, SortDir, SortField, Value,
};

/// SQL query builder with dialect support.
#[derive(Debug)]
pub struct QueryBuilder<D: Dialect> {
    dialect: D,
    table: String,
    fields: Vec<String>,
    computed: Vec<ComputedField>,
    aggregates: Vec<Aggregate>,
    filters: Vec<Filter>,
    filter_expr: Option<FilterExpr>,
    group_by: Vec<String>,
    having: Option<FilterExpr>,
    sorts: Vec<SortField>,
    limit: Option<u32>,
    offset: Option<u32>,
    cursor: Option<Cursor>,
    cursor_direction: Option<CursorDirection>,
}

impl<D: Dialect> QueryBuilder<D> {
    /// Create a new query builder for the given table.
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
            fields: Vec::new(),
            computed: Vec::new(),
            aggregates: Vec::new(),
            filters: Vec::new(),
            filter_expr: None,
            group_by: Vec::new(),
            having: None,
            sorts: Vec::new(),
            limit: None,
            offset: None,
            cursor: None,
            cursor_direction: None,
        }
    }

    /// Set the fields to SELECT.
    ///
    /// # Panics
    ///
    /// Panics if any field name is not a valid SQL identifier.
    pub fn fields(mut self, fields: &[&str]) -> Self {
        for field in fields {
            assert_valid_sql_identifier(field, "field");
        }
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Add a computed field to the SELECT clause.
    ///
    /// # Example
    ///
    /// ```
    /// # use mik_sql::prelude::*;
    /// let result = postgres("orders")
    ///     .computed("line_total", "quantity * price")
    ///     .build();
    /// assert!(result.sql.contains("(quantity * price) AS line_total"));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if alias is not a valid SQL identifier or expression contains
    /// dangerous patterns (comments, semicolons, SQL keywords).
    ///
    /// # Security
    ///
    /// **WARNING**: Only use with trusted expressions from code, never with user input.
    pub fn computed(mut self, alias: impl Into<String>, expression: impl Into<String>) -> Self {
        let alias = alias.into();
        let expression = expression.into();
        assert_valid_sql_identifier(&alias, "computed field alias");
        assert_valid_sql_expression(&expression, "computed field");
        self.computed.push(ComputedField::new(alias, expression));
        self
    }

    /// Add an aggregation to the SELECT clause.
    pub fn aggregate(mut self, agg: Aggregate) -> Self {
        self.aggregates.push(agg);
        self
    }

    /// Add a COUNT(*) aggregation.
    pub fn count(mut self) -> Self {
        self.aggregates.push(Aggregate::count());
        self
    }

    /// Add a SUM(field) aggregation.
    pub fn sum(mut self, field: impl Into<String>) -> Self {
        self.aggregates.push(Aggregate::sum(field));
        self
    }

    /// Add an AVG(field) aggregation.
    pub fn avg(mut self, field: impl Into<String>) -> Self {
        self.aggregates.push(Aggregate::avg(field));
        self
    }

    /// Add a MIN(field) aggregation.
    pub fn min(mut self, field: impl Into<String>) -> Self {
        self.aggregates.push(Aggregate::min(field));
        self
    }

    /// Add a MAX(field) aggregation.
    pub fn max(mut self, field: impl Into<String>) -> Self {
        self.aggregates.push(Aggregate::max(field));
        self
    }

    /// Add a filter condition.
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

    /// Set a compound filter expression (replaces simple filters for WHERE clause).
    pub fn filter_expr(mut self, expr: FilterExpr) -> Self {
        self.filter_expr = Some(expr);
        self
    }

    /// Add an AND compound filter.
    pub fn and(mut self, filters: Vec<FilterExpr>) -> Self {
        self.filter_expr = Some(FilterExpr::Compound(CompoundFilter::and(filters)));
        self
    }

    /// Add an OR compound filter.
    pub fn or(mut self, filters: Vec<FilterExpr>) -> Self {
        self.filter_expr = Some(FilterExpr::Compound(CompoundFilter::or(filters)));
        self
    }

    /// Add GROUP BY fields.
    ///
    /// # Panics
    ///
    /// Panics if any field name is not a valid SQL identifier.
    pub fn group_by(mut self, fields: &[&str]) -> Self {
        for field in fields {
            assert_valid_sql_identifier(field, "group by field");
        }
        self.group_by = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Add a HAVING clause (for filtering aggregated results).
    pub fn having(mut self, expr: FilterExpr) -> Self {
        self.having = Some(expr);
        self
    }

    /// Add a sort field.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn sort(mut self, field: impl Into<String>, dir: SortDir) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "sort field");
        self.sorts.push(SortField::new(field, dir));
        self
    }

    /// Add multiple sort fields.
    pub fn sorts(mut self, sorts: &[SortField]) -> Self {
        self.sorts.extend(sorts.iter().cloned());
        self
    }

    /// Set pagination with page number (1-indexed) and limit.
    pub const fn page(mut self, page: u32, limit: u32) -> Self {
        self.limit = Some(limit);
        self.offset = Some(page.saturating_sub(1).saturating_mul(limit));
        self
    }

    /// Set explicit limit and offset.
    pub const fn limit_offset(mut self, limit: u32, offset: u32) -> Self {
        self.limit = Some(limit);
        self.offset = Some(offset);
        self
    }

    /// Set a limit without offset.
    pub const fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Paginate after this cursor (forward pagination).
    ///
    /// This method accepts flexible input types for great DX:
    /// - `&Cursor` - when you have an already-parsed cursor
    /// - `&str` - automatically decodes the base64 cursor
    /// - `Option<&str>` - perfect for `req.query("after")` results
    ///
    /// If the cursor is invalid or None, it's silently ignored.
    /// This makes it safe to pass `req.query("after")` directly.
    pub fn after_cursor(mut self, cursor: impl IntoCursor) -> Self {
        if let Some(c) = cursor.into_cursor() {
            self.cursor = Some(c);
            self.cursor_direction = Some(CursorDirection::After);
        }
        self
    }

    /// Paginate before this cursor (backward pagination).
    ///
    /// This method accepts flexible input types for great DX:
    /// - `&Cursor` - when you have an already-parsed cursor
    /// - `&str` - automatically decodes the base64 cursor
    /// - `Option<&str>` - perfect for `req.query("before")` results
    ///
    /// If the cursor is invalid or None, it's silently ignored.
    pub fn before_cursor(mut self, cursor: impl IntoCursor) -> Self {
        if let Some(c) = cursor.into_cursor() {
            self.cursor = Some(c);
            self.cursor_direction = Some(CursorDirection::Before);
        }
        self
    }

    /// Build the SQL query and parameters.
    pub fn build(self) -> QueryResult {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_idx = 1usize;

        // SELECT clause
        let mut select_parts = Vec::new();

        // Add regular fields
        if !self.fields.is_empty() {
            select_parts.extend(self.fields.clone());
        }

        // Add computed fields
        for comp in &self.computed {
            select_parts.push(comp.to_sql());
        }

        // Add aggregations
        for agg in &self.aggregates {
            select_parts.push(agg.to_sql());
        }

        let select_str = if select_parts.is_empty() {
            "*".to_string()
        } else {
            select_parts.join(", ")
        };

        sql.push_str(&format!("SELECT {} FROM {}", select_str, self.table));

        // WHERE clause - combine filter_expr, simple filters, and cursor conditions
        let has_filter_expr = self.filter_expr.is_some();
        let has_simple_filters = !self.filters.is_empty();
        let has_cursor = self.cursor.is_some() && self.cursor_direction.is_some();

        if has_filter_expr || has_simple_filters || has_cursor {
            sql.push_str(" WHERE ");
            let mut all_conditions = Vec::new();

            // Add filter_expr conditions first
            if let Some(ref expr) = self.filter_expr {
                let (condition, new_params, new_idx) =
                    build_filter_expr_impl(&self.dialect, expr, param_idx);
                all_conditions.push(condition);
                params.extend(new_params);
                param_idx = new_idx;
            }

            // Add simple filters (from merge or direct .filter() calls)
            for filter in &self.filters {
                let (condition, new_params, new_idx) =
                    build_condition_impl(&self.dialect, filter, param_idx);
                all_conditions.push(condition);
                params.extend(new_params);
                param_idx = new_idx;
            }

            // Add cursor pagination conditions
            if let (Some(cursor), Some(direction)) = (&self.cursor, self.cursor_direction) {
                let (condition, new_params, new_idx) =
                    self.build_cursor_condition(cursor, direction, param_idx);
                if !condition.is_empty() {
                    all_conditions.push(condition);
                    params.extend(new_params);
                    param_idx = new_idx;
                }
            }

            sql.push_str(&all_conditions.join(" AND "));
        }

        // GROUP BY clause
        if !self.group_by.is_empty() {
            sql.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
        }

        // HAVING clause
        // Note: _new_idx intentionally unused - ORDER BY/LIMIT/OFFSET don't use parameters
        if let Some(ref expr) = self.having {
            let (condition, new_params, _new_idx) =
                build_filter_expr_impl(&self.dialect, expr, param_idx);
            sql.push_str(&format!(" HAVING {condition}"));
            params.extend(new_params);
        }

        // ORDER BY clause
        if !self.sorts.is_empty() {
            sql.push_str(" ORDER BY ");
            let sort_parts: Vec<String> = self
                .sorts
                .iter()
                .map(|s| {
                    let dir = match s.dir {
                        SortDir::Asc => "ASC",
                        SortDir::Desc => "DESC",
                    };
                    format!("{} {}", s.field, dir)
                })
                .collect();
            sql.push_str(&sort_parts.join(", "));
        }

        // LIMIT/OFFSET clause
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        QueryResult { sql, params }
    }

    /// Build cursor pagination condition.
    ///
    /// Generates keyset-style WHERE conditions based on sort fields and cursor values.
    /// For single field: `field > $1` (or `<` for DESC)
    /// For multiple fields: `(a, b) > ($1, $2)` using row comparison.
    fn build_cursor_condition(
        &self,
        cursor: &Cursor,
        direction: CursorDirection,
        start_idx: usize,
    ) -> (String, Vec<Value>, usize) {
        // If no sorts defined, try using cursor fields directly with ascending order
        let sort_fields: Vec<SortField> = if self.sorts.is_empty() {
            cursor
                .fields
                .iter()
                .map(|(name, _)| SortField::new(name.clone(), SortDir::Asc))
                .collect()
        } else {
            self.sorts.clone()
        };

        if sort_fields.is_empty() {
            return (String::new(), vec![], start_idx);
        }

        // Collect values for each sort field from cursor
        let mut cursor_values: Vec<(&str, &Value)> = Vec::new();
        for sort in &sort_fields {
            if let Some((_, value)) = cursor.fields.iter().find(|(name, _)| name == &sort.field) {
                cursor_values.push((&sort.field, value));
            }
        }

        if cursor_values.is_empty() {
            return (String::new(), vec![], start_idx);
        }

        let mut idx = start_idx;
        let mut params = Vec::new();

        if cursor_values.len() == 1 {
            // Single field: simple comparison
            let (field, value) = cursor_values[0];
            let sort = &sort_fields[0];
            let op = match (direction, sort.dir) {
                (CursorDirection::After, SortDir::Asc) => ">",
                (CursorDirection::After, SortDir::Desc) => "<",
                (CursorDirection::Before, SortDir::Asc) => "<",
                (CursorDirection::Before, SortDir::Desc) => ">",
            };

            let sql = format!("{} {} {}", field, op, self.dialect.param(idx));
            params.push(value.clone());
            idx += 1;

            (sql, params, idx)
        } else {
            // Multiple fields: use row/tuple comparison for efficiency
            // (a, b, c) > ($1, $2, $3) handles lexicographic ordering correctly
            let fields: Vec<&str> = cursor_values.iter().map(|(f, _)| *f).collect();
            let placeholders: Vec<String> = cursor_values
                .iter()
                .enumerate()
                .map(|(i, (_, value))| {
                    params.push((*value).clone());
                    self.dialect.param(idx + i)
                })
                .collect();
            idx += cursor_values.len();

            // Determine comparison operator based on primary sort direction
            let primary_dir = sort_fields[0].dir;
            let op = match (direction, primary_dir) {
                (CursorDirection::After, SortDir::Asc) => ">",
                (CursorDirection::After, SortDir::Desc) => "<",
                (CursorDirection::Before, SortDir::Asc) => "<",
                (CursorDirection::Before, SortDir::Desc) => ">",
            };

            let sql = format!(
                "({}) {} ({})",
                fields.join(", "),
                op,
                placeholders.join(", ")
            );

            (sql, params, idx)
        }
    }
}
