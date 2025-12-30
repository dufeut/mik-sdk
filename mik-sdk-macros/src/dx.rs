//! Developer experience macros: guard!, ensure!.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, LitInt, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

// =============================================================================
// NEW DX MACROS
// =============================================================================

/// Early return validation guard.
///
/// If the condition is false, returns an error response immediately.
///
/// # Examples
///
/// ```ignore
/// fn create_user(req: &Request) -> handler::Response {
///     let name = req.param("name").unwrap_or_default();
///
///     guard!(!name.is_empty(), 400, "Name is required");
///     guard!(name.len() <= 100, 400, "Name too long");
///
///     ok!({ "created": true })
/// }
/// ```
struct GuardInput {
    condition: Expr,
    status: LitInt,
    message: LitStr,
}

impl Parse for GuardInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let condition: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected a condition expression.\n\
                     \n\
                     The guard! macro takes: guard!(condition, status_code, \"message\")\n\
                     \n\
                     Example:\n\
                     guard!(!name.is_empty(), 400, \"Name is required\");\n\
                     guard!(age >= 18, 400, \"Must be 18 or older\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![,]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected comma after condition.\n\
                     \n\
                     Correct syntax: guard!(condition, status_code, \"message\")\n\
                     \n\
                     Example:\n\
                     guard!(!name.is_empty(), 400, \"Name is required\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let status: LitInt = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected HTTP status code (integer literal).\n\
                     \n\
                     Common status codes:\n\
                     - 400 Bad Request\n\
                     - 401 Unauthorized\n\
                     - 403 Forbidden\n\
                     - 404 Not Found\n\
                     - 422 Unprocessable Entity\n\
                     \n\
                     Example:\n\
                     guard!(!name.is_empty(), 400, \"Name is required\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![,]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected comma after status code.\n\
                     \n\
                     Correct syntax: guard!(condition, {status}, \"message\")\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let message: LitStr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected error message (string literal).\n\
                     \n\
                     The message should describe why the guard failed.\n\
                     \n\
                     Example:\n\
                     guard!(!name.is_empty(), 400, \"Name is required\");\n\
                     guard!(items.len() <= 100, 400, \"Too many items (max 100)\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        Ok(Self {
            condition,
            status,
            message,
        })
    }
}

pub fn guard_impl(input: TokenStream) -> TokenStream {
    let GuardInput {
        condition,
        status,
        message,
    } = parse_macro_input!(input as GuardInput);

    let tokens = quote! {
        if !(#condition) {
            let __mik_sdk_status_code = #status as u16;
            let __mik_sdk_title = ::mik_sdk::constants::status_title(__mik_sdk_status_code);
            let __mik_sdk_body = json::obj()
                .set("type", json::str("about:blank"))
                .set("title", json::str(__mik_sdk_title))
                .set("status", json::int(__mik_sdk_status_code as i64))
                .set("detail", json::str(#message));
            return handler::Response {
                status: __mik_sdk_status_code,
                headers: vec![
                    (
                        ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                        ::mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                    )
                ],
                body: Some(__mik_sdk_body.to_bytes()),
            };
        }
    };

    TokenStream::from(tokens)
}

/// Unwrap Option/Result or return error response.
///
/// Works with both Option and Result types. Returns early with an error
/// response if the value is None or Err.
///
/// # Examples
///
/// ```ignore
/// fn get_user(req: &Request) -> handler::Response {
///     // Unwrap Option - returns 404 if None
///     let user = ensure!(find_user(id), 404, "User not found");
///
///     // Unwrap Result - returns 400 if Err
///     let data = ensure!(parse_json(body), 400, "Invalid JSON");
///
///     // With dynamic message
///     let item = ensure!(find_item(id), 404, format!("Item {} not found", id));
///
///     ok!({ "user": str(user.name) })
/// }
/// ```
struct EnsureInput {
    expr: Expr,
    status: LitInt,
    message: Expr,
}

impl Parse for EnsureInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let expr: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected an expression that returns Option or Result.\n\
                     \n\
                     The ensure! macro unwraps Option/Result or returns an error:\n\
                     ensure!(expression, status_code, \"message\")\n\
                     ensure!(expression, status_code, format!(\"...\", args))\n\
                     \n\
                     Example:\n\
                     let user = ensure!(find_user(id), 404, \"User not found\");\n\
                     let data = ensure!(parse_json(body), 400, \"Invalid JSON\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![,]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected comma after expression.\n\
                     \n\
                     Correct syntax: ensure!(expr, status_code, message)\n\
                     \n\
                     Example:\n\
                     let user = ensure!(find_user(id), 404, \"User not found\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let status: LitInt = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected HTTP status code (integer literal).\n\
                     \n\
                     Common status codes:\n\
                     - 400 Bad Request (for invalid input)\n\
                     - 404 Not Found (for missing resources)\n\
                     - 500 Internal Server Error\n\
                     \n\
                     Example:\n\
                     let user = ensure!(find_user(id), 404, \"User not found\");\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![,]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected comma after status code.\n\
                     \n\
                     Correct syntax: ensure!(expr, {status}, message)\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let message: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected error message (string literal or format! expression).\n\
                     \n\
                     The message can be:\n\
                     - A string literal: \"User not found\"\n\
                     - A format! expression: format!(\"User {{}} not found\", id)\n\
                     \n\
                     Example:\n\
                     let user = ensure!(find_user(id), 404, \"User not found\");\n\
                     let item = ensure!(get_item(id), 404, format!(\"Item {{}} not found\", id));\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        Ok(Self {
            expr,
            status,
            message,
        })
    }
}

pub fn ensure_impl(input: TokenStream) -> TokenStream {
    let EnsureInput {
        expr,
        status,
        message,
    } = parse_macro_input!(input as EnsureInput);

    let tokens = quote! {
        match ::mik_sdk::__ensure_helper(#expr) {
            Some(__mik_sdk_val) => __mik_sdk_val,
            None => {
                let __mik_sdk_status_code = #status as u16;
                let __mik_sdk_title = ::mik_sdk::constants::status_title(__mik_sdk_status_code);
                let __mik_sdk_body = json::obj()
                    .set("type", json::str("about:blank"))
                    .set("title", json::str(__mik_sdk_title))
                    .set("status", json::int(__mik_sdk_status_code as i64))
                    .set("detail", json::str(&#message));
                return handler::Response {
                    status: __mik_sdk_status_code,
                    headers: vec![
                        (
                            ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                            ::mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                        )
                    ],
                    body: Some(__mik_sdk_body.to_bytes()),
                };
            }
        }
    };

    TokenStream::from(tokens)
}
