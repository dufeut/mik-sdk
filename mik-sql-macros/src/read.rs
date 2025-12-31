//! `sql_read!` macro implementation for SELECT queries.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, Result, Token, braced, bracketed,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token,
};

use crate::common::{
    SqlAggregate, SqlCompute, SqlDialect, SqlFilterExpr, SqlSort, compute_expr_to_sql,
    parse_aggregates, parse_compute_fields, parse_filter_block, parse_optional_dialect,
    sql_aggregate_to_tokens, sql_filter_expr_to_tokens,
};

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

/// Build a SELECT query using the query builder (CRUD: Read).
#[allow(clippy::too_many_lines)] // Query building has many options to handle
#[allow(clippy::cognitive_complexity)] // Complex macro with many branches
pub fn sql_read_impl(input: TokenStream) -> TokenStream {
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

    let filter_chain = filter_expr.map_or_else(
        || quote! {},
        |expr| {
            let expr_tokens = sql_filter_expr_to_tokens(&expr);
            quote! { .filter_expr(#expr_tokens) }
        },
    );

    let group_by_chain = if group_by.is_empty() {
        quote! {}
    } else {
        let field_strs: Vec<String> = group_by
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        quote! { .group_by(&[#(#field_strs),*]) }
    };

    let having_chain = having.map_or_else(
        || quote! {},
        |expr| {
            let expr_tokens = sql_filter_expr_to_tokens(&expr);
            quote! { .having(#expr_tokens) }
        },
    );

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

    let dynamic_sort_setup = dynamic_sort.as_ref().map_or_else(
        || quote! {},
        |sort_expr| {
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
        },
    );

    let dynamic_sort_chain = dynamic_sort
        .as_ref()
        .map_or_else(|| quote! {}, |_| quote! { .sorts(&__dynamic_sorts) });

    let (merge_setup, merge_chain) = merge_filters.as_ref().map_or_else(
        || (quote! {}, quote! {}),
        |merge_expr| {
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

            let max_depth_val = max_depth.map_or_else(|| quote! { 5 }, |d| quote! { #d as usize });

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
        },
    );

    let needs_result = dynamic_sort.is_some() || merge_filters.is_some();

    let pagination_chain = match (page, limit, offset) {
        (Some(p), Some(l), None) => quote! { .page(#p as u32, #l as u32) },
        (None, Some(l), Some(o)) => quote! { .limit_offset(#l as u32, #o as u32) },
        (None, Some(l), None) => quote! { .limit_offset(#l as u32, 0) },
        _ => quote! {},
    };

    let after_chain = after
        .as_ref()
        .map_or_else(|| quote! {}, |expr| quote! { .after_cursor(#expr) });

    let before_chain = before
        .as_ref()
        .map_or_else(|| quote! {}, |expr| quote! { .before_cursor(#expr) });

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
