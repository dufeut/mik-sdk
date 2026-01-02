//! Common parsing utilities for SQL CRUD macros.

use syn::{
    Result, Token, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use super::filter::parse_sql_value;
use crate::types::{SqlDialect, SqlValue};

/// Parse optional dialect prefix: `sql_read!(sqlite, users { ... })`
pub fn parse_optional_dialect(input: ParseStream) -> Result<SqlDialect> {
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

/// Parse column-value pairs for INSERT/UPDATE operations.
pub fn parse_column_values(input: ParseStream) -> Result<Vec<(syn::Ident, SqlValue)>> {
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

/// Parse a bracketed identifier list: `[field1, field2, ...]`.
///
/// Used for `select:`, `group_by:`, `allow:`, `allow_sort:`, `returning:`, etc.
pub fn parse_ident_list(input: ParseStream) -> Result<Vec<syn::Ident>> {
    let content;
    bracketed!(content in input);
    let fields: Punctuated<syn::Ident, Token![,]> =
        content.parse_terminated(syn::Ident::parse, Token![,])?;
    Ok(fields.into_iter().collect())
}
