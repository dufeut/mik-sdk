//! Query builder for SQL generation with parameterization.

use crate::dialect::{Dialect, Postgres, Sqlite};
use crate::pagination::{Cursor, IntoCursor};
use crate::validate::{assert_valid_sql_expression, assert_valid_sql_identifier};

/// SQL comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// Equal: `=`
    Eq,
    /// Not equal: `!=`
    Ne,
    /// Greater than: `>`
    Gt,
    /// Greater than or equal: `>=`
    Gte,
    /// Less than: `<`
    Lt,
    /// Less than or equal: `<=`
    Lte,
    /// In array: `IN` or `= ANY`
    In,
    /// Not in array: `NOT IN` or `!= ALL`
    NotIn,
    /// Regex match: `~` (Postgres) or `LIKE` (`SQLite`)
    Regex,
    /// Pattern match: `LIKE`
    Like,
    /// Case-insensitive pattern match: `ILIKE` (Postgres) or `LIKE` (`SQLite`)
    ILike,
    /// String starts with: `LIKE $1 || '%'`
    StartsWith,
    /// String ends with: `LIKE '%' || $1`
    EndsWith,
    /// String contains: `LIKE '%' || $1 || '%'`
    Contains,
    /// Between two values: `BETWEEN $1 AND $2`
    Between,
}

/// Logical operators for compound filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    /// All conditions must match: `AND`
    And,
    /// At least one condition must match: `OR`
    Or,
    /// Negate the condition: `NOT`
    Not,
}

/// A filter expression that can be simple or compound.
#[derive(Debug, Clone)]
pub enum FilterExpr {
    /// A simple field comparison.
    Simple(Filter),
    /// A compound filter with logical operator.
    Compound(CompoundFilter),
}

/// A compound filter combining multiple expressions with a logical operator.
#[derive(Debug, Clone)]
pub struct CompoundFilter {
    pub op: LogicalOp,
    pub filters: Vec<FilterExpr>,
}

impl CompoundFilter {
    /// Create an AND compound filter.
    #[must_use]
    pub fn and(filters: Vec<FilterExpr>) -> Self {
        Self {
            op: LogicalOp::And,
            filters,
        }
    }

    /// Create an OR compound filter.
    #[must_use]
    pub fn or(filters: Vec<FilterExpr>) -> Self {
        Self {
            op: LogicalOp::Or,
            filters,
        }
    }

    /// Create a NOT compound filter (wraps a single filter).
    #[must_use]
    pub fn not(filter: FilterExpr) -> Self {
        Self {
            op: LogicalOp::Not,
            filters: vec![filter],
        }
    }
}

/// Aggregation functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunc {
    /// Count rows: `COUNT(*)`
    Count,
    /// Count distinct values: `COUNT(DISTINCT field)`
    CountDistinct,
    /// Sum values: `SUM(field)`
    Sum,
    /// Average value: `AVG(field)`
    Avg,
    /// Minimum value: `MIN(field)`
    Min,
    /// Maximum value: `MAX(field)`
    Max,
}

/// An aggregation expression.
#[derive(Debug, Clone)]
pub struct Aggregate {
    pub func: AggregateFunc,
    /// Field to aggregate, None for COUNT(*)
    pub field: Option<String>,
    /// Optional alias for the result
    pub alias: Option<String>,
}

impl Aggregate {
    /// Create a COUNT(*) aggregation.
    #[must_use]
    pub fn count() -> Self {
        Self {
            func: AggregateFunc::Count,
            field: None,
            alias: Some("count".to_string()),
        }
    }

    /// Create a COUNT(field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn count_field(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::Count,
            field: Some(field),
            alias: None,
        }
    }

    /// Create a COUNT(DISTINCT field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn count_distinct(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::CountDistinct,
            field: Some(field),
            alias: None,
        }
    }

    /// Create a SUM(field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn sum(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::Sum,
            field: Some(field),
            alias: None,
        }
    }

    /// Create an AVG(field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn avg(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::Avg,
            field: Some(field),
            alias: None,
        }
    }

    /// Create a MIN(field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn min(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::Min,
            field: Some(field),
            alias: None,
        }
    }

    /// Create a MAX(field) aggregation.
    ///
    /// # Panics
    ///
    /// Panics if the field name is not a valid SQL identifier.
    pub fn max(field: impl Into<String>) -> Self {
        let field = field.into();
        assert_valid_sql_identifier(&field, "aggregate field");
        Self {
            func: AggregateFunc::Max,
            field: Some(field),
            alias: None,
        }
    }

    /// Set an alias for the aggregation result.
    ///
    /// # Panics
    ///
    /// Panics if the alias is not a valid SQL identifier.
    pub fn as_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into();
        assert_valid_sql_identifier(&alias, "aggregate alias");
        self.alias = Some(alias);
        self
    }

    /// Generate SQL for this aggregation.
    #[must_use]
    pub fn to_sql(&self) -> String {
        let expr = match (&self.func, &self.field) {
            (AggregateFunc::Count, None) => "COUNT(*)".to_string(),
            (AggregateFunc::Count, Some(f)) => format!("COUNT({f})"),
            (AggregateFunc::CountDistinct, Some(f)) => format!("COUNT(DISTINCT {f})"),
            (AggregateFunc::Sum, Some(f)) => format!("SUM({f})"),
            (AggregateFunc::Avg, Some(f)) => format!("AVG({f})"),
            (AggregateFunc::Min, Some(f)) => format!("MIN({f})"),
            (AggregateFunc::Max, Some(f)) => format!("MAX({f})"),
            _ => "COUNT(*)".to_string(),
        };

        match &self.alias {
            Some(a) => format!("{expr} AS {a}"),
            None => expr,
        }
    }
}

/// SQL parameter values.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// Sort field with direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortField {
    pub field: String,
    pub dir: SortDir,
}

impl SortField {
    /// Create a new sort field.
    pub fn new(field: impl Into<String>, dir: SortDir) -> Self {
        Self {
            field: field.into(),
            dir,
        }
    }

    /// Parse a sort string like "name,-created_at" into sort fields.
    ///
    /// Fields prefixed with `-` are sorted descending.
    /// Validates against allowed fields list.
    ///
    /// # Security Note
    ///
    /// If `allowed` is empty, ALL fields are allowed. For user input, always
    /// provide an explicit whitelist to prevent sorting by sensitive columns.
    pub fn parse_sort_string(sort: &str, allowed: &[&str]) -> Result<Vec<SortField>, String> {
        let mut result = Vec::new();

        for part in sort.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (field, dir) = if let Some(stripped) = part.strip_prefix('-') {
                (stripped, SortDir::Desc)
            } else {
                (part, SortDir::Asc)
            };

            // Validate against whitelist (empty = allow all, consistent with FilterValidator)
            if !allowed.is_empty() && !allowed.contains(&field) {
                return Err(format!(
                    "Sort field '{field}' not allowed. Allowed: {allowed:?}"
                ));
            }

            result.push(SortField::new(field, dir));
        }

        Ok(result)
    }
}

/// Filter condition.
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub op: Operator,
    pub value: Value,
}

/// Query result with SQL string and parameters.
#[derive(Debug)]
#[must_use = "QueryResult must be used to execute the query"]
pub struct QueryResult {
    pub sql: String,
    pub params: Vec<Value>,
}

/// A computed field expression with alias.
#[derive(Debug, Clone)]
pub struct ComputedField {
    /// The alias for the computed field.
    pub alias: String,
    /// The SQL expression (e.g., "`first_name` || ' ' || `last_name`").
    pub expression: String,
}

impl ComputedField {
    /// Create a new computed field.
    pub fn new(alias: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            expression: expression.into(),
        }
    }

    /// Generate the SQL for this computed field.
    #[must_use]
    pub fn to_sql(&self) -> String {
        format!("({}) AS {}", self.expression, self.alias)
    }
}

/// Cursor pagination direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDirection {
    /// Paginate forward (after the cursor).
    After,
    /// Paginate backward (before the cursor).
    Before,
}

// ═══════════════════════════════════════════════════════════════════════════
// SHARED FILTER BUILDING FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Build a filter expression (simple or compound).
fn build_filter_expr_impl<D: Dialect>(
    dialect: &D,
    expr: &FilterExpr,
    start_idx: usize,
) -> (String, Vec<Value>, usize) {
    match expr {
        FilterExpr::Simple(filter) => build_condition_impl(dialect, filter, start_idx),
        FilterExpr::Compound(compound) => build_compound_filter_impl(dialect, compound, start_idx),
    }
}

/// Build a compound filter (AND, OR, NOT).
fn build_compound_filter_impl<D: Dialect>(
    dialect: &D,
    compound: &CompoundFilter,
    start_idx: usize,
) -> (String, Vec<Value>, usize) {
    let mut idx = start_idx;
    let mut all_params = Vec::new();
    let mut conditions = Vec::new();

    for filter_expr in &compound.filters {
        let (condition, params, new_idx) = build_filter_expr_impl(dialect, filter_expr, idx);
        conditions.push(condition);
        all_params.extend(params);
        idx = new_idx;
    }

    let sql = match compound.op {
        LogicalOp::And => {
            if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                format!("({})", conditions.join(" AND "))
            }
        },
        LogicalOp::Or => {
            if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                format!("({})", conditions.join(" OR "))
            }
        },
        LogicalOp::Not => {
            let inner = conditions.into_iter().next().unwrap_or_default();
            format!("NOT ({inner})")
        },
    };

    (sql, all_params, idx)
}

/// Build a single filter condition.
fn build_condition_impl<D: Dialect>(
    dialect: &D,
    filter: &Filter,
    start_idx: usize,
) -> (String, Vec<Value>, usize) {
    let field = &filter.field;
    let idx = start_idx;

    match (&filter.op, &filter.value) {
        // NULL handling
        (Operator::Eq, Value::Null) => (format!("{field} IS NULL"), vec![], idx),
        (Operator::Ne, Value::Null) => (format!("{field} IS NOT NULL"), vec![], idx),

        // IN/NOT IN with arrays
        (Operator::In, Value::Array(values)) => {
            let (sql, params) = dialect.in_clause(field, values, idx);
            let new_idx = idx + params.len();
            (sql, params, new_idx)
        },
        (Operator::NotIn, Value::Array(values)) => {
            let (sql, params) = dialect.not_in_clause(field, values, idx);
            let new_idx = idx + params.len();
            (sql, params, new_idx)
        },

        // Boolean values - parameterized
        (Operator::Eq, Value::Bool(_)) => {
            let sql = format!("{} = {}", field, dialect.param(idx));
            (sql, vec![filter.value.clone()], idx + 1)
        },
        (Operator::Ne, Value::Bool(_)) => {
            let sql = format!("{} != {}", field, dialect.param(idx));
            (sql, vec![filter.value.clone()], idx + 1)
        },

        // Regex
        (Operator::Regex, value) => {
            let op = dialect.regex_op();
            let sql = format!("{} {} {}", field, op, dialect.param(idx));
            (sql, vec![value.clone()], idx + 1)
        },

        // ILIKE (falls back to LIKE on SQLite)
        (Operator::ILike, value) => {
            let sql = if dialect.supports_ilike() {
                format!("{} ILIKE {}", field, dialect.param(idx))
            } else {
                format!("{} LIKE {}", field, dialect.param(idx))
            };
            (sql, vec![value.clone()], idx + 1)
        },

        // String pattern operators
        (Operator::StartsWith, value) => {
            let sql = dialect.starts_with_clause(field, idx);
            (sql, vec![value.clone()], idx + 1)
        },
        (Operator::EndsWith, value) => {
            let sql = dialect.ends_with_clause(field, idx);
            (sql, vec![value.clone()], idx + 1)
        },
        (Operator::Contains, value) => {
            let sql = dialect.contains_clause(field, idx);
            (sql, vec![value.clone()], idx + 1)
        },

        // BETWEEN operator - takes an array with exactly 2 values
        (Operator::Between, Value::Array(values)) => {
            if values.len() != 2 {
                // Return a safe fallback that produces no results (consistent in debug and release)
                return (
                    format!("1=0 /* BETWEEN requires 2 values, got {} */", values.len()),
                    vec![],
                    idx,
                );
            }
            let sql = format!(
                "{} BETWEEN {} AND {}",
                field,
                dialect.param(idx),
                dialect.param(idx + 1)
            );
            (sql, values.clone(), idx + 2)
        },

        // Standard comparisons
        (op, value) => {
            let op_str = match op {
                Operator::Eq => "=",
                Operator::Ne => "!=",
                Operator::Gt => ">",
                Operator::Gte => ">=",
                Operator::Lt => "<",
                Operator::Lte => "<=",
                Operator::Like => "LIKE",
                _ => "=", // fallback for unhandled cases
            };
            let sql = format!("{} {} {}", field, op_str, dialect.param(idx));
            (sql, vec![value.clone()], idx + 1)
        },
    }
}

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
    /// ```ignore
    /// .computed("full_name", "first_name || ' ' || last_name")
    /// .computed("line_total", "quantity * price")
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
    pub fn page(mut self, page: u32, limit: u32) -> Self {
        self.limit = Some(limit);
        self.offset = Some(page.saturating_sub(1).saturating_mul(limit));
        self
    }

    /// Set explicit limit and offset.
    pub fn limit_offset(mut self, limit: u32, offset: u32) -> Self {
        self.limit = Some(limit);
        self.offset = Some(offset);
        self
    }

    /// Set a limit without offset.
    pub fn limit(mut self, limit: u32) -> Self {
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

/// Helper function to create a simple filter expression.
///
/// # Panics
///
/// Panics if the field name is not a valid SQL identifier.
pub fn simple(field: impl Into<String>, op: Operator, value: Value) -> FilterExpr {
    let field = field.into();
    assert_valid_sql_identifier(&field, "filter field");
    FilterExpr::Simple(Filter { field, op, value })
}

/// Helper function to create an AND compound filter.
#[must_use]
pub fn and(filters: Vec<FilterExpr>) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::and(filters))
}

/// Helper function to create an OR compound filter.
#[must_use]
pub fn or(filters: Vec<FilterExpr>) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::or(filters))
}

/// Helper function to create a NOT filter.
#[must_use]
pub fn not(filter: FilterExpr) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::not(filter))
}

// ═══════════════════════════════════════════════════════════════════════════
// INSERT BUILDER
// ═══════════════════════════════════════════════════════════════════════════

/// Builder for INSERT queries.
#[derive(Debug)]
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

// ═══════════════════════════════════════════════════════════════════════════
// UPDATE BUILDER
// ═══════════════════════════════════════════════════════════════════════════

/// Builder for UPDATE queries.
#[derive(Debug)]
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

// ═══════════════════════════════════════════════════════════════════════════
// DELETE BUILDER
// ═══════════════════════════════════════════════════════════════════════════

/// Builder for DELETE queries.
#[derive(Debug)]
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

// ═══════════════════════════════════════════════════════════════════════════
// CONVENIENCE CONSTRUCTORS
// ═══════════════════════════════════════════════════════════════════════════

/// Create an INSERT builder for Postgres.
pub fn insert(table: impl Into<String>) -> InsertBuilder<Postgres> {
    InsertBuilder::new(Postgres, table)
}

/// Create an UPDATE builder for Postgres.
pub fn update(table: impl Into<String>) -> UpdateBuilder<Postgres> {
    UpdateBuilder::new(Postgres, table)
}

/// Create a DELETE builder for Postgres.
pub fn delete(table: impl Into<String>) -> DeleteBuilder<Postgres> {
    DeleteBuilder::new(Postgres, table)
}

/// Create an INSERT builder for `SQLite`.
pub fn insert_sqlite(table: impl Into<String>) -> InsertBuilder<Sqlite> {
    InsertBuilder::new(Sqlite, table)
}

/// Create an UPDATE builder for `SQLite`.
pub fn update_sqlite(table: impl Into<String>) -> UpdateBuilder<Sqlite> {
    UpdateBuilder::new(Sqlite, table)
}

/// Create a DELETE builder for `SQLite`.
pub fn delete_sqlite(table: impl Into<String>) -> DeleteBuilder<Sqlite> {
    DeleteBuilder::new(Sqlite, table)
}
