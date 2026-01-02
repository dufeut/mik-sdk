//! Aggregate parsing for SQL CRUD macros.

use syn::{Result, Token, parse::ParseStream};

use crate::errors::did_you_mean;
use crate::types::{SqlAggregate, SqlAggregateFunc};

/// Valid aggregate functions.
const VALID_AGGREGATE_FUNCS: &[&str] = &[
    "count",
    "count_distinct",
    "countDistinct",
    "sum",
    "avg",
    "min",
    "max",
];

/// Parse the aggregate block.
pub fn parse_aggregates(input: ParseStream) -> Result<Vec<SqlAggregate>> {
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
                let suggestion = did_you_mean(other, VALID_AGGREGATE_FUNCS);
                return Err(syn::Error::new(
                    func_name.span(),
                    format!(
                        "Unknown aggregate function '{other}'.{suggestion}\n\n\
                         Valid functions: count, count_distinct, sum, avg, min, max"
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
