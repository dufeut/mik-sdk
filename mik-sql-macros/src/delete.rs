//! `sql_delete!` macro implementation for DELETE queries.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Result, Token, braced, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

use crate::codegen::sql_filter_expr_to_tokens;
use crate::errors::did_you_mean;
use crate::parse::{parse_filter_block, parse_optional_dialect};
use crate::types::{SqlDialect, SqlFilterExpr};

/// Valid options for `sql_delete!` macro.
const VALID_DELETE_OPTIONS: &[&str] = &["where", "filter", "returning"];

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
                other => {
                    let suggestion = did_you_mean(other, VALID_DELETE_OPTIONS);
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "Unknown option '{other}'.{suggestion}\n\n\
                             Valid options: where, filter, returning"
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
            where_expr,
            returning,
        })
    }
}

/// Build a DELETE query using object-like syntax.
pub fn sql_delete_impl(input: TokenStream) -> TokenStream {
    let DeleteInput {
        dialect,
        table,
        where_expr,
        returning,
    } = parse_macro_input!(input as DeleteInput);

    let table_str = table.to_string();
    let builder_constructor = dialect.delete_tokens(&table_str);

    let filter_chain = where_expr.map_or_else(
        || quote! {},
        |expr| {
            let expr_tokens = sql_filter_expr_to_tokens(&expr);
            quote! { .filter_expr(#expr_tokens) }
        },
    );

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
