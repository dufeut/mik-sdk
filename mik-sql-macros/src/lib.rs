//! Proc-macros for mik-sql - SQL query builder with Mongo-style filters.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, LitBool, LitFloat, LitInt, LitStr, Result, Token, braced, bracketed,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token,
};

// ============================================================================
// SQL CRUD MACROS - Query builder with JSON-like syntax
// ============================================================================

/// SQL dialect for query generation.
#[derive(Clone, Copy, Default)]
enum SqlDialect {
    #[default]
    Postgres,
    Sqlite,
}

impl SqlDialect {
    /// Parse dialect from identifier, returns None if not a dialect keyword.
    fn from_ident(ident: &syn::Ident) -> Option<Self> {
        match ident.to_string().as_str() {
            "postgres" | "pg" => Some(SqlDialect::Postgres),
            "sqlite" => Some(SqlDialect::Sqlite),
            _ => None,
        }
    }

    /// Generate the query builder constructor call.
    fn builder_tokens(self, table: &str) -> TokenStream2 {
        match self {
            SqlDialect::Postgres => quote! { ::mik_sql::postgres(#table) },
            SqlDialect::Sqlite => quote! { ::mik_sql::sqlite(#table) },
        }
    }

    /// Generate the insert builder constructor call.
    fn insert_tokens(self, table: &str) -> TokenStream2 {
        match self {
            SqlDialect::Postgres => quote! { ::mik_sql::insert(#table) },
            SqlDialect::Sqlite => quote! { ::mik_sql::insert_sqlite(#table) },
        }
    }

    /// Generate the update builder constructor call.
    fn update_tokens(self, table: &str) -> TokenStream2 {
        match self {
            SqlDialect::Postgres => quote! { ::mik_sql::update(#table) },
            SqlDialect::Sqlite => quote! { ::mik_sql::update_sqlite(#table) },
        }
    }

    /// Generate the delete builder constructor call.
    fn delete_tokens(self, table: &str) -> TokenStream2 {
        match self {
            SqlDialect::Postgres => quote! { ::mik_sql::delete(#table) },
            SqlDialect::Sqlite => quote! { ::mik_sql::delete_sqlite(#table) },
        }
    }
}

/// Parse optional dialect prefix: `sql_read!(sqlite, users { ... })`
fn parse_optional_dialect(input: ParseStream) -> Result<SqlDialect> {
    let fork = input.fork();
    if let Ok(ident) = fork.parse::<syn::Ident>()
        && let Some(dialect) = SqlDialect::from_ident(&ident)
        && fork.peek(Token![,])
    {
        input.parse::<syn::Ident>()?;
        input.parse::<Token![,]>()?;
        return Ok(dialect);
    }
    Ok(SqlDialect::default())
}

/// Input for the [`sql_read!`] macro.
struct SqlInput {
    dialect: SqlDialect,
    table: syn::Ident,
    select_fields: Vec<syn::Ident>,
    computed: Vec<SqlCompute>,
    aggregates: Vec<SqlAggregate>,
    filter_expr: Option<SqlFilterExpr>,
    group_by: Vec<syn::Ident>,
    having: Option<SqlFilterExpr>,
    sorts: Vec<SqlSort>,
    dynamic_sort: Option<Expr>,
    allow_sort: Vec<syn::Ident>,
    merge_filters: Option<Expr>,
    allow_fields: Vec<syn::Ident>,
    deny_ops: Vec<syn::Ident>,
    max_depth: Option<Expr>,
    page: Option<Expr>,
    limit: Option<Expr>,
    offset: Option<Expr>,
    after: Option<Expr>,
    before: Option<Expr>,
}

/// An aggregation in the sql! macro.
struct SqlAggregate {
    func: SqlAggregateFunc,
    field: Option<syn::Ident>,
    alias: Option<syn::Ident>,
}

/// Aggregation functions.
#[derive(Clone, Copy)]
enum SqlAggregateFunc {
    Count,
    CountDistinct,
    Sum,
    Avg,
    Min,
    Max,
}

/// A filter expression - can be simple or compound.
enum SqlFilterExpr {
    Simple(SqlFilter),
    Compound {
        op: SqlLogicalOp,
        filters: Vec<SqlFilterExpr>,
    },
}

/// Logical operators for compound filters.
#[derive(Clone, Copy)]
enum SqlLogicalOp {
    And,
    Or,
    Not,
}

/// A filter condition in the sql! macro.
struct SqlFilter {
    field: syn::Ident,
    op: SqlOperator,
    value: SqlValue,
}

/// SQL operators.
#[derive(Clone)]
enum SqlOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    Like,
    ILike,
    Regex,
    StartsWith,
    EndsWith,
    Contains,
    Between,
}

/// Value in a filter.
enum SqlValue {
    Null,
    Bool(bool),
    Int(LitInt),
    Float(LitFloat),
    String(LitStr),
    Array(Vec<SqlValue>),
    IntHint(Expr),
    StrHint(Expr),
    FloatHint(Expr),
    BoolHint(Expr),
    Expr(Expr),
}

/// Sort field with direction.
struct SqlSort {
    field: syn::Ident,
    desc: bool,
}

/// A computed field in the sql! macro.
struct SqlCompute {
    alias: syn::Ident,
    expr: SqlComputeExpr,
}

/// A compute expression (arithmetic, function call, or literal).
enum SqlComputeExpr {
    Column(syn::Ident),
    LitStr(LitStr),
    LitInt(LitInt),
    LitFloat(LitFloat),
    BinOp {
        left: Box<SqlComputeExpr>,
        op: SqlComputeBinOp,
        right: Box<SqlComputeExpr>,
    },
    Func {
        name: SqlComputeFunc,
        args: Vec<SqlComputeExpr>,
    },
    Paren(Box<SqlComputeExpr>),
}

/// Binary operators for compute expressions.
#[derive(Clone, Copy)]
enum SqlComputeBinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Whitelisted compute functions.
#[derive(Clone, Copy)]
enum SqlComputeFunc {
    Concat,
    Coalesce,
    Upper,
    Lower,
    Round,
    Abs,
    Length,
}

impl Parse for SqlInput {
    // Complex DSL parsing requires seeing full flow in one place
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream) -> Result<Self> {
        let dialect = parse_optional_dialect(input)?;
        let table: syn::Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected table name.\n\
                     Usage: sql_read!(table_name {{ ... }}) or sql_read!(sqlite, table_name {{ ... }})\n\
                     Original error: {e}"
                ),
            )
        })?;

        let content;
        braced!(content in input);

        let mut select_fields = Vec::new();
        let mut computed = Vec::new();
        let mut aggregates = Vec::new();
        let mut filter_expr = None;
        let mut group_by = Vec::new();
        let mut having = None;
        let mut sorts = Vec::new();
        let mut dynamic_sort = None;
        let mut allow_sort = Vec::new();
        let mut merge_filters = None;
        let mut allow_fields = Vec::new();
        let mut deny_ops = Vec::new();
        let mut max_depth = None;
        let mut page = None;
        let mut limit = None;
        let mut offset = None;
        let mut after = None;
        let mut before = None;

        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "select" => {
                    let fields_content;
                    bracketed!(fields_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        fields_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    select_fields = fields.into_iter().collect();
                },
                "compute" => {
                    let compute_content;
                    braced!(compute_content in content);
                    computed = parse_compute_fields(&compute_content)?;
                },
                "aggregate" | "agg" => {
                    let agg_content;
                    braced!(agg_content in content);
                    aggregates = parse_aggregates(&agg_content)?;
                },
                "filter" => {
                    let filter_content;
                    braced!(filter_content in content);
                    filter_expr = Some(parse_filter_block(&filter_content)?);
                },
                "group_by" | "groupBy" => {
                    let group_content;
                    bracketed!(group_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        group_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    group_by = fields.into_iter().collect();
                },
                "having" => {
                    let having_content;
                    braced!(having_content in content);
                    having = Some(parse_filter_block(&having_content)?);
                },
                "order" => {
                    if content.peek(token::Bracket) {
                        let order_content;
                        bracketed!(order_content in content);
                        let sort_items: Punctuated<SqlSort, Token![,]> =
                            order_content.parse_terminated(SqlSort::parse, Token![,])?;
                        sorts = sort_items.into_iter().collect();
                    } else if content.peek(Token![-]) {
                        let sort: SqlSort = content.parse()?;
                        sorts.push(sort);
                    } else if content.peek(syn::Ident)
                        && !content.peek2(Token![,])
                        && !content.peek2(token::Brace)
                    {
                        let fork = content.fork();
                        let ident: syn::Ident = fork.parse()?;
                        if fork.peek(Token![,]) && fork.peek2(syn::Ident) {
                            fork.parse::<Token![,]>().ok();
                            if let Ok(_next_ident) = fork.parse::<syn::Ident>() {
                                if fork.peek(Token![:]) {
                                    dynamic_sort = Some(syn::Expr::Path(syn::ExprPath {
                                        attrs: vec![],
                                        qself: None,
                                        path: ident.clone().into(),
                                    }));
                                    content.parse::<syn::Ident>()?;
                                } else {
                                    let sort: SqlSort = content.parse()?;
                                    sorts.push(sort);
                                }
                            } else {
                                let sort: SqlSort = content.parse()?;
                                sorts.push(sort);
                            }
                        } else if fork.is_empty()
                            || (fork.peek(Token![,]) && !fork.peek2(syn::Ident))
                        {
                            dynamic_sort = Some(content.parse()?);
                        } else {
                            let sort: SqlSort = content.parse()?;
                            sorts.push(sort);
                        }
                    } else {
                        dynamic_sort = Some(content.parse()?);
                    }
                },
                "allow_sort" | "allowSort" => {
                    let allow_content;
                    bracketed!(allow_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        allow_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    allow_sort = fields.into_iter().collect();
                },
                "merge" => {
                    merge_filters = Some(content.parse()?);
                },
                "allow" => {
                    let allow_content;
                    bracketed!(allow_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        allow_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    allow_fields = fields.into_iter().collect();
                },
                "deny_ops" | "denyOps" => {
                    let deny_content;
                    bracketed!(deny_content in content);
                    let mut ops = Vec::new();
                    while !deny_content.is_empty() {
                        deny_content.parse::<Token![$]>()?;
                        let op: syn::Ident = deny_content.call(syn::Ident::parse_any)?;
                        ops.push(op);
                        if deny_content.peek(Token![,]) {
                            deny_content.parse::<Token![,]>()?;
                        }
                    }
                    deny_ops = ops;
                },
                "max_depth" | "maxDepth" => {
                    max_depth = Some(content.parse()?);
                },
                "page" => {
                    page = Some(content.parse()?);
                },
                "limit" => {
                    limit = Some(content.parse()?);
                },
                "offset" => {
                    offset = Some(content.parse()?);
                },
                "after" => {
                    after = Some(content.parse()?);
                },
                "before" => {
                    before = Some(content.parse()?);
                },
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "Unknown option '{other}'. Valid options: select, compute, aggregate, filter, merge, allow, deny_ops, max_depth, group_by, having, order, allow_sort, page, limit, offset, after, before"
                        ),
                    ));
                },
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(SqlInput {
            dialect,
            table,
            select_fields,
            computed,
            aggregates,
            filter_expr,
            group_by,
            having,
            sorts,
            dynamic_sort,
            allow_sort,
            merge_filters,
            allow_fields,
            deny_ops,
            max_depth,
            page,
            limit,
            offset,
            after,
            before,
        })
    }
}

/// Parse the aggregate block.
fn parse_aggregates(input: ParseStream) -> Result<Vec<SqlAggregate>> {
    let mut aggregates = Vec::new();

    while !input.is_empty() {
        let func_name: syn::Ident = input.parse()?;
        input.parse::<Token![:]>()?;

        let func_str = func_name.to_string();
        let (func, field, alias) = match func_str.as_str() {
            "count" => {
                if input.peek(Token![*]) {
                    input.parse::<Token![*]>()?;
                    (
                        SqlAggregateFunc::Count,
                        None,
                        Some(syn::Ident::new("count", func_name.span())),
                    )
                } else {
                    let field: syn::Ident = input.parse()?;
                    (SqlAggregateFunc::Count, Some(field), None)
                }
            },
            "count_distinct" | "countDistinct" => {
                let field: syn::Ident = input.parse()?;
                (SqlAggregateFunc::CountDistinct, Some(field), None)
            },
            "sum" => {
                let field: syn::Ident = input.parse()?;
                (SqlAggregateFunc::Sum, Some(field), None)
            },
            "avg" => {
                let field: syn::Ident = input.parse()?;
                (SqlAggregateFunc::Avg, Some(field), None)
            },
            "min" => {
                let field: syn::Ident = input.parse()?;
                (SqlAggregateFunc::Min, Some(field), None)
            },
            "max" => {
                let field: syn::Ident = input.parse()?;
                (SqlAggregateFunc::Max, Some(field), None)
            },
            other => {
                return Err(syn::Error::new(
                    func_name.span(),
                    format!(
                        "Unknown aggregate function '{other}'. Valid: count, count_distinct, sum, avg, min, max"
                    ),
                ));
            },
        };

        aggregates.push(SqlAggregate { func, field, alias });

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(aggregates)
}

/// Parse the compute block.
fn parse_compute_fields(input: ParseStream) -> Result<Vec<SqlCompute>> {
    let mut computed = Vec::new();

    while !input.is_empty() {
        let alias: syn::Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let expr = parse_compute_expr(input)?;
        computed.push(SqlCompute { alias, expr });

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(computed)
}

fn parse_compute_expr(input: ParseStream) -> Result<SqlComputeExpr> {
    parse_compute_additive(input)
}

fn parse_compute_additive(input: ParseStream) -> Result<SqlComputeExpr> {
    let mut left = parse_compute_multiplicative(input)?;

    while input.peek(Token![+]) || input.peek(Token![-]) {
        let op = if input.peek(Token![+]) {
            input.parse::<Token![+]>()?;
            SqlComputeBinOp::Add
        } else {
            input.parse::<Token![-]>()?;
            SqlComputeBinOp::Sub
        };

        let right = parse_compute_multiplicative(input)?;
        left = SqlComputeExpr::BinOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }

    Ok(left)
}

fn parse_compute_multiplicative(input: ParseStream) -> Result<SqlComputeExpr> {
    let mut left = parse_compute_primary(input)?;

    while input.peek(Token![*]) || input.peek(Token![/]) {
        let op = if input.peek(Token![*]) {
            input.parse::<Token![*]>()?;
            SqlComputeBinOp::Mul
        } else {
            input.parse::<Token![/]>()?;
            SqlComputeBinOp::Div
        };

        let right = parse_compute_primary(input)?;
        left = SqlComputeExpr::BinOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }

    Ok(left)
}

fn parse_compute_primary(input: ParseStream) -> Result<SqlComputeExpr> {
    if input.peek(token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        let inner = parse_compute_expr(&content)?;
        return Ok(SqlComputeExpr::Paren(Box::new(inner)));
    }

    if input.peek(LitStr) {
        return Ok(SqlComputeExpr::LitStr(input.parse()?));
    }

    if input.peek(LitFloat) {
        return Ok(SqlComputeExpr::LitFloat(input.parse()?));
    }

    if input.peek(LitInt) {
        return Ok(SqlComputeExpr::LitInt(input.parse()?));
    }

    if input.peek(syn::Ident) {
        let ident: syn::Ident = input.parse()?;

        if input.peek(token::Paren) {
            let func_name = ident.to_string();
            let func = match func_name.as_str() {
                "concat" => SqlComputeFunc::Concat,
                "coalesce" => SqlComputeFunc::Coalesce,
                "upper" => SqlComputeFunc::Upper,
                "lower" => SqlComputeFunc::Lower,
                "round" => SqlComputeFunc::Round,
                "abs" => SqlComputeFunc::Abs,
                "length" | "len" => SqlComputeFunc::Length,
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "Unknown compute function '{other}'. Valid: concat, coalesce, upper, lower, round, abs, length"
                        ),
                    ));
                },
            };

            let args_content;
            syn::parenthesized!(args_content in input);
            let args: Punctuated<SqlComputeExpr, Token![,]> =
                args_content.parse_terminated(parse_compute_expr, Token![,])?;

            return Ok(SqlComputeExpr::Func {
                name: func,
                args: args.into_iter().collect(),
            });
        }

        return Ok(SqlComputeExpr::Column(ident));
    }

    Err(syn::Error::new(
        input.span(),
        "Expected a compute expression: column, literal, function call, or (expression)",
    ))
}

fn parse_filter_block(input: ParseStream) -> Result<SqlFilterExpr> {
    let mut simple_filters = Vec::new();

    while !input.is_empty() {
        if input.peek(Token![$]) {
            input.parse::<Token![$]>()?;
            let op_name: syn::Ident = input.call(syn::Ident::parse_any)?;
            input.parse::<Token![:]>()?;

            let logical_op = match op_name.to_string().as_str() {
                "and" => SqlLogicalOp::And,
                "or" => SqlLogicalOp::Or,
                "not" => SqlLogicalOp::Not,
                other => {
                    return Err(syn::Error::new(
                        op_name.span(),
                        format!("Unknown logical operator '${other}'. Valid: $and, $or, $not"),
                    ));
                },
            };

            let filters = parse_filter_array(input)?;

            if !simple_filters.is_empty() {
                let mut all_filters: Vec<SqlFilterExpr> = simple_filters
                    .into_iter()
                    .map(SqlFilterExpr::Simple)
                    .collect();
                all_filters.push(SqlFilterExpr::Compound {
                    op: logical_op,
                    filters,
                });
                return Ok(SqlFilterExpr::Compound {
                    op: SqlLogicalOp::And,
                    filters: all_filters,
                });
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }

            if !input.is_empty() {
                let remaining = parse_filter_block(input)?;
                return Ok(SqlFilterExpr::Compound {
                    op: SqlLogicalOp::And,
                    filters: vec![
                        SqlFilterExpr::Compound {
                            op: logical_op,
                            filters,
                        },
                        remaining,
                    ],
                });
            }

            return Ok(SqlFilterExpr::Compound {
                op: logical_op,
                filters,
            });
        }

        let filter = parse_sql_filter(input)?;
        simple_filters.push(filter);

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    match simple_filters.len() {
        0 => Err(syn::Error::new(input.span(), "Empty filter block")),
        1 => Ok(SqlFilterExpr::Simple(simple_filters.remove(0))),
        _ => Ok(SqlFilterExpr::Compound {
            op: SqlLogicalOp::And,
            filters: simple_filters
                .into_iter()
                .map(SqlFilterExpr::Simple)
                .collect(),
        }),
    }
}

fn parse_filter_array(input: ParseStream) -> Result<Vec<SqlFilterExpr>> {
    let content;
    bracketed!(content in input);

    let mut filters = Vec::new();
    while !content.is_empty() {
        let filter_content;
        braced!(filter_content in content);
        let filter_expr = parse_filter_block(&filter_content)?;
        filters.push(filter_expr);

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(filters)
}

fn parse_sql_filter(input: ParseStream) -> Result<SqlFilter> {
    let field: syn::Ident = input.parse()?;
    input.parse::<Token![:]>()?;

    if input.peek(token::Brace) {
        let op_content;
        braced!(op_content in input);

        op_content.parse::<Token![$]>()?;
        let op_name: syn::Ident = op_content.call(syn::Ident::parse_any)?;
        op_content.parse::<Token![:]>()?;

        let op = match op_name.to_string().as_str() {
            "eq" => SqlOperator::Eq,
            "ne" => SqlOperator::Ne,
            "gt" => SqlOperator::Gt,
            "gte" => SqlOperator::Gte,
            "lt" => SqlOperator::Lt,
            "lte" => SqlOperator::Lte,
            "in" => SqlOperator::In,
            "nin" => SqlOperator::NotIn,
            "like" => SqlOperator::Like,
            "ilike" => SqlOperator::ILike,
            "regex" => SqlOperator::Regex,
            "startsWith" | "starts_with" => SqlOperator::StartsWith,
            "endsWith" | "ends_with" => SqlOperator::EndsWith,
            "contains" => SqlOperator::Contains,
            "between" => SqlOperator::Between,
            other => {
                return Err(syn::Error::new(
                    op_name.span(),
                    format!(
                        "Unknown operator '${other}'. Valid operators: $eq, $ne, $gt, $gte, $lt, $lte, $in, $nin, $like, $ilike, $regex, $startsWith, $endsWith, $contains, $between"
                    ),
                ));
            },
        };

        let value = parse_sql_value(&op_content)?;
        Ok(SqlFilter { field, op, value })
    } else {
        let value = parse_sql_value(input)?;
        Ok(SqlFilter {
            field,
            op: SqlOperator::Eq,
            value,
        })
    }
}

fn parse_sql_value(input: ParseStream) -> Result<SqlValue> {
    let lookahead = input.lookahead1();

    if lookahead.peek(token::Bracket) {
        let content;
        bracketed!(content in input);
        let elements: Punctuated<SqlValue, Token![,]> =
            content.parse_terminated(|inner| parse_sql_value(inner), Token![,])?;
        Ok(SqlValue::Array(elements.into_iter().collect()))
    } else if lookahead.peek(LitStr) {
        Ok(SqlValue::String(input.parse()?))
    } else if lookahead.peek(LitInt) {
        Ok(SqlValue::Int(input.parse()?))
    } else if lookahead.peek(LitFloat) {
        Ok(SqlValue::Float(input.parse()?))
    } else if lookahead.peek(LitBool) {
        let lit: LitBool = input.parse()?;
        Ok(SqlValue::Bool(lit.value))
    } else if input.peek(syn::Ident) && input.peek2(token::Paren) {
        let fork = input.fork();
        let ident: syn::Ident = fork.parse()?;
        match ident.to_string().as_str() {
            "int" => {
                input.parse::<syn::Ident>()?;
                let content;
                syn::parenthesized!(content in input);
                Ok(SqlValue::IntHint(content.parse()?))
            },
            "str" => {
                input.parse::<syn::Ident>()?;
                let content;
                syn::parenthesized!(content in input);
                Ok(SqlValue::StrHint(content.parse()?))
            },
            "float" => {
                input.parse::<syn::Ident>()?;
                let content;
                syn::parenthesized!(content in input);
                Ok(SqlValue::FloatHint(content.parse()?))
            },
            "bool" => {
                input.parse::<syn::Ident>()?;
                let content;
                syn::parenthesized!(content in input);
                Ok(SqlValue::BoolHint(content.parse()?))
            },
            _ => Ok(SqlValue::Expr(input.parse()?)),
        }
    } else if input.peek(syn::Ident) {
        let fork = input.fork();
        let ident: syn::Ident = fork.parse()?;
        match ident.to_string().as_str() {
            "null" => {
                input.parse::<syn::Ident>()?;
                Ok(SqlValue::Null)
            },
            "true" => {
                input.parse::<syn::Ident>()?;
                Ok(SqlValue::Bool(true))
            },
            "false" => {
                input.parse::<syn::Ident>()?;
                Ok(SqlValue::Bool(false))
            },
            _ => Ok(SqlValue::Expr(input.parse()?)),
        }
    } else {
        Err(syn::Error::new(
            input.span(),
            "Expected a value: string, number, boolean, null, array, or type hint (int(), str(), etc.)",
        ))
    }
}

impl Parse for SqlSort {
    fn parse(input: ParseStream) -> Result<Self> {
        let desc = if input.peek(Token![-]) {
            input.parse::<Token![-]>()?;
            true
        } else {
            false
        };
        let field: syn::Ident = input.parse()?;
        Ok(SqlSort { field, desc })
    }
}

fn sql_value_to_tokens(value: &SqlValue) -> TokenStream2 {
    match value {
        SqlValue::Null => quote! { ::mik_sql::Value::Null },
        SqlValue::Bool(b) => quote! { ::mik_sql::Value::Bool(#b) },
        SqlValue::Int(i) => quote! { ::mik_sql::Value::Int(#i as i64) },
        SqlValue::Float(f) => quote! { ::mik_sql::Value::Float(#f as f64) },
        SqlValue::String(s) => quote! { ::mik_sql::Value::String(#s.to_string()) },
        SqlValue::Array(arr) => {
            let elements: Vec<_> = arr.iter().map(sql_value_to_tokens).collect();
            quote! { ::mik_sql::Value::Array(vec![#(#elements),*]) }
        },
        SqlValue::IntHint(e) => quote! { ::mik_sql::Value::Int(#e as i64) },
        SqlValue::StrHint(e) | SqlValue::Expr(e) => {
            quote! { ::mik_sql::Value::String((#e).to_string()) }
        },
        SqlValue::FloatHint(e) => quote! { ::mik_sql::Value::Float(#e as f64) },
        SqlValue::BoolHint(e) => quote! { ::mik_sql::Value::Bool(#e) },
    }
}

fn sql_operator_to_tokens(op: &SqlOperator) -> TokenStream2 {
    match op {
        SqlOperator::Eq => quote! { ::mik_sql::Operator::Eq },
        SqlOperator::Ne => quote! { ::mik_sql::Operator::Ne },
        SqlOperator::Gt => quote! { ::mik_sql::Operator::Gt },
        SqlOperator::Gte => quote! { ::mik_sql::Operator::Gte },
        SqlOperator::Lt => quote! { ::mik_sql::Operator::Lt },
        SqlOperator::Lte => quote! { ::mik_sql::Operator::Lte },
        SqlOperator::In => quote! { ::mik_sql::Operator::In },
        SqlOperator::NotIn => quote! { ::mik_sql::Operator::NotIn },
        SqlOperator::Like => quote! { ::mik_sql::Operator::Like },
        SqlOperator::ILike => quote! { ::mik_sql::Operator::ILike },
        SqlOperator::Regex => quote! { ::mik_sql::Operator::Regex },
        SqlOperator::StartsWith => quote! { ::mik_sql::Operator::StartsWith },
        SqlOperator::EndsWith => quote! { ::mik_sql::Operator::EndsWith },
        SqlOperator::Contains => quote! { ::mik_sql::Operator::Contains },
        SqlOperator::Between => quote! { ::mik_sql::Operator::Between },
    }
}

/// Build a SELECT query using the query builder (CRUD: Read).
#[proc_macro]
#[allow(clippy::too_many_lines)] // Query building has many options to handle
pub fn sql_read(input: TokenStream) -> TokenStream {
    let SqlInput {
        dialect,
        table,
        select_fields,
        computed,
        aggregates,
        filter_expr,
        group_by,
        having,
        sorts,
        dynamic_sort,
        allow_sort,
        merge_filters,
        allow_fields,
        deny_ops,
        max_depth,
        page,
        limit,
        offset,
        after,
        before,
    } = parse_macro_input!(input as SqlInput);

    let (sorts, dynamic_sort) = if let Some(ref expr) = dynamic_sort {
        if allow_sort.is_empty() {
            if let syn::Expr::Path(syn::ExprPath { path, .. }) = expr {
                if path.segments.len() == 1 && path.segments[0].arguments.is_empty() {
                    let field_name = path.segments[0].ident.clone();
                    let mut new_sorts = sorts;
                    new_sorts.push(SqlSort {
                        field: field_name,
                        desc: false,
                    });
                    (new_sorts, None)
                } else {
                    (sorts, dynamic_sort)
                }
            } else {
                (sorts, dynamic_sort)
            }
        } else {
            (sorts, dynamic_sort)
        }
    } else {
        (sorts, dynamic_sort)
    };

    let table_str = table.to_string();

    let fields_chain = if select_fields.is_empty() {
        quote! {}
    } else {
        let field_strs: Vec<String> = select_fields
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .fields(&[#(#field_strs),*]) }
    };

    let computed_chain: Vec<TokenStream2> = computed
        .iter()
        .map(|c| {
            let alias = c.alias.to_string();
            let expr_sql = compute_expr_to_sql(&c.expr);
            quote! { .computed(#alias, #expr_sql) }
        })
        .collect();

    let aggregate_chain: Vec<TokenStream2> = aggregates
        .iter()
        .map(|agg| {
            let agg_tokens = sql_aggregate_to_tokens(agg);
            quote! { .aggregate(#agg_tokens) }
        })
        .collect();

    let filter_chain = if let Some(expr) = filter_expr {
        let expr_tokens = sql_filter_expr_to_tokens(&expr);
        quote! { .filter_expr(#expr_tokens) }
    } else {
        quote! {}
    };

    let group_by_chain = if group_by.is_empty() {
        quote! {}
    } else {
        let field_strs: Vec<String> = group_by
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .group_by(&[#(#field_strs),*]) }
    };

    let having_chain = if let Some(expr) = having {
        let expr_tokens = sql_filter_expr_to_tokens(&expr);
        quote! { .having(#expr_tokens) }
    } else {
        quote! {}
    };

    let sort_chain: Vec<TokenStream2> = sorts
        .iter()
        .map(|s| {
            let field_str = s.field.to_string();
            let dir = if s.desc {
                quote! { ::mik_sql::SortDir::Desc }
            } else {
                quote! { ::mik_sql::SortDir::Asc }
            };
            quote! { .sort(#field_str, #dir) }
        })
        .collect();

    let dynamic_sort_setup = if let Some(ref sort_expr) = dynamic_sort {
        let allow_strs: Vec<String> = allow_sort
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        if allow_strs.is_empty() {
            quote! {
                let __dynamic_sorts = ::mik_sql::SortField::parse_sort_string(
                    &#sort_expr,
                    &[]
                ).map_err(|e| e)?;
            }
        } else {
            quote! {
                let __dynamic_sorts = ::mik_sql::SortField::parse_sort_string(
                    &#sort_expr,
                    &[#(#allow_strs),*]
                ).map_err(|e| e)?;
            }
        }
    } else {
        quote! {}
    };

    let dynamic_sort_chain = if dynamic_sort.is_some() {
        quote! { .sorts(&__dynamic_sorts) }
    } else {
        quote! {}
    };

    let (merge_setup, merge_chain) = if let Some(ref merge_expr) = merge_filters {
        let allow_strs: Vec<String> = allow_fields
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let deny_op_tokens: Vec<TokenStream2> = deny_ops
            .iter()
            .map(|op| {
                let op_str = op.to_string();
                match op_str.as_str() {
                    "ne" => quote! { ::mik_sql::Operator::Ne },
                    "gt" => quote! { ::mik_sql::Operator::Gt },
                    "gte" => quote! { ::mik_sql::Operator::Gte },
                    "lt" => quote! { ::mik_sql::Operator::Lt },
                    "lte" => quote! { ::mik_sql::Operator::Lte },
                    "in" => quote! { ::mik_sql::Operator::In },
                    "nin" | "notIn" => quote! { ::mik_sql::Operator::NotIn },
                    "like" => quote! { ::mik_sql::Operator::Like },
                    "ilike" => quote! { ::mik_sql::Operator::ILike },
                    "regex" => quote! { ::mik_sql::Operator::Regex },
                    "startsWith" | "starts_with" => quote! { ::mik_sql::Operator::StartsWith },
                    "endsWith" | "ends_with" => quote! { ::mik_sql::Operator::EndsWith },
                    "contains" => quote! { ::mik_sql::Operator::Contains },
                    "between" => quote! { ::mik_sql::Operator::Between },
                    // "eq" and unknown operators default to Eq
                    _ => quote! { ::mik_sql::Operator::Eq },
                }
            })
            .collect();

        let max_depth_val = max_depth
            .map(|d| quote! { #d as usize })
            .unwrap_or(quote! { 5 });

        let setup = quote! {
            let __validator = ::mik_sql::FilterValidator::new()
                .allow_fields(&[#(#allow_strs),*])
                .deny_operators(&[#(#deny_op_tokens),*])
                .max_depth(#max_depth_val);

            for __user_filter in &#merge_expr {
                __validator.validate(__user_filter).map_err(|e| e.to_string())?;
            }
        };

        let chain = quote! {
            for __f in &#merge_expr {
                __builder = __builder.filter(__f.field.clone(), __f.op, __f.value.clone());
            }
        };

        (setup, chain)
    } else {
        (quote! {}, quote! {})
    };

    let needs_result = dynamic_sort.is_some() || merge_filters.is_some();

    let pagination_chain = match (page, limit, offset) {
        (Some(p), Some(l), None) => quote! { .page(#p as u32, #l as u32) },
        (None, Some(l), Some(o)) => quote! { .limit_offset(#l as u32, #o as u32) },
        (None, Some(l), None) => quote! { .limit_offset(#l as u32, 0) },
        _ => quote! {},
    };

    let after_chain = if let Some(ref expr) = after {
        quote! { .after_cursor(#expr) }
    } else {
        quote! {}
    };

    let before_chain = if let Some(ref expr) = before {
        quote! { .before_cursor(#expr) }
    } else {
        quote! {}
    };

    let builder_constructor = dialect.builder_tokens(&table_str);

    let tokens = if needs_result {
        quote! {
            (|| -> ::std::result::Result<(String, Vec<::mik_sql::Value>), String> {
                #dynamic_sort_setup
                #merge_setup

                let mut __builder = #builder_constructor
                    #fields_chain
                    #(#computed_chain)*
                    #(#aggregate_chain)*
                    #filter_chain;

                #merge_chain

                let __sql_result = __builder
                    #group_by_chain
                    #having_chain
                    #(#sort_chain)*
                    #dynamic_sort_chain
                    #after_chain
                    #before_chain
                    #pagination_chain
                    .build();

                Ok((__sql_result.sql, __sql_result.params))
            })()
        }
    } else {
        quote! {
            {
                let __sql_result = #builder_constructor
                    #fields_chain
                    #(#computed_chain)*
                    #(#aggregate_chain)*
                    #filter_chain
                    #group_by_chain
                    #having_chain
                    #(#sort_chain)*
                    #after_chain
                    #before_chain
                    #pagination_chain
                    .build();
                (__sql_result.sql, __sql_result.params)
            }
        }
    };

    TokenStream::from(tokens)
}

fn sql_aggregate_to_tokens(agg: &SqlAggregate) -> TokenStream2 {
    let field_str = agg.field.as_ref().map(std::string::ToString::to_string);
    let alias_str = agg.alias.as_ref().map(std::string::ToString::to_string);

    let base = match (&agg.func, &field_str) {
        (SqlAggregateFunc::Count, Some(f)) => quote! { ::mik_sql::Aggregate::count_field(#f) },
        (SqlAggregateFunc::CountDistinct, Some(f)) => {
            quote! { ::mik_sql::Aggregate::count_distinct(#f) }
        },
        (SqlAggregateFunc::Sum, Some(f)) => quote! { ::mik_sql::Aggregate::sum(#f) },
        (SqlAggregateFunc::Avg, Some(f)) => quote! { ::mik_sql::Aggregate::avg(#f) },
        (SqlAggregateFunc::Min, Some(f)) => quote! { ::mik_sql::Aggregate::min(#f) },
        (SqlAggregateFunc::Max, Some(f)) => quote! { ::mik_sql::Aggregate::max(#f) },
        // Count without field, or missing required field - default to count()
        _ => quote! { ::mik_sql::Aggregate::count() },
    };

    if let Some(alias) = alias_str {
        quote! { #base.as_alias(#alias) }
    } else {
        base
    }
}

fn sql_filter_expr_to_tokens(expr: &SqlFilterExpr) -> TokenStream2 {
    match expr {
        SqlFilterExpr::Simple(filter) => {
            let field_str = filter.field.to_string();
            let op = sql_operator_to_tokens(&filter.op);
            let value = sql_value_to_tokens(&filter.value);
            quote! { ::mik_sql::simple(#field_str, #op, #value) }
        },
        SqlFilterExpr::Compound { op, filters } => {
            let filter_tokens: Vec<TokenStream2> =
                filters.iter().map(sql_filter_expr_to_tokens).collect();

            match op {
                SqlLogicalOp::And => quote! { ::mik_sql::and(vec![#(#filter_tokens),*]) },
                SqlLogicalOp::Or => quote! { ::mik_sql::or(vec![#(#filter_tokens),*]) },
                SqlLogicalOp::Not => {
                    let inner = filter_tokens.into_iter().next().unwrap_or_default();
                    quote! { ::mik_sql::not(#inner) }
                },
            }
        },
    }
}

fn compute_expr_to_sql(expr: &SqlComputeExpr) -> String {
    match expr {
        SqlComputeExpr::Column(ident) => ident.to_string(),
        SqlComputeExpr::LitStr(lit) => {
            let s = lit.value();
            format!("'{}'", s.replace('\'', "''"))
        },
        SqlComputeExpr::LitInt(lit) => lit.to_string(),
        SqlComputeExpr::LitFloat(lit) => lit.to_string(),
        SqlComputeExpr::BinOp { left, op, right } => {
            let left_sql = compute_expr_to_sql(left);
            let right_sql = compute_expr_to_sql(right);
            let op_str = match op {
                SqlComputeBinOp::Add => "+",
                SqlComputeBinOp::Sub => "-",
                SqlComputeBinOp::Mul => "*",
                SqlComputeBinOp::Div => "/",
            };
            format!("{left_sql} {op_str} {right_sql}")
        },
        SqlComputeExpr::Func { name, args } => {
            let args_sql: Vec<String> = args.iter().map(compute_expr_to_sql).collect();
            match name {
                SqlComputeFunc::Concat => args_sql.join(" || "),
                SqlComputeFunc::Coalesce => format!("COALESCE({})", args_sql.join(", ")),
                SqlComputeFunc::Upper => format!("UPPER({})", args_sql.join(", ")),
                SqlComputeFunc::Lower => format!("LOWER({})", args_sql.join(", ")),
                SqlComputeFunc::Round => format!("ROUND({})", args_sql.join(", ")),
                SqlComputeFunc::Abs => format!("ABS({})", args_sql.join(", ")),
                SqlComputeFunc::Length => format!("LENGTH({})", args_sql.join(", ")),
            }
        },
        SqlComputeExpr::Paren(inner) => format!("({})", compute_expr_to_sql(inner)),
    }
}

/// Collect field values from a list for batched loading.
#[proc_macro]
pub fn ids(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as IdsInput);

    let list = &input.list;
    let field = &input.field;

    let tokens = quote! {
        #list.iter().map(|__item| __item.#field.clone()).collect::<Vec<_>>()
    };

    TokenStream::from(tokens)
}

struct IdsInput {
    list: Expr,
    field: syn::Ident,
}

impl Parse for IdsInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let list: Expr = input.parse()?;

        let field = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            input.parse()?
        } else {
            syn::Ident::new("id", proc_macro2::Span::call_site())
        };

        Ok(IdsInput { list, field })
    }
}

/// Build an INSERT query using object-like syntax.
#[proc_macro]
pub fn sql_create(input: TokenStream) -> TokenStream {
    let InsertInput {
        dialect,
        table,
        columns,
        returning,
    } = parse_macro_input!(input as InsertInput);

    let table_str = table.to_string();
    let builder_constructor = dialect.insert_tokens(&table_str);

    let col_strs: Vec<String> = columns.iter().map(|(c, _)| c.to_string()).collect();

    let value_tokens: Vec<TokenStream2> = columns
        .iter()
        .map(|(_, v)| sql_value_to_tokens(v))
        .collect();

    let returning_chain = if returning.is_empty() {
        quote! {}
    } else {
        let ret_strs: Vec<String> = returning
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .returning(&[#(#ret_strs),*]) }
    };

    let tokens = quote! {
        {
            let __result = #builder_constructor
                .columns(&[#(#col_strs),*])
                .values(vec![#(#value_tokens),*])
                #returning_chain
                .build();
            (__result.sql, __result.params)
        }
    };

    TokenStream::from(tokens)
}

struct InsertInput {
    dialect: SqlDialect,
    table: syn::Ident,
    columns: Vec<(syn::Ident, SqlValue)>,
    returning: Vec<syn::Ident>,
}

impl Parse for InsertInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let dialect = parse_optional_dialect(input)?;
        let table: syn::Ident = input.parse()?;

        let content;
        braced!(content in input);

        let mut columns = Vec::new();
        let mut returning = Vec::new();

        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            if key.to_string().as_str() == "returning" {
                let ret_content;
                bracketed!(ret_content in content);
                let fields: Punctuated<syn::Ident, Token![,]> =
                    ret_content.parse_terminated(syn::Ident::parse, Token![,])?;
                returning = fields.into_iter().collect();
            } else {
                let value = parse_sql_value(&content)?;
                columns.push((key, value));
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(InsertInput {
            dialect,
            table,
            columns,
            returning,
        })
    }
}

/// Build an UPDATE query using object-like syntax.
#[proc_macro]
pub fn sql_update(input: TokenStream) -> TokenStream {
    let UpdateInput {
        dialect,
        table,
        sets,
        where_expr,
        returning,
    } = parse_macro_input!(input as UpdateInput);

    let table_str = table.to_string();
    let builder_constructor = dialect.update_tokens(&table_str);

    let set_chain: Vec<TokenStream2> = sets
        .iter()
        .map(|(col, val)| {
            let col_str = col.to_string();
            let val_tokens = sql_value_to_tokens(val);
            quote! { .set(#col_str, #val_tokens) }
        })
        .collect();

    let filter_chain = if let Some(expr) = where_expr {
        let expr_tokens = sql_filter_expr_to_tokens(&expr);
        quote! { .filter_expr(#expr_tokens) }
    } else {
        quote! {}
    };

    let returning_chain = if returning.is_empty() {
        quote! {}
    } else {
        let ret_strs: Vec<String> = returning
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .returning(&[#(#ret_strs),*]) }
    };

    let tokens = quote! {
        {
            let __result = #builder_constructor
                #(#set_chain)*
                #filter_chain
                #returning_chain
                .build();
            (__result.sql, __result.params)
        }
    };

    TokenStream::from(tokens)
}

struct UpdateInput {
    dialect: SqlDialect,
    table: syn::Ident,
    sets: Vec<(syn::Ident, SqlValue)>,
    where_expr: Option<SqlFilterExpr>,
    returning: Vec<syn::Ident>,
}

impl Parse for UpdateInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let dialect = parse_optional_dialect(input)?;
        let table: syn::Ident = input.parse()?;

        let content;
        braced!(content in input);

        let mut sets = Vec::new();
        let mut where_expr = None;
        let mut returning = Vec::new();

        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "set" => {
                    let set_content;
                    braced!(set_content in content);
                    sets = parse_column_values(&set_content)?;
                },
                "where" | "filter" => {
                    let where_content;
                    braced!(where_content in content);
                    where_expr = Some(parse_filter_block(&where_content)?);
                },
                "returning" => {
                    let ret_content;
                    bracketed!(ret_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        ret_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    returning = fields.into_iter().collect();
                },
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("Unknown option '{key}'. Expected 'set', 'where', or 'returning'"),
                    ));
                },
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(UpdateInput {
            dialect,
            table,
            sets,
            where_expr,
            returning,
        })
    }
}

/// Build a DELETE query using object-like syntax.
#[proc_macro]
pub fn sql_delete(input: TokenStream) -> TokenStream {
    let DeleteInput {
        dialect,
        table,
        where_expr,
        returning,
    } = parse_macro_input!(input as DeleteInput);

    let table_str = table.to_string();
    let builder_constructor = dialect.delete_tokens(&table_str);

    let filter_chain = if let Some(expr) = where_expr {
        let expr_tokens = sql_filter_expr_to_tokens(&expr);
        quote! { .filter_expr(#expr_tokens) }
    } else {
        quote! {}
    };

    let returning_chain = if returning.is_empty() {
        quote! {}
    } else {
        let ret_strs: Vec<String> = returning
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .returning(&[#(#ret_strs),*]) }
    };

    let tokens = quote! {
        {
            let __result = #builder_constructor
                #filter_chain
                #returning_chain
                .build();
            (__result.sql, __result.params)
        }
    };

    TokenStream::from(tokens)
}

struct DeleteInput {
    dialect: SqlDialect,
    table: syn::Ident,
    where_expr: Option<SqlFilterExpr>,
    returning: Vec<syn::Ident>,
}

impl Parse for DeleteInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let dialect = parse_optional_dialect(input)?;
        let table: syn::Ident = input.parse()?;

        let content;
        braced!(content in input);

        let mut where_expr = None;
        let mut returning = Vec::new();

        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "where" | "filter" => {
                    let where_content;
                    braced!(where_content in content);
                    where_expr = Some(parse_filter_block(&where_content)?);
                },
                "returning" => {
                    let ret_content;
                    bracketed!(ret_content in content);
                    let fields: Punctuated<syn::Ident, Token![,]> =
                        ret_content.parse_terminated(syn::Ident::parse, Token![,])?;
                    returning = fields.into_iter().collect();
                },
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("Unknown option '{key}'. Expected 'where' or 'returning'"),
                    ));
                },
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(DeleteInput {
            dialect,
            table,
            where_expr,
            returning,
        })
    }
}

fn parse_column_values(input: ParseStream) -> Result<Vec<(syn::Ident, SqlValue)>> {
    let mut result = Vec::new();

    while !input.is_empty() {
        let key: syn::Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = parse_sql_value(input)?;
        result.push((key, value));

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(result)
}
