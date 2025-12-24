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

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Ident, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

// =============================================================================
// TYPES
// =============================================================================

#[derive(Clone)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "get",
            HttpMethod::Post => "post",
            HttpMethod::Put => "put",
            HttpMethod::Patch => "patch",
            HttpMethod::Delete => "delete",
            HttpMethod::Head => "head",
            HttpMethod::Options => "options",
        }
    }
}

/// Input source for typed parameters
#[derive(Clone)]
enum InputSource {
    Path,  // from URL path params
    Body,  // from JSON body
    Query, // from query string
}

/// A typed input parameter for a handler
#[derive(Clone)]
struct TypedInput {
    source: InputSource,
    type_name: Ident,
}

/// A route definition
struct RouteDef {
    method: HttpMethod,
    patterns: Vec<String>,
    handler: Ident,
    inputs: Vec<TypedInput>,
    output_type: Option<Ident>,
}

/// All routes in the macro
struct RoutesDef {
    routes: Vec<RouteDef>,
}

// =============================================================================
// PARSING
// =============================================================================

impl Parse for RoutesDef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut routes = Vec::new();

        while !input.is_empty() {
            let route = parse_route(input)?;
            routes.push(route);

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(RoutesDef { routes })
    }
}

fn parse_route(input: ParseStream<'_>) -> Result<RouteDef> {
    // Parse method: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS
    let method_ident: Ident = input.parse().map_err(|e| {
        syn::Error::new(
            e.span(),
            format!(
                "Expected HTTP method at start of route definition.\n\
                 \n\
                 Valid methods: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS\n\
                 \n\
                 Example:\n\
                 routes! {{\n\
                     GET \"/users\" => list_users,\n\
                     POST \"/users\" => create_user(body: CreateUser) -> User,\n\
                 }}\n\
                 \n\
                 Original error: {e}"
            ),
        )
    })?;

    let method_str = method_ident.to_string().to_uppercase();
    let method = match method_str.as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => {
            return Err(syn::Error::new_spanned(
                &method_ident,
                format!(
                    "Invalid HTTP method '{}'.\n\
                     \n\
                     Valid methods: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS\n\
                     \n\
                     Example: GET \"/users\" => list_users",
                    method_ident
                ),
            ));
        },
    };

    // Parse pattern(s): "/path" or "/path" | "/other"
    let mut patterns = Vec::new();
    let first_pattern: LitStr = input.parse().map_err(|e| {
        syn::Error::new(
            e.span(),
            format!(
                "Expected route path (string literal) after HTTP method '{}'.\n\
                 \n\
                 Correct syntax: {} \"/path\" => handler\n\
                 \n\
                 Common mistakes:\n\
                 - Path must be a string literal: {} \"/users\" ✓ not {} /users ✗\n\
                 - Path should start with /: {} \"/users\" ✓ not {} \"users\" ✗\n\
                 \n\
                 Original error: {e}",
                method_str, method_str, method_str, method_str, method_str, method_str
            ),
        )
    })?;
    patterns.push(first_pattern.value());

    while input.peek(Token![|]) {
        input.parse::<Token![|]>()?;
        let alt_pattern: LitStr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected alternative route path after '|'.\n\
                     \n\
                     Correct syntax: {} \"/path\" | \"/alt-path\" => handler\n\
                     \n\
                     Original error: {e}",
                    method_str
                ),
            )
        })?;
        patterns.push(alt_pattern.value());
    }

    // Parse =>
    input.parse::<Token![=>]>().map_err(|e| {
        syn::Error::new(
            e.span(),
            format!(
                "Expected '=>' after route path.\n\
                 \n\
                 Correct syntax: {} \"{}\" => handler_name\n\
                 \n\
                 Common mistakes:\n\
                 - Use => not -> for route arrow: {} \"{}\" => handler ✓\n\
                 - Use => not : for route arrow: {} \"{}\" => handler ✓\n\
                 \n\
                 Original error: {e}",
                method_str,
                patterns.first().unwrap_or(&String::from("/path")),
                method_str,
                patterns.first().unwrap_or(&String::from("/path")),
                method_str,
                patterns.first().unwrap_or(&String::from("/path")),
            ),
        )
    })?;

    // Parse handler name
    let handler: Ident = input.parse().map_err(|e| {
        syn::Error::new(
            e.span(),
            format!(
                "Expected handler function name after '=>'.\n\
                 \n\
                 Correct syntax: {} \"{}\" => handler_name\n\
                 \n\
                 The handler must be an identifier (function name), not a string.\n\
                 \n\
                 Example:\n\
                 fn list_users(_req: &Request) -> Response {{ ... }}\n\
                 \n\
                 routes! {{\n\
                     {} \"{}\" => list_users,\n\
                 }}\n\
                 \n\
                 Original error: {e}",
                method_str,
                patterns.first().unwrap_or(&String::from("/path")),
                method_str,
                patterns.first().unwrap_or(&String::from("/path")),
            ),
        )
    })?;

    // Parse optional typed inputs: (path: Id, body: CreateUser, query: ListQuery)
    let inputs = if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        parse_typed_inputs(&content, &method_str, &patterns, &handler)?
    } else {
        Vec::new()
    };

    // Parse optional output type: -> User or -> Vec<User>
    let output_type = if input.peek(Token![->]) {
        input.parse::<Token![->]>()?;
        // For now just parse as Ident, handle Vec<T> later if needed
        let type_ident: Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected response type name after '->'.\n\
                     \n\
                     Correct syntax: {} \"{}\" => {}(...) -> ResponseType\n\
                     \n\
                     The response type should be an identifier like User, Vec<User>, etc.\n\
                     \n\
                     Example:\n\
                     {} \"{}\" => {} -> User,\n\
                     \n\
                     Original error: {e}",
                    method_str,
                    patterns.first().unwrap_or(&String::from("/path")),
                    handler,
                    method_str,
                    patterns.first().unwrap_or(&String::from("/path")),
                    handler,
                ),
            )
        })?;
        Some(type_ident)
    } else {
        None
    };

    Ok(RouteDef {
        method,
        patterns,
        handler,
        inputs,
        output_type,
    })
}

fn parse_typed_inputs(
    input: ParseStream<'_>,
    method_str: &str,
    patterns: &[String],
    handler: &Ident,
) -> Result<Vec<TypedInput>> {
    let mut inputs = Vec::new();
    let path = patterns.first().map(|s| s.as_str()).unwrap_or("/path");

    while !input.is_empty() {
        // Parse source: path, body, or query
        let source_ident: Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected input source in handler parameters.\n\
                     \n\
                     Valid sources:\n\
                     - path: Type   - URL path parameters (e.g., /users/{{id}})\n\
                     - body: Type   - JSON request body\n\
                     - query: Type  - Query string parameters\n\
                     \n\
                     Example:\n\
                     {} \"{}\" => {}(path: UserId, body: CreateUser, query: Pagination) -> User\n\
                     \n\
                     Original error: {e}",
                    method_str, path, handler
                ),
            )
        })?;

        let source = match source_ident.to_string().as_str() {
            "path" => InputSource::Path,
            "body" => InputSource::Body,
            "query" => InputSource::Query,
            other => {
                return Err(syn::Error::new_spanned(
                    &source_ident,
                    format!(
                        "Invalid input source '{}'.\n\
                         \n\
                         Valid sources:\n\
                         - path  - URL path parameters (e.g., /users/{{id}})\n\
                         - body  - JSON request body\n\
                         - query - Query string parameters\n\
                         \n\
                         Example:\n\
                         {} \"{}\" => {}(path: Id, body: CreateUser) -> User",
                        other, method_str, path, handler
                    ),
                ));
            },
        };

        // Parse colon
        input.parse::<Token![:]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected ':' after input source '{}'.\n\
                     \n\
                     Correct syntax: {}: TypeName\n\
                     \n\
                     Example:\n\
                     {} \"{}\" => {}({}: UserId)\n\
                     \n\
                     Original error: {e}",
                    source_ident, source_ident, method_str, path, handler, source_ident
                ),
            )
        })?;

        // Parse type name
        let type_name: Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected type name after '{}: '.\n\
                     \n\
                     The type must be a struct that derives the appropriate trait:\n\
                     - path: Type   - Type must derive Path\n\
                     - body: Type   - Type must derive Type (for JSON parsing)\n\
                     - query: Type  - Type must derive Query\n\
                     \n\
                     Example:\n\
                     #[derive(Path)]\n\
                     struct UserId {{ id: String }}\n\
                     \n\
                     {} \"{}\" => {}({}: UserId)\n\
                     \n\
                     Original error: {e}",
                    source_ident, method_str, path, handler, source_ident
                ),
            )
        })?;

        inputs.push(TypedInput { source, type_name });

        // Optional comma
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(inputs)
}

// =============================================================================
// CODE GENERATION - ROUTE MATCHING
// =============================================================================

fn extract_param_names(pattern: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            if !name.is_empty() {
                params.push(name);
            }
        }
    }

    params
}

fn generate_pattern_matcher(pattern: &str) -> TokenStream2 {
    let params = extract_param_names(pattern);

    if params.is_empty() {
        quote! {
            (|| -> Option<::std::collections::HashMap<String, String>> {
                if __mik_path == #pattern {
                    Some(::std::collections::HashMap::new())
                } else {
                    None
                }
            })()
        }
    } else {
        let segments: Vec<&str> = pattern.split('/').collect();
        let segment_count = segments.len();

        let mut checks = Vec::new();
        let mut extractions = Vec::new();

        for (i, segment) in segments.iter().enumerate() {
            if segment.starts_with('{') && segment.ends_with('}') {
                // Parameter segment - URL decode the value
                let param_name = &segment[1..segment.len() - 1];
                extractions.push(quote! {
                    let __mik_raw_param = __mik_segments[#i];
                    // URL decode the path parameter. If decoding fails (malformed percent-encoding),
                    // fall back to the raw value. This is intentional: invalid encoding shouldn't
                    // crash the handler, and the raw value will either match the route or not.
                    let __mik_decoded_param = mik_sdk::url_decode(__mik_raw_param)
                        .unwrap_or_else(|_| __mik_raw_param.to_string());
                    __mik_params.insert(#param_name.to_string(), __mik_decoded_param);
                });
            } else if !segment.is_empty() {
                checks.push(quote! {
                    __mik_segments[#i] == #segment
                });
            } else if i > 0 {
                checks.push(quote! {
                    __mik_segments[#i].is_empty()
                });
            }
        }

        let all_checks = if checks.is_empty() {
            quote! { true }
        } else {
            quote! { #(#checks)&&* }
        };

        quote! {
            (|| -> Option<::std::collections::HashMap<String, String>> {
                let __mik_segments: Vec<&str> = __mik_path.split('/').collect();
                if __mik_segments.len() == #segment_count && #all_checks {
                    let mut __mik_params = ::std::collections::HashMap::new();
                    #(#extractions)*
                    Some(__mik_params)
                } else {
                    None
                }
            })()
        }
    }
}

// =============================================================================
// CODE GENERATION - HANDLER WRAPPERS
// =============================================================================

fn generate_input_parsing(inputs: &[TypedInput]) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut parsing = Vec::new();
    let mut args = Vec::new();

    for (i, input) in inputs.iter().enumerate() {
        let var_name = format_ident!("__mik_input_{}", i);
        let type_name = &input.type_name;

        match input.source {
            InputSource::Path => {
                parsing.push(quote! {
                    let #var_name = match <#type_name as mik_sdk::typed::FromPath>::from_params(&__mik_params) {
                        Ok(v) => v,
                        Err(e) => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
            InputSource::Body => {
                parsing.push(quote! {
                    let #var_name = match __mik_req.body() {
                        Some(bytes) => {
                            match mik_sdk::json::try_parse(bytes) {
                                Some(json) => {
                                    match <#type_name as mik_sdk::typed::FromJson>::from_json(&json) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            return handler::Response {
                                                status: 400,
                                                headers: vec![
                                                    (
                                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                                    )
                                                ],
                                                body: Some(mik_sdk::json::obj()
                                                    .set("type", mik_sdk::json::str("about:blank"))
                                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                                    .set("status", mik_sdk::json::int(400))
                                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                                    .to_bytes()),
                                            };
                                        }
                                    }
                                }
                                None => {
                                    return handler::Response {
                                        status: 400,
                                        headers: vec![
                                            (
                                                mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                                mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                            )
                                        ],
                                        body: Some(mik_sdk::json::obj()
                                            .set("type", mik_sdk::json::str("about:blank"))
                                            .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                            .set("status", mik_sdk::json::int(400))
                                            .set("detail", mik_sdk::json::str("Invalid JSON body"))
                                            .to_bytes()),
                                    };
                                }
                            }
                        }
                        None => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str("Request body required"))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
            InputSource::Query => {
                parsing.push(quote! {
                    let __mik_query_params: Vec<(String, String)> = __mik_req.path()
                        .split_once('?')
                        .map(|(_, q)| {
                            q.split('&')
                                .filter_map(|pair| {
                                    let mut parts = pair.splitn(2, '=');
                                    Some((
                                        parts.next()?.to_string(),
                                        parts.next().unwrap_or("").to_string()
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let #var_name = match <#type_name as mik_sdk::typed::FromQuery>::from_query(&__mik_query_params) {
                        Ok(v) => v,
                        Err(e) => {
                            return handler::Response {
                                status: 400,
                                headers: vec![
                                    (
                                        mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                                        mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                                    )
                                ],
                                body: Some(mik_sdk::json::obj()
                                    .set("type", mik_sdk::json::str("about:blank"))
                                    .set("title", mik_sdk::json::str(mik_sdk::constants::status_title(400)))
                                    .set("status", mik_sdk::json::int(400))
                                    .set("detail", mik_sdk::json::str(&e.to_string()))
                                    .to_bytes()),
                            };
                        }
                    };
                });
                args.push(quote! { #var_name });
            },
        }
    }

    (parsing, args)
}

fn generate_route_block(route: &RouteDef) -> TokenStream2 {
    let handler = &route.handler;
    let method_check = match route.method {
        HttpMethod::Get => quote! { mik_sdk::Method::Get },
        HttpMethod::Post => quote! { mik_sdk::Method::Post },
        HttpMethod::Put => quote! { mik_sdk::Method::Put },
        HttpMethod::Patch => quote! { mik_sdk::Method::Patch },
        HttpMethod::Delete => quote! { mik_sdk::Method::Delete },
        HttpMethod::Head => quote! { mik_sdk::Method::Head },
        HttpMethod::Options => quote! { mik_sdk::Method::Options },
    };

    let pattern_checks: Vec<TokenStream2> = route
        .patterns
        .iter()
        .map(|pattern_str| {
            let matcher = generate_pattern_matcher(pattern_str);
            quote! {
                if let Some(__mik_params) = #matcher {
                    return Some(__mik_params);
                }
            }
        })
        .collect();

    let (input_parsing, input_args) = generate_input_parsing(&route.inputs);

    // Build handler call with typed inputs + &Request
    let handler_call = if input_args.is_empty() {
        quote! { #handler(&__mik_req) }
    } else {
        quote! { #handler(#(#input_args),*, &__mik_req) }
    };

    quote! {
        if __mik_method == #method_check {
            let __mik_try_match = || -> Option<::std::collections::HashMap<String, String>> {
                #(#pattern_checks)*
                None
            };

            if let Some(__mik_params) = __mik_try_match() {
                let __mik_req = mik_sdk::Request::new(
                    __mik_method.clone(),
                    __mik_raw.path.clone(),
                    __mik_raw.headers.clone(),
                    __mik_raw.body.clone(),
                    __mik_params.clone(),
                );

                #(#input_parsing)*

                return #handler_call;
            }
        }
    }
}

// =============================================================================
// CODE GENERATION - OPENAPI
// =============================================================================

/// Generate runtime code to build an OpenAPI path entry for a route.
///
/// Returns TokenStream2 that evaluates to a String containing the method entry JSON.
fn generate_openapi_path_entry_code(route: &RouteDef) -> TokenStream2 {
    let method_name = route.method.as_str();
    let path = route.patterns.first().map(|s| s.as_str()).unwrap_or("/");

    // Build request body reference if we have a body input (static)
    let request_body_str = route
        .inputs
        .iter()
        .find(|i| matches!(i.source, InputSource::Body))
        .map(|i| {
            let type_name = i.type_name.to_string();
            format!(
                "\"requestBody\":{{\"required\":true,\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{}\"}}}}}}}}",
                type_name
            )
        })
        .unwrap_or_default();

    // Build path parameters (static - derived from URL pattern)
    let path_params: Vec<String> = extract_param_names(path)
        .into_iter()
        .map(|name| {
            format!(
                "{{\"name\":\"{}\",\"in\":\"path\",\"required\":true,\"schema\":{{\"type\":\"string\"}}}}",
                name
            )
        })
        .collect();
    let path_params_str = path_params.join(",");

    // Build response (static)
    let response_str = route.output_type.as_ref().map_or_else(
        || "\"responses\":{\"200\":{\"description\":\"Success\"}}".to_string(),
        |t| {
            format!(
                "\"responses\":{{\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{{\"$ref\":\"#/components/schemas/{}\"}}}}}}}}}}",
                t
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
            static_parts.push(format!("\"parameters\":[{}]", path_params_str));
        }
        static_parts.push(response_str);

        let static_json = format!("\"{}\":{{{}}}", method_name, static_parts.join(","));
        quote! { #static_json.to_string() }
    }
}

fn generate_openapi_json(routes: &[RouteDef]) -> TokenStream2 {
    use std::collections::HashMap;

    // Group routes by path
    let mut paths: HashMap<String, Vec<&RouteDef>> = HashMap::new();
    for route in routes {
        let path = route.patterns.first().map(|s| s.as_str()).unwrap_or("/");
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

        bindings::export!(Handler with_types_in bindings);
    };

    TokenStream::from(tokens)
}
