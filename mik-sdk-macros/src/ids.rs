//! IDS! macro for collecting field values.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

// ============================================================================
// IDS! MACRO - Collect field values for batched loading
// ============================================================================

/// Collect field values from a list for batched loading.
///
/// # Example
///
/// ```ignore
/// // Collect .id field (default)
/// let user_ids = ids!(users);
///
/// // Collect specific field
/// let user_ids = ids!(users, user_id);
/// let emails = ids!(users, email);
/// ```
pub fn ids_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as IdsInput);

    let list = &input.list;
    let field = &input.field;

    let tokens = quote! {
        #list.iter().map(|__item| __item.#field.clone()).collect::<Vec<_>>()
    };

    TokenStream::from(tokens)
}

/// Input for the ids! macro.
struct IdsInput {
    list: Expr,
    field: syn::Ident,
}

impl Parse for IdsInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let list: Expr = input.parse()?;

        let field = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            input.parse()?
        } else {
            // Default to "id"
            syn::Ident::new("id", proc_macro2::Span::call_site())
        };

        Ok(IdsInput { list, field })
    }
}
