//! OpenAPI specification generation for routes.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Ident;

use super::field_schema::extract_param_names;
use super::type_schema::{InputSource, RouteDef};

// =============================================================================
// CODE GENERATION - OPENAPI
// =============================================================================

/// Generate runtime code to build an OpenAPI path entry for a route.
///
/// Returns TokenStream2 that evaluates to a String containing the method entry JSON.
pub fn generate_openapi_path_entry_code(route: &RouteDef) -> TokenStream2 {
    let method_name = route.method.as_str();
    let path = route
        .patterns
        .first()
        .map_or("/", std::string::String::as_str);

    // Build request body reference if we have a body input (static)
    let request_body_str = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Body))
        .map(|i| {
            let type_name = i.type_name.to_string();
            format!(
                "\"requestBody\":{{\"required\":true,\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{type_name}\"}}}}}}}}"
            )
        })
        .unwrap_or_default();

    // Build path parameters (static - derived from URL pattern)
    let path_params: Vec<String> = extract_param_names(path)
        .into_iter()
        .map(|name| {
            format!(
                "{{\"name\":\"{name}\",\"in\":\"path\",\"required\":true,\"schema\":{{\"type\":\"string\"}}}}"
            )
        })
        .collect();
    let path_params_str = path_params.join(",");

    // Build response (static)
    let response_str = route.output_type.as_ref().map_or_else(
        || "\"responses\":{\"200\":{\"description\":\"Success\"}}".to_string(),
        |t| {
            format!(
                "\"responses\":{{\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{t}\"}}}}}}}}}}"
            )
        },
    );

    // Check if we have query parameters - if so, we need runtime code
    let query_type = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Query))
        .map(|i| &i.type_name);

    // Generate code based on whether we have query params
    if let Some(query_type) = query_type {
        // Runtime: need to call openapi_query_params() and merge with path params
        quote! {
            {
                // Get query parameters from the type's trait implementation
                let __mik_query_params = <#query_type as mik_sdk::typed::OpenApiSchema>::openapi_query_params();
                // Strip the surrounding brackets to get just the contents
                let __mik_query_inner = __mik_query_params
                    .strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .unwrap_or("");

                // Combine path params and query params
                let __mik_all_params = if #path_params_str.is_empty() && __mik_query_inner.is_empty() {
                    String::new()
                } else if #path_params_str.is_empty() {
                    format!("\"parameters\":[{}]", __mik_query_inner)
                } else if __mik_query_inner.is_empty() {
                    format!("\"parameters\":[{}]", #path_params_str)
                } else {
                    format!("\"parameters\":[{},{}]", #path_params_str, __mik_query_inner)
                };

                // Build all parts
                let mut __mik_parts: Vec<String> = Vec::new();
                let __mik_request_body = #request_body_str;
                if !__mik_request_body.is_empty() {
                    __mik_parts.push(__mik_request_body.to_string());
                }
                if !__mik_all_params.is_empty() {
                    __mik_parts.push(__mik_all_params);
                }
                __mik_parts.push(#response_str.to_string());

                format!("\"{}\":{{{}}}", #method_name, __mik_parts.join(","))
            }
        }
    } else {
        // Static: no query params, can build entire string at compile time
        let mut static_parts: Vec<String> = Vec::new();
        if !request_body_str.is_empty() {
            static_parts.push(request_body_str);
        }
        if !path_params_str.is_empty() {
            static_parts.push(format!("\"parameters\":[{path_params_str}]"));
        }
        static_parts.push(response_str);

        let static_json = format!("\"{}\":{{{}}}", method_name, static_parts.join(","));
        quote! { #static_json.to_string() }
    }
}

pub fn generate_openapi_json(routes: &[RouteDef]) -> TokenStream2 {
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

    // Collect all schema type names
    let mut schema_types: Vec<&Ident> = Vec::new();
    for route in routes {
        for input in &route.inputs {
            schema_types.push(&input.type_name);
        }
        if let Some(ref output) = route.output_type {
            schema_types.push(output);
        }
    }

    // Generate runtime code for each path entry
    // Each path generates code that builds its methods JSON
    let path_builders: Vec<TokenStream2> = paths
        .iter()
        .map(|(path, methods)| {
            let method_codes: Vec<TokenStream2> = methods
                .iter()
                .map(|r| generate_openapi_path_entry_code(r))
                .collect();

            quote! {
                {
                    let __mik_methods: Vec<String> = vec![
                        #(#method_codes),*
                    ];
                    format!(r#""{}":{{{}}}"#, #path, __mik_methods.join(","))
                }
            }
        })
        .collect();

    // Generate schema collection code that calls OpenApiSchema::openapi_schema()
    // at runtime for each type
    let schema_collectors: Vec<TokenStream2> = schema_types
        .iter()
        .map(|t| {
            let name = t.to_string();
            quote! {
                __mik_schemas.push(format!(
                    r#""{}":{}"#,
                    #name,
                    <#t as mik_sdk::typed::OpenApiSchema>::openapi_schema()
                ));
            }
        })
        .collect();

    quote! {
        {
            // Build paths at runtime (to support query param expansion)
            let __mik_paths: Vec<String> = vec![
                #(#path_builders),*
            ];
            let __mik_paths_json = format!("{{{}}}", __mik_paths.join(","));

            // Build schemas at runtime
            let mut __mik_schemas: Vec<String> = Vec::new();
            #(#schema_collectors)*

            format!(
                r#"{{"openapi":"3.0.0","info":{{"title":"API","version":"1.0.0"}},"paths":{},"components":{{"schemas":{{{}}}}}}}"#,
                __mik_paths_json,
                __mik_schemas.join(",")
            )
        }
    }
}
