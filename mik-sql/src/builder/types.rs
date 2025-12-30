//! Core types for the SQL query builder.

use crate::validate::assert_valid_sql_identifier;

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
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    /// A simple field comparison.
    Simple(Filter),
    /// A compound filter with logical operator.
    Compound(CompoundFilter),
}

/// A compound filter combining multiple expressions with a logical operator.
#[derive(Debug, Clone, PartialEq)]
pub struct CompoundFilter {
    pub op: LogicalOp,
    pub filters: Vec<FilterExpr>,
}

impl CompoundFilter {
    /// Create an AND compound filter.
    #[must_use]
    pub const fn and(filters: Vec<FilterExpr>) -> Self {
        Self {
            op: LogicalOp::And,
            filters,
        }
    }

    /// Create an OR compound filter.
    #[must_use]
    pub const fn or(filters: Vec<FilterExpr>) -> Self {
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
    pub fn parse_sort_string(sort: &str, allowed: &[&str]) -> Result<Vec<Self>, String> {
        let mut result = Vec::new();

        for part in sort.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (field, dir) = part
                .strip_prefix('-')
                .map_or((part, SortDir::Asc), |stripped| (stripped, SortDir::Desc));

            // Validate against whitelist (empty = allow all, consistent with FilterValidator)
            if !allowed.is_empty() && !allowed.contains(&field) {
                return Err(format!(
                    "Sort field '{field}' not allowed. Allowed: {allowed:?}"
                ));
            }

            result.push(Self::new(field, dir));
        }

        Ok(result)
    }
}

/// Filter condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub field: String,
    pub op: Operator,
    pub value: Value,
}

/// Query result with SQL string and parameters.
#[derive(Debug, Clone, PartialEq)]
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
pub const fn and(filters: Vec<FilterExpr>) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::and(filters))
}

/// Helper function to create an OR compound filter.
#[must_use]
pub const fn or(filters: Vec<FilterExpr>) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::or(filters))
}

/// Helper function to create a NOT filter.
#[must_use]
pub fn not(filter: FilterExpr) -> FilterExpr {
    FilterExpr::Compound(CompoundFilter::not(filter))
}
