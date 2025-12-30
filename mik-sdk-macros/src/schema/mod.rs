//! Routes macro for typed handlers with OpenAPI generation.
//!
//! New flat syntax with typed inputs:
//! ```ignore
//! routes! {
//!     GET "/users" => list_users(query: ListQuery) -> Vec<User>,
//!     POST "/users" => create_user(body: CreateUserInput) -> User,
//!     GET "/users/{id}" => get_user(path: Id) -> User,
//!     PUT "/users/{id}" => update_user(path: Id, body: UpdateUser) -> User,
//!     DELETE "/users/{id}" => delete_user(path: Id),
//! }
//! ```

mod field_schema;
mod openapi;
mod type_schema;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse_macro_input;

use field_schema::generate_route_block;
use openapi::generate_openapi_json;
use type_schema::RoutesDef;

// =============================================================================
// MAIN IMPLEMENTATION
// =============================================================================

pub fn routes_impl(input: TokenStream) -> TokenStream {
    let defs = parse_macro_input!(input as RoutesDef);

    // Validate for duplicate routes (same method + pattern)
    {
        use std::collections::HashSet;
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for route in &defs.routes {
            let method_str = route.method.as_str();
            for pattern in &route.patterns {
                if !seen.insert((method_str, pattern.as_str())) {
                    return syn::Error::new_spanned(
                        &route.handler,
                        format!(
                            "Duplicate route: {} \"{}\" is already defined. Each method + pattern combination must be unique.",
                            method_str.to_uppercase(),
                            pattern
                        )
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }
    }

    let route_blocks: Vec<TokenStream2> = defs.routes.iter().map(generate_route_block).collect();

    let openapi_generator = generate_openapi_json(&defs.routes);

    let tokens = quote! {
        // Compile-time check: ensure bindings module is properly configured.
        // If you see an error here, make sure you have:
        //   1. `mod bindings;` at the top of your lib.rs
        //   2. Generated bindings via cargo-component build
        //   3. The bindings module exports `mik::core::handler::{Guest, Response, RequestData, Method}`
        const _: () = {
            // This const assertion verifies the Guest trait is accessible
            fn __mik_check_bindings_setup() {
                fn __check<T: handler::Guest>() {}
            }
        };

        /// Handler for /__schema endpoint - returns OpenAPI JSON.
        pub fn __schema(_req: &mik_sdk::Request) -> handler::Response {
            let schema_json = #openapi_generator;
            handler::Response {
                status: 200,
                headers: vec![
                    (
                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                        mik_sdk::constants::MIME_JSON.to_string()
                    ),
                ],
                body: Some(schema_json.into_bytes()),
            }
        }

        struct Handler;

        impl Guest for Handler {
            fn handle(__mik_raw: handler::RequestData) -> handler::Response {
                let __mik_method = match __mik_raw.method {
                    handler::Method::Get => mik_sdk::Method::Get,
                    handler::Method::Post => mik_sdk::Method::Post,
                    handler::Method::Put => mik_sdk::Method::Put,
                    handler::Method::Patch => mik_sdk::Method::Patch,
                    handler::Method::Delete => mik_sdk::Method::Delete,
                    handler::Method::Head => mik_sdk::Method::Head,
                    handler::Method::Options => mik_sdk::Method::Options,
                };

                let __mik_path = __mik_raw.path.split('?').next().unwrap_or(&__mik_raw.path);

                // Check for /__schema route first
                if __mik_path == "/__schema" {
                    let __mik_req = mik_sdk::Request::new(
                        __mik_method,
                        __mik_raw.path.clone(),
                        __mik_raw.headers.clone(),
                        __mik_raw.body.clone(),
                        ::std::collections::HashMap::new(),
                    );
                    return __schema(&__mik_req);
                }

                #(#route_blocks)*

                // No route matched - return 404
                handler::Response {
                    status: 404,
                    headers: vec![
                        (
                            mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                            mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                        )
                    ],
                    body: Some(mik_sdk::json::obj()
                        .set("type", mik_sdk::json::str("about:blank"))
                        .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(404)))
                        .set("status", mik_sdk::json::int(404))
                        .set("detail", mik_sdk::json::str("Route not found"))
                        .to_bytes()),
                }
            }
        }

        // Allow unsafe_code for generated WIT bindings export macro
        #[allow(unsafe_code)]
        const _: () = { bindings::export!(Handler with_types_in bindings); };
    };

    TokenStream::from(tokens)
}
