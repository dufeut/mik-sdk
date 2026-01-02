//! Compute expression parsing for SQL CRUD macros.

use syn::{
    LitFloat, LitInt, LitStr, Result, Token, parse::ParseStream, punctuated::Punctuated, token,
};

use crate::errors::did_you_mean;
use crate::types::{SqlCompute, SqlComputeBinOp, SqlComputeExpr, SqlComputeFunc};

/// Valid compute functions.
const VALID_COMPUTE_FUNCS: &[&str] = &[
    "concat", "coalesce", "upper", "lower", "round", "abs", "length", "len",
];

/// Parse the compute block.
pub fn parse_compute_fields(input: ParseStream) -> Result<Vec<SqlCompute>> {
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

/// Parse a compute expression.
pub fn parse_compute_expr(input: ParseStream) -> Result<SqlComputeExpr> {
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
                    let suggestion = did_you_mean(other, VALID_COMPUTE_FUNCS);
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "Unknown compute function '{other}'.{suggestion}\n\n\
                             Valid functions: concat, coalesce, upper, lower, round, abs, length"
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
