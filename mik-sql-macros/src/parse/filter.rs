//! Filter parsing for SQL CRUD macros.

use syn::{
    LitBool, LitFloat, LitInt, LitStr, Result, Token, braced, bracketed, ext::IdentExt,
    parse::ParseStream, punctuated::Punctuated, token,
};

use crate::errors::{did_you_mean, invalid_operator};
use crate::types::{SqlFilter, SqlFilterExpr, SqlLogicalOp, SqlOperator, SqlValue};

/// Valid logical operators for filter blocks.
const VALID_LOGICAL_OPS: &[&str] = &["and", "or", "not"];

/// Parse a filter block containing simple and/or compound filters.
pub fn parse_filter_block(input: ParseStream) -> Result<SqlFilterExpr> {
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
                    let suggestion = did_you_mean(other, VALID_LOGICAL_OPS);
                    return Err(syn::Error::new(
                        op_name.span(),
                        format!(
                            "Unknown logical operator '${other}'.{suggestion}\n\n\
                             Valid operators: $and, $or, $not"
                        ),
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
                return Err(invalid_operator(op_name.span(), other));
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

/// Parse a SQL value (literal, array, type hint, or expression).
pub fn parse_sql_value(input: ParseStream) -> Result<SqlValue> {
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
