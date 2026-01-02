//! `sql_create!` macro implementation for INSERT queries.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Result, Token, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::codegen::{generate_returning_chain, sql_value_to_tokens};
use crate::parse::{parse_ident_list, parse_optional_dialect, parse_sql_value};
use crate::types::{SqlDialect, SqlValue};

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
                returning = parse_ident_list(&content)?;
            } else {
                let value = parse_sql_value(&content)?;
                columns.push((key, value));
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            dialect,
            table,
            columns,
            returning,
        })
    }
}

/// Build an INSERT query using object-like syntax.
pub fn sql_create_impl(input: TokenStream) -> TokenStream {
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

    let returning_chain = generate_returning_chain(&returning);

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
