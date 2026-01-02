//! OpenAPI specification generation for routes.
//!
//! Generates OpenAPI 3.0 specifications as compile-time static strings
//! to avoid runtime stack overflow issues in WASM environments.
//!
//! Strategy: Everything is computed at macro expansion time and embedded
//! as a single static string. No runtime computation needed.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::schema::codegen::extract_param_names;
use crate::schema::types::{InputSource, RouteDef};

// =============================================================================
// FULLY STATIC OPENAPI GENERATION
// =============================================================================

/// Generate a static OpenAPI method entry for a route.
fn generate_method_entry(route: &RouteDef) -> String {
    let method_name = route.method.as_str();
    let path = route
        .patterns
        .first()
        .map_or("/", std::string::String::as_str);

    let mut parts: Vec<String> = Vec::new();

    // Request body reference
    if let Some(body_input) = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Body))
    {
        let type_name = body_input.type_name.to_string();
        parts.push(format!(
            "\"requestBody\":{{\"required\":true,\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{type_name}\"}}}}}}}}"
        ));
    }

    // Parameters (path + query)
    let mut params: Vec<String> = Vec::new();

    // Path parameters from URL pattern
    for name in extract_param_names(path) {
        params.push(format!(
            "{{\"name\":\"{name}\",\"in\":\"path\",\"required\":true,\"schema\":{{\"type\":\"string\"}}}}"
        ));
    }

    // Query parameters - reference the type for schema lookup
    if let Some(query_input) = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Query))
    {
        let type_name = query_input.type_name.to_string();
        // Add a reference note - full query params are in the schema
        params.push(format!(
            "{{\"name\":\"(see {type_name})\",\"in\":\"query\",\"required\":false,\"schema\":{{\"$ref\":\"#/components/schemas/{type_name}\"}}}}"
        ));
    }

    if !params.is_empty() {
        parts.push(format!("\"parameters\":[{}]", params.join(",")));
    }

    // Response
    if let Some(ref output_type) = route.output_type {
        parts.push(format!(
            "\"responses\":{{\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{output_type}\"}}}}}}}}}}"
        ));
    } else {
        parts.push("\"responses\":{\"200\":{\"description\":\"Success\"}}".to_string());
    }

    format!("\"{}\":{{{}}}", method_name, parts.join(","))
}

/// Generate the complete OpenAPI JSON as a compile-time static string.
///
/// This is the main entry point. Everything is computed at macro expansion time.
pub fn generate_openapi_json(routes: &[RouteDef]) -> TokenStream2 {
    use std::collections::{HashMap, HashSet};

    // Group routes by path
    let mut paths: HashMap<String, Vec<&RouteDef>> = HashMap::new();
    for route in routes {
        let path = route
            .patterns
            .first()
            .map_or("/", std::string::String::as_str);
        paths.entry(path.to_string()).or_default().push(route);
    }

    // Build paths JSON
    let path_entries: Vec<String> = paths
        .iter()
        .map(|(path, methods)| {
            let method_entries: Vec<String> =
                methods.iter().map(|r| generate_method_entry(r)).collect();
            format!("\"{}\":{{{}}}", path, method_entries.join(","))
        })
        .collect();

    let paths_json = path_entries.join(",");

    // Collect unique type names for schema references
    let mut type_names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for route in routes {
        for input in &route.inputs {
            let name = input.type_name.to_string();
            if seen.insert(name.clone()) {
                type_names.push(name);
            }
        }
        if let Some(ref output) = route.output_type {
            let name = output.to_string();
            if seen.insert(name.clone()) {
                type_names.push(name);
            }
        }
    }

    // Build schema references (just type names pointing to empty objects)
    // Full schemas would require trait calls, so we provide references only
    let schema_entries: Vec<String> = type_names
        .iter()
        .map(|name| {
            format!("\"{name}\":{{\"type\":\"object\",\"description\":\"See type definition\"}}")
        })
        .collect();

    let schemas_json = schema_entries.join(",");

    // Build the complete static OpenAPI JSON
    let static_openapi = format!(
        r#"{{"openapi":"3.0.0","info":{{"title":"API","version":"1.0.0"}},"paths":{{{paths_json}}},"components":{{"schemas":{{{schemas_json}}}}}}}"#
    );

    // Return the static string literal directly (for use in a const)
    quote! { #static_openapi }
}
