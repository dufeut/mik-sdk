// =============================================================================
// CRATE-LEVEL QUALITY LINTS (following Tokio/Serde standards)
// =============================================================================
#![forbid(unsafe_code)]
#![deny(unused_must_use)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]
// Note: unreachable_pub is not applicable to proc-macro crates where internal
// functions need pub visibility for module organization but aren't exported

// =============================================================================
// CLIPPY CONFIGURATION
// =============================================================================
#![allow(clippy::doc_markdown)] // Code in docs - extensive changes needed
#![allow(clippy::missing_errors_doc)] // # Errors sections - doc-heavy
#![allow(clippy::missing_panics_doc)] // # Panics sections - doc-heavy

//! Proc-macros for `mik-sdk` - clean JSON syntax for WASI HTTP handlers.

use proc_macro::TokenStream;

mod derive;
mod dx;
mod http_client;
mod ids;
mod json;
mod response;
mod schema;

// Re-export internal types needed by other modules

// ============================================================================
// JSON Macro
// ============================================================================

/// Create a JSON value with clean syntax.
#[proc_macro]
pub fn json(input: TokenStream) -> TokenStream {
    json::json_impl(input)
}

// ============================================================================
// Response Macros
// ============================================================================

/// Return 200 OK with JSON body.
#[proc_macro]
pub fn ok(input: TokenStream) -> TokenStream {
    response::ok_impl(input)
}

/// Return RFC 7807 Problem Details error response.
#[proc_macro]
pub fn error(input: TokenStream) -> TokenStream {
    response::error_impl(input)
}

/// Create a 201 Created response.
#[proc_macro]
pub fn created(input: TokenStream) -> TokenStream {
    response::created_impl(input)
}

/// Create a 204 No Content response.
#[proc_macro]
pub fn no_content(input: TokenStream) -> TokenStream {
    response::no_content_impl(input)
}

/// Create a redirect response.
#[proc_macro]
pub fn redirect(input: TokenStream) -> TokenStream {
    response::redirect_impl(input)
}

/// Return 404 Not Found.
#[proc_macro]
pub fn not_found(input: TokenStream) -> TokenStream {
    response::not_found_impl(input)
}

/// Return 409 Conflict.
#[proc_macro]
pub fn conflict(input: TokenStream) -> TokenStream {
    response::conflict_impl(input)
}

/// Return 403 Forbidden.
#[proc_macro]
pub fn forbidden(input: TokenStream) -> TokenStream {
    response::forbidden_impl(input)
}

/// Return 400 Bad Request.
#[proc_macro]
pub fn bad_request(input: TokenStream) -> TokenStream {
    response::bad_request_impl(input)
}

/// Return 202 Accepted.
#[proc_macro]
pub fn accepted(input: TokenStream) -> TokenStream {
    response::accepted_impl(input)
}

// ============================================================================
// DX Macros
// ============================================================================

/// Early return validation guard.
#[proc_macro]
pub fn guard(input: TokenStream) -> TokenStream {
    dx::guard_impl(input)
}

/// Unwrap Option/Result or return error.
#[proc_macro]
pub fn ensure(input: TokenStream) -> TokenStream {
    dx::ensure_impl(input)
}

// ============================================================================
// HTTP Client Macro
// ============================================================================

/// Build an HTTP client request.
#[proc_macro]
pub fn fetch(input: TokenStream) -> TokenStream {
    http_client::fetch_impl(input)
}

// ============================================================================
// Utility Macros
// ============================================================================

/// Collect field values from a list.
#[proc_macro]
pub fn ids(input: TokenStream) -> TokenStream {
    ids::ids_impl(input)
}

/// Define routes with typed inputs and OpenAPI generation.
///
/// ```ignore
/// routes! {
///     GET "/users" => list_users(query: ListQuery) -> Vec<User>,
///     POST "/users" => create_user(body: CreateUserInput) -> User,
///     GET "/users/{id}" => get_user(path: Id) -> User,
///     PUT "/users/{id}" => update_user(path: Id, body: UpdateUser) -> User,
///     DELETE "/users/{id}" => delete_user(path: Id),
/// }
/// ```
#[proc_macro]
pub fn routes(input: TokenStream) -> TokenStream {
    schema::routes_impl(input)
}

// ============================================================================
// Derive Macros
// ============================================================================

/// Derive macro for JSON body types.
///
/// Generates `FromJson`, `Validate`, and `OpenApiSchema` implementations.
///
/// ```ignore
/// #[derive(Type)]
/// pub struct CreateUser {
///     #[field(min = 1, max = 100)]
///     pub name: String,
///
///     #[field(format = "email")]
///     pub email: String,
///
///     pub age: Option<i32>,
/// }
/// ```
#[proc_macro_derive(Type, attributes(field))]
pub fn derive_type(input: TokenStream) -> TokenStream {
    derive::derive_type_impl(input)
}

/// Derive macro for query parameter types.
///
/// Generates `FromQuery` implementation.
///
/// ```ignore
/// #[derive(Query)]
/// pub struct ListQuery {
///     #[field(default = 1)]
///     pub page: u32,
///
///     #[field(default = 20, max = 100)]
///     pub limit: u32,
///
///     pub search: Option<String>,
/// }
/// ```
#[proc_macro_derive(Query, attributes(field))]
pub fn derive_query(input: TokenStream) -> TokenStream {
    derive::derive_query_impl(input)
}

/// Derive macro for path parameter types.
///
/// Generates `FromPath` implementation.
///
/// ```ignore
/// #[derive(Path)]
/// pub struct OrgUserPath {
///     pub org_id: String,
///     pub id: String,
/// }
/// ```
#[proc_macro_derive(Path, attributes(field))]
pub fn derive_path(input: TokenStream) -> TokenStream {
    derive::derive_path_impl(input)
}
