//! `SqlInput` parsing for `sql_read!` macro.

use syn::{
    Expr, Result, Token, braced, bracketed,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

use crate::errors::did_you_mean;
use crate::parse::{
    parse_aggregates, parse_compute_fields, parse_filter_block, parse_ident_list,
    parse_optional_dialect,
};
use crate::types::{SqlAggregate, SqlCompute, SqlDialect, SqlFilterExpr, SqlSort};

/// Valid options for `sql_read!` macro.
const VALID_READ_OPTIONS: &[&str] = &[
    "select",
    "compute",
    "aggregate",
    "filter",
    "merge",
    "allow",
    "deny_ops",
    "max_depth",
    "group_by",
    "having",
    "order",
    "allow_sort",
    "page",
    "limit",
    "offset",
    "after",
    "before",
];

/// Input for the [`sql_read!`] macro.
pub struct SqlInput {
    pub dialect: SqlDialect,
    pub table: syn::Ident,
    pub select_fields: Vec<syn::Ident>,
    pub computed: Vec<SqlCompute>,
    pub aggregates: Vec<SqlAggregate>,
    pub filter_expr: Option<SqlFilterExpr>,
    pub group_by: Vec<syn::Ident>,
    pub having: Option<SqlFilterExpr>,
    pub sorts: Vec<SqlSort>,
    pub dynamic_sort: Option<Expr>,
    pub allow_sort: Vec<syn::Ident>,
    pub merge_filters: Option<Expr>,
    pub allow_fields: Vec<syn::Ident>,
    pub deny_ops: Vec<syn::Ident>,
    pub max_depth: Option<Expr>,
    pub page: Option<Expr>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
    pub after: Option<Expr>,
    pub before: Option<Expr>,
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
                    select_fields = parse_ident_list(&content)?;
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
                    group_by = parse_ident_list(&content)?;
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
                    allow_sort = parse_ident_list(&content)?;
                },
                "merge" => {
                    merge_filters = Some(content.parse()?);
                },
                "allow" => {
                    allow_fields = parse_ident_list(&content)?;
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
                    let suggestion = did_you_mean(other, VALID_READ_OPTIONS);
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "Unknown option '{other}'.{suggestion}\n\n\
                             Valid options: select, compute, aggregate, filter, merge, allow, \
                             deny_ops, max_depth, group_by, having, order, allow_sort, page, \
                             limit, offset, after, before"
                        ),
                    ));
                },
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
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
