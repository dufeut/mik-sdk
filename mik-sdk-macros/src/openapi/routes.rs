//! OpenAPI specification generation for routes.
//!
//! Generates OpenAPI 3.0 specifications with full type schemas.
//!
//! Strategy: Everything is computed once at startup via LazyLock.
//! Path/query parameters come from trait methods on the input types,
//! allowing full type information to be included.

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;

use crate::schema::types::{InputSource, RouteDef};

// =============================================================================
// OPENAPI GENERATION
// =============================================================================

/// Generate code that builds an OpenAPI method entry at runtime.
fn generate_method_entry_code(route: &RouteDef) -> TokenStream2 {
    let method_name = route.method.as_str();

    let mut parts: Vec<TokenStream2> = Vec::new();

    // Request body reference
    if let Some(body_input) = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Body))
    {
        let type_name = body_input.type_name.to_string();
        parts.push(quote! {
            __parts.push(::std::format!(
                "\"requestBody\":{{\"required\":true,\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{}\"}}}}}}}}",
                #type_name
            ));
        });
    }

    // Parameters (path + query) - collected from trait methods
    let path_input = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Path));
    let query_input = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Query));

    if path_input.is_some() || query_input.is_some() {
        let path_params_code = path_input.map_or_else(
            || quote! { let __path_params: &str = "[]"; },
            |input| {
                let type_name = &input.type_name;
                quote! {
                    let __path_params: &str = <super::#type_name as mik_sdk::typed::OpenApiSchema>::openapi_path_params();
                }
            },
        );

        let query_params_code = query_input.map_or_else(
            || quote! { let __query_params: &str = "[]"; },
            |input| {
                let type_name = &input.type_name;
                quote! {
                    let __query_params: &str = <super::#type_name as mik_sdk::typed::OpenApiSchema>::openapi_query_params();
                }
            },
        );

        parts.push(quote! {
            {
                #path_params_code
                #query_params_code
                // Merge path and query params (strip brackets and combine)
                let __path_inner = __path_params.trim_start_matches('[').trim_end_matches(']');
                let __query_inner = __query_params.trim_start_matches('[').trim_end_matches(']');
                let __all_params = match (__path_inner.is_empty(), __query_inner.is_empty()) {
                    (true, true) => ::std::string::String::new(),
                    (false, true) => __path_inner.to_string(),
                    (true, false) => __query_inner.to_string(),
                    (false, false) => ::std::format!("{},{}", __path_inner, __query_inner),
                };
                if !__all_params.is_empty() {
                    __parts.push(::std::format!("\"parameters\":[{}]", __all_params));
                }
            }
        });
    }

    // Response
    if let Some(ref output_type) = route.output_type {
        let output_str = output_type.to_string();
        parts.push(quote! {
            __parts.push(::std::format!(
                "\"responses\":{{\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{}\"}}}}}}}}}}",
                #output_str
            ));
        });
    } else {
        parts.push(quote! {
            __parts.push("\"responses\":{\"200\":{\"description\":\"Success\"}}".to_string());
        });
    }

    quote! {
        {
            let mut __parts: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();
            #(#parts)*
            ::std::format!("\"{}\":{{{}}}", #method_name, __parts.join(","))
        }
    }
}

/// Collect unique type names from routes for schema generation.
fn collect_type_names(routes: &[RouteDef]) -> Vec<Ident> {
    use std::collections::HashSet;

    let mut type_names: Vec<Ident> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for route in routes {
        for input in &route.inputs {
            let name = input.type_name.to_string();
            if seen.insert(name) {
                type_names.push(input.type_name.clone());
            }
        }
        if let Some(ref output) = route.output_type {
            let name = output.to_string();
            if seen.insert(name) {
                type_names.push(output.clone());
            }
        }
    }

    type_names
}

/// Generate code that builds paths JSON at runtime.
fn generate_paths_code(routes: &[RouteDef]) -> TokenStream2 {
    use std::collections::HashMap;

    // Group routes by path
    let mut paths: HashMap<String, Vec<&RouteDef>> = HashMap::new();
    for route in routes {
        let path = route
            .patterns
            .first()
            .map_or("/", std::string::String::as_str);
        paths.entry(path.to_string()).or_default().push(route);
    }

    // Generate code for each path
    let path_builders: Vec<TokenStream2> = paths
        .iter()
        .map(|(path, methods)| {
            let method_codes: Vec<TokenStream2> = methods
                .iter()
                .map(|r| generate_method_entry_code(r))
                .collect();
            quote! {
                {
                    let mut __methods: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();
                    #(
                        __methods.push(#method_codes);
                    )*
                    __path_entries.push(::std::format!("\"{}\":{{{}}}", #path, __methods.join(",")));
                }
            }
        })
        .collect();

    quote! {
        {
            let mut __path_entries: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();
            #(#path_builders)*
            __path_entries.join(",")
        }
    }
}

/// Generate the complete OpenAPI JSON with full type schemas.
///
/// Everything is computed once at startup via LazyLock.
/// Path/query parameters come from trait methods, allowing full type information.
pub fn generate_openapi_json(routes: &[RouteDef]) -> TokenStream2 {
    let paths_code = generate_paths_code(routes);
    let type_names = collect_type_names(routes);

    // Generate code to build schema entries by calling trait methods
    // Use super:: prefix because this runs inside __mik_schema module
    let schema_builders: Vec<TokenStream2> = type_names
        .iter()
        .map(|type_name| {
            let type_name_str = type_name.to_string();
            quote! {
                __schema_parts.push(::std::format!(
                    "\"{}\":{}",
                    #type_name_str,
                    <super::#type_name as mik_sdk::typed::OpenApiSchema>::openapi_schema()
                ));
            }
        })
        .collect();

    // Return code that builds the OpenAPI JSON at init time
    quote! {
        {
            let __paths_json = #paths_code;
            let mut __schema_parts: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();
            #(#schema_builders)*
            let __schemas_json = __schema_parts.join(",");
            ::std::format!(
                r#"{{"openapi":"3.0.0","info":{{"title":"API","version":"1.0.0"}},"paths":{{{}}},"components":{{"schemas":{{{}}}}}}}"#,
                __paths_json,
                __schemas_json
            )
        }
    }
}
