// =============================================================================
// CRATE-LEVEL QUALITY LINTS (following Tokio/Serde standards)
// =============================================================================
#![forbid(unsafe_code)]
#![deny(unused_must_use)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]
// Note: unreachable_pub is not applicable to proc-macro crates where internal
// functions need pub visibility for module organization but aren't exported
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
// =============================================================================
// CLIPPY CONFIGURATION FOR PROC-MACRO CRATES
// =============================================================================
// These lints are relaxed for proc-macro crates where syn/quote patterns are used
#![allow(clippy::indexing_slicing)] // Checked by syn parsing structure
#![allow(elided_lifetimes_in_paths)] // Common pattern with ParseStream

//! Proc-macros for mik-sql - SQL query builder with Mongo-style filters.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

mod common;
mod create;
mod delete;
mod read;
mod update;

// ============================================================================
// SQL CRUD MACROS - Query builder with JSON-like syntax
// ============================================================================

/// Build a SELECT query using the query builder (CRUD: Read).
///
/// # Example
/// ```ignore
/// let (sql, params) = sql_read!(users {
///     select: [id, name, email],
///     filter: { active: true },
///     order: name,
///     limit: 10,
/// });
/// ```
#[proc_macro]
pub fn sql_read(input: TokenStream) -> TokenStream {
    read::sql_read_impl(input)
}

/// Build an INSERT query using object-like syntax (CRUD: Create).
///
/// # Example
/// ```ignore
/// let (sql, params) = sql_create!(users {
///     name: str(name),
///     email: str(email),
///     returning: [id],
/// });
/// ```
#[proc_macro]
pub fn sql_create(input: TokenStream) -> TokenStream {
    create::sql_create_impl(input)
}

/// Build an UPDATE query using object-like syntax (CRUD: Update).
///
/// # Example
/// ```ignore
/// let (sql, params) = sql_update!(users {
///     set: { name: str(new_name) },
///     filter: { id: int(user_id) },
/// });
/// ```
#[proc_macro]
pub fn sql_update(input: TokenStream) -> TokenStream {
    update::sql_update_impl(input)
}

/// Build a DELETE query using object-like syntax (CRUD: Delete).
///
/// # Example
/// ```ignore
/// let (sql, params) = sql_delete!(users {
///     filter: { id: int(user_id) },
/// });
/// ```
#[proc_macro]
pub fn sql_delete(input: TokenStream) -> TokenStream {
    delete::sql_delete_impl(input)
}

// ============================================================================
// IDS MACRO - Collect field values from a list for batched loading
// ============================================================================

/// Collect field values from a list for batched loading.
///
/// # Example
/// ```ignore
/// let user_ids = ids!(users);           // extracts .id from each item
/// let author_ids = ids!(posts, author_id);  // extracts .author_id from each item
/// ```
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

        Ok(Self { list, field })
    }
}
