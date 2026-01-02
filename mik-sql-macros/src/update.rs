//! `sql_update!` macro implementation for UPDATE queries.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Result, Token, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::codegen::{generate_returning_chain, sql_filter_expr_to_tokens, sql_value_to_tokens};
use crate::errors::did_you_mean;
use crate::parse::{
    parse_column_values, parse_filter_block, parse_ident_list, parse_optional_dialect,
};
use crate::types::{SqlDialect, SqlFilterExpr, SqlValue};

/// Valid options for `sql_update!` macro.
const VALID_UPDATE_OPTIONS: &[&str] = &["set", "where", "filter", "returning"];

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
                    returning = parse_ident_list(&content)?;
                },
                other => {
                    let suggestion = did_you_mean(other, VALID_UPDATE_OPTIONS);
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "Unknown option '{other}'.{suggestion}\n\n\
                             Valid options: set, where, filter, returning"
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
            sets,
            where_expr,
            returning,
        })
    }
}

/// Build an UPDATE query using object-like syntax.
pub fn sql_update_impl(input: TokenStream) -> TokenStream {
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

    let filter_chain = where_expr.map_or_else(
        || quote! {},
        |expr| {
            let expr_tokens = sql_filter_expr_to_tokens(&expr);
            quote! { .filter_expr(#expr_tokens) }
        },
    );

    let returning_chain = generate_returning_chain(&returning);

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
