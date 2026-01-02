//! HTTP client macro: fetch!.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Result, Token, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

use crate::errors::{did_you_mean, duplicate_field_error};
use crate::json::{JsonValue, json_value_to_tokens};

/// Valid HTTP methods for fetch! macro.
const VALID_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

/// Valid options for fetch! macro.
const VALID_OPTIONS: &[&str] = &["headers", "json", "body", "timeout"];

// =============================================================================
// HTTP Client Macro
// =============================================================================

/// Build an HTTP client request with a clean syntax.
///
/// Creates a `http_client::ClientRequest` that can be sent using `send_with()`.
/// Supports all HTTP methods, headers, JSON body, raw body, and timeout.
///
/// # Basic Usage
///
/// ```ignore
/// use bindings::wasi::http::outgoing_handler;
/// use mik_sdk::http_client;
///
/// // Simple GET request
/// let response = fetch!(GET "https://api.example.com/users")
///     .send_with(&outgoing_handler::handle)?;
///
/// // POST with JSON body (uses json! macro syntax)
/// let response = fetch!(POST "https://api.example.com/users", json: {
///     "name": "Alice",
///     "email": "alice@example.com"
/// }).send_with(&outgoing_handler::handle)?;
/// ```
///
/// # With Headers
///
/// ```ignore
/// let response = fetch!(GET "https://api.example.com/protected",
///     headers: {
///         "Authorization": "Bearer token123",
///         "Accept": "application/json"
///     }
/// ).send_with(&outgoing_handler::handle)?;
/// ```
///
/// # With Dynamic Values
///
/// ```ignore
/// let user_id = "123";
/// let token = get_auth_token();
///
/// let response = fetch!(GET format!("https://api.example.com/users/{}", user_id),
///     headers: {
///         "Authorization": format!("Bearer {}", token)
///     }
/// ).send_with(&outgoing_handler::handle)?;
/// ```
///
/// # All Options
///
/// ```ignore
/// let response = fetch!(POST "https://api.example.com/upload",
///     headers: {
///         "Authorization": "Bearer token",
///         "X-Custom": "value"
///     },
///     body: file_bytes,          // Raw bytes
///     timeout: 30000             // Milliseconds
/// ).send_with(&outgoing_handler::handle)?;
///
/// // Or with JSON body
/// let response = fetch!(PUT "https://api.example.com/users/123",
///     headers: { "Authorization": format!("Bearer {}", token) },
///     json: {
///         "name": "Updated Name",
///         "status": "active"
///     },
///     timeout: 5000
/// ).send_with(&outgoing_handler::handle)?;
/// ```
///
/// # Supported Methods
///
/// - `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`
///
/// # Options
///
/// - `headers: { "Name": "value", ... }` - Request headers (string keys and values)
/// - `json: { ... }` - JSON body using json! macro syntax (sets Content-Type)
/// - `body: expr` - Raw body bytes (`&[u8]` or `Vec<u8>`)
/// - `timeout: ms` - Request timeout in milliseconds
struct FetchInput {
    method: syn::Ident,
    url: Expr,
    headers: Option<Vec<(Expr, Expr)>>,
    json_body: Option<JsonValue>,
    raw_body: Option<Expr>,
    timeout_ms: Option<Expr>,
}

impl Parse for FetchInput {
    // HTTP client DSL parsing with method, URL, headers, body, timeout
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        // Parse HTTP method (GET, POST, etc.)
        let method: syn::Ident = input.parse().map_err(|_| {
            syn::Error::new(
                input.span(),
                "Expected HTTP method: GET, POST, PUT, DELETE, PATCH, HEAD, or OPTIONS",
            )
        })?;

        // Validate method
        let method_str = method.to_string().to_uppercase();
        if !VALID_METHODS.contains(&method_str.as_str()) {
            let suggestion = did_you_mean(&method_str, VALID_METHODS);
            return Err(syn::Error::new_spanned(
                &method,
                format!(
                    "Unknown HTTP method '{method}'.{suggestion}\n\
                     \n\
                     Valid methods: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS\n\
                     \n\
                     Example:\n\
                     fetch!(GET \"https://api.example.com/users\")\n\
                     fetch!(POST \"https://api.example.com/users\", json: {{ \"name\": \"Alice\" }})"
                ),
            ));
        }

        // Parse URL expression
        let url: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected URL after HTTP method '{method_str}'.\n\
                     \n\
                     The URL can be:\n\
                     - A string literal: \"https://api.example.com/users\"\n\
                     - A format! expression: format!(\"https://api.example.com/users/{{}}\", id)\n\
                     - A variable: api_url\n\
                     \n\
                     Example:\n\
                     fetch!({method_str} \"https://api.example.com/users\")\n\
                     fetch!({method_str} format!(\"https://api.example.com/users/{{}}\", user_id))\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let mut headers = None;
        let mut json_body = None;
        let mut raw_body = None;
        let mut timeout_ms = None;

        // Parse optional keyword arguments
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            if input.is_empty() {
                break;
            }

            let key: syn::Ident = input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Expected option name after comma.\n\
                         \n\
                         Valid options:\n\
                         - headers: {{ \"Name\": \"value\" }}  - Request headers\n\
                         - json: {{ \"key\": value }}         - JSON body (sets Content-Type)\n\
                         - body: expression                   - Raw body bytes\n\
                         - timeout: milliseconds              - Request timeout\n\
                         \n\
                         Original error: {e}"
                    ),
                )
            })?;

            input.parse::<Token![:]>().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Expected ':' after option name '{key}'.\n\
                         \n\
                         Correct syntax: {key}: value\n\
                         \n\
                         Original error: {e}"
                    ),
                )
            })?;

            match key.to_string().as_str() {
                "headers" => {
                    if headers.is_some() {
                        return Err(duplicate_field_error(key.span(), "headers"));
                    }
                    // Parse headers: { "Name": "value", ... }
                    let content;
                    braced!(content in input);
                    let pairs: Punctuated<HeaderPair, Token![,]> = content
                        .parse_terminated(HeaderPair::parse, Token![,])
                        .map_err(|e| {
                            syn::Error::new(
                                e.span(),
                                format!(
                                    "Invalid headers syntax.\n\
                                     \n\
                                     Expected: headers: {{ \"Name\": \"value\", ... }}\n\
                                     \n\
                                     Example:\n\
                                     headers: {{\n\
                                         \"Content-Type\": \"application/json\",\n\
                                         \"Authorization\": format!(\"Bearer {{}}\", token)\n\
                                     }}\n\
                                     \n\
                                     Original error: {e}"
                                ),
                            )
                        })?;
                    headers = Some(pairs.into_iter().map(|p| (p.key, p.value)).collect());
                },
                "json" => {
                    if json_body.is_some() {
                        return Err(duplicate_field_error(key.span(), "json"));
                    }
                    // Parse JSON body using JsonValue parser
                    json_body = Some(input.parse::<JsonValue>().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid JSON body syntax.\n\
                                 \n\
                                 Expected: json: {{ \"key\": value, ... }}\n\
                                 \n\
                                 Example:\n\
                                 json: {{\n\
                                     \"name\": \"Alice\",\n\
                                     \"email\": str(user_email)\n\
                                 }}\n\
                                 \n\
                                 Original error: {e}"
                            ),
                        )
                    })?);
                },
                "body" => {
                    if raw_body.is_some() {
                        return Err(duplicate_field_error(key.span(), "body"));
                    }
                    // Parse raw body expression
                    raw_body = Some(input.parse::<Expr>().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid body expression.\n\
                                 \n\
                                 The body should be an expression that evaluates to bytes:\n\
                                 - body: file_bytes\n\
                                 - body: data.as_bytes()\n\
                                 - body: &[1, 2, 3]\n\
                                 \n\
                                 Original error: {e}"
                            ),
                        )
                    })?);
                },
                "timeout" => {
                    if timeout_ms.is_some() {
                        return Err(duplicate_field_error(key.span(), "timeout"));
                    }
                    // Parse timeout in milliseconds
                    timeout_ms = Some(input.parse::<Expr>().map_err(|e| {
                        syn::Error::new(
                            e.span(),
                            format!(
                                "Invalid timeout value.\n\
                                 \n\
                                 Expected: timeout: <milliseconds>\n\
                                 \n\
                                 Example:\n\
                                 timeout: 5000      // 5 seconds\n\
                                 timeout: 30_000    // 30 seconds\n\
                                 \n\
                                 Original error: {e}"
                            ),
                        )
                    })?);
                },
                other => {
                    let suggestion = did_you_mean(other, VALID_OPTIONS);
                    return Err(syn::Error::new_spanned(
                        &key,
                        format!(
                            "Unknown option '{other}'.{suggestion}\n\
                             \n\
                             Valid options:\n\
                             - headers: {{ \"Name\": \"value\" }}  - Request headers\n\
                             - json: {{ \"key\": value }}         - JSON body (sets Content-Type)\n\
                             - body: expression                   - Raw body bytes\n\
                             - timeout: milliseconds              - Request timeout\n\
                             \n\
                             Example:\n\
                             fetch!(POST \"https://api.example.com\",\n\
                                 headers: {{ \"Authorization\": \"Bearer token\" }},\n\
                                 json: {{ \"name\": \"Alice\" }},\n\
                                 timeout: 5000\n\
                             )"
                        ),
                    ));
                },
            }
        }

        // Validate: can't have both json and body
        if json_body.is_some() && raw_body.is_some() {
            return Err(syn::Error::new(
                input.span(),
                "Cannot specify both 'json' and 'body' options.\n\
                 \n\
                 Use 'json' for JSON data (automatically sets Content-Type: application/json):\n\
                 fetch!(POST url, json: {{ \"key\": \"value\" }})\n\
                 \n\
                 Use 'body' for raw bytes:\n\
                 fetch!(POST url, body: raw_bytes)",
            ));
        }

        Ok(Self {
            method,
            url,
            headers,
            json_body,
            raw_body,
            timeout_ms,
        })
    }
}

/// Header pair for fetch! macro
struct HeaderPair {
    key: Expr,
    value: Expr,
}

impl Parse for HeaderPair {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Invalid header name.\n\
                     \n\
                     Headers should be string expressions:\n\
                     headers: {{\n\
                         \"Content-Type\": \"application/json\",\n\
                         \"Authorization\": format!(\"Bearer {{}}\", token)\n\
                     }}\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        input.parse::<Token![:]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected ':' between header name and value.\n\
                     \n\
                     Correct syntax: \"Header-Name\": \"value\"\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let value: Expr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Invalid header value.\n\
                     \n\
                     Header values should be string expressions:\n\
                     headers: {{\n\
                         \"Content-Type\": \"application/json\",\n\
                         \"X-Custom\": my_variable\n\
                     }}\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        Ok(Self { key, value })
    }
}

pub fn fetch_impl(input: TokenStream) -> TokenStream {
    let FetchInput {
        method,
        url,
        headers,
        json_body,
        raw_body,
        timeout_ms,
    } = parse_macro_input!(input as FetchInput);

    // Map method identifier to http_client::Method
    let method_str = method.to_string().to_uppercase();
    let method_variant = match method_str.as_str() {
        "GET" => quote! { ::mik_sdk::http_client::Method::Get },
        "POST" => quote! { ::mik_sdk::http_client::Method::Post },
        "PUT" => quote! { ::mik_sdk::http_client::Method::Put },
        "DELETE" => quote! { ::mik_sdk::http_client::Method::Delete },
        "PATCH" => quote! { ::mik_sdk::http_client::Method::Patch },
        "HEAD" => quote! { ::mik_sdk::http_client::Method::Head },
        "OPTIONS" => quote! { ::mik_sdk::http_client::Method::Options },
        _ => unreachable!(), // Already validated
    };

    // Build header chain
    let header_chain = headers.map_or_else(
        || quote! {},
        |pairs| {
            let header_calls: Vec<_> = pairs
                .into_iter()
                .map(|(k, v)| {
                    quote! { .header(&#k, &#v) }
                })
                .collect();
            quote! { #(#header_calls)* }
        },
    );

    // Build body chain
    let body_chain = json_body.map_or_else(
        || raw_body.map_or_else(|| quote! {}, |raw| quote! { .body(#raw) }),
        |json_val| {
            let json_expr = json_value_to_tokens(&json_val);
            quote! { .json(&#json_expr.to_bytes()) }
        },
    );

    // Build timeout chain
    let timeout_chain =
        timeout_ms.map_or_else(|| quote! {}, |ms| quote! { .timeout_ms(#ms as u64) });

    let tokens = quote! {
        {
            ::mik_sdk::http_client::ClientRequest::new(#method_variant, &#url)
                #header_chain
                #body_chain
                #timeout_chain
        }
    };

    TokenStream::from(tokens)
}
