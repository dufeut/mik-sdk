//! Response macros: ok!, error!, and HTTP response shortcuts.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, LitInt, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

use crate::errors::did_you_mean;
use crate::json::{JsonValue, json_value_to_tokens};

/// Valid fields for error! macro.
const VALID_ERROR_FIELDS: &[&str] = &[
    "status",
    "title",
    "detail",
    "problem_type",
    "type",
    "instance",
    "meta",
];

/// Return 200 OK with JSON body.
///
/// # Examples
///
/// ```ignore
/// // Literals work directly
/// ok!({ "message": "Hello!" })
///
/// // Use type hints for expressions
/// ok!({
///     "name": str(name),
///     "count": int(items.len())
/// })
/// ```
pub fn ok_impl(input: TokenStream) -> TokenStream {
    let value = parse_macro_input!(input as JsonValue);
    let json_tokens = json_value_to_tokens(&value);
    let tokens = quote! {
        handler::Response {
            status: 200,
            headers: vec![
                (
                    ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                    ::mik_sdk::constants::MIME_JSON.to_string()
                )
            ],
            body: Some(#json_tokens.to_bytes()),
        }
    };
    TokenStream::from(tokens)
}

/// Inner implementation for potential future refactoring.
#[allow(dead_code)]
pub fn ok_impl_inner(input: proc_macro2::TokenStream) -> TokenStream2 {
    match syn::parse2::<JsonValue>(input) {
        Ok(value) => {
            let json_tokens = json_value_to_tokens(&value);
            quote! {
                handler::Response {
                    status: 200,
                    headers: vec![
                        (
                            ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                            ::mik_sdk::constants::MIME_JSON.to_string()
                        )
                    ],
                    body: Some(#json_tokens.to_bytes()),
                }
            }
        },
        Err(e) => e.to_compile_error(),
    }
}

/// RFC 7807 Problem Details - builder pattern fields.
struct ProblemDetails {
    status: Expr,
    title: Option<Expr>,
    detail: Option<Expr>,
    problem_type: Option<Expr>,
    instance: Option<Expr>,
    meta: Option<JsonValue>,
}

/// A single field in the problem builder: `key: value`
enum ProblemFieldValue {
    Expr(Expr),
    Json(JsonValue),
}

struct ProblemField {
    name: syn::Ident,
    value: ProblemFieldValue,
}

impl Parse for ProblemField {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: syn::Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected field name in error! macro.\n\
                         \n\
                         Valid fields:\n\
                         - status: <u16> (required)\n\
                         - title: <string>\n\
                         - detail: <string>\n\
                         - problem_type (or type): <string>\n\
                         - instance: <string>\n\
                         - meta: {{ ... }} (additional fields)\n\
                         \n\
                         Example:\n\
                         error! {{\n\
                             status: 404,\n\
                             title: \"Not Found\",\n\
                             detail: \"User not found\"\n\
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
                    "Expected colon (:) after field name '{name}'.\n\
                         \n\
                         Correct syntax: {name}: value\n\
                         \n\
                         Original error: {e}"
                ),
            )
        })?;

        // For 'meta' field, parse as JsonValue (allows { ... } object syntax)
        let value = if name == "meta" {
            ProblemFieldValue::Json(input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Invalid 'meta' field value.\n\
                                 Expected: JSON object with additional fields\n\
                                 \n\
                                 Example:\n\
                                 meta: {{\n\
                                     \"field\": \"email\",\n\
                                     \"reason\": \"Invalid format\"\n\
                                 }}\n\
                                 \n\
                                 Original error: {e}"
                    ),
                )
            })?)
        } else {
            ProblemFieldValue::Expr(input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Invalid value for field '{name}'.\n\
                                 \n\
                                 Expected values:\n\
                                 - status: 404 (u16 number)\n\
                                 - title: \"Not Found\" (string literal or variable)\n\
                                 - detail: \"User not found\" (string literal or variable)\n\
                                 - problem_type: \"urn:problem:user-not-found\" (string)\n\
                                 - instance: \"/users/123\" (string)\n\
                                 \n\
                                 Original error: {e}"
                    ),
                )
            })?)
        };

        Ok(Self { name, value })
    }
}

impl Parse for ProblemDetails {
    // RFC 7807 Problem Details has many optional fields to parse
    #[allow(clippy::too_many_lines)]
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let fields: Punctuated<ProblemField, Token![,]> = input
            .parse_terminated(ProblemField::parse, Token![,])
            .map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!(
                        "Invalid field syntax in error! macro.\n\
                             \n\
                             Correct syntax:\n\
                             error! {{\n\
                                 status: 404,\n\
                                 title: \"Not Found\",\n\
                                 detail: \"User not found\"\n\
                             }}\n\
                             \n\
                             Common mistakes:\n\
                             - Use commas to separate fields\n\
                             - Use colon (:) between field name and value\n\
                             - Don't forget quotes around string values\n\
                             \n\
                             Original error: {e}"
                    ),
                )
            })?;

        let mut status: Option<Expr> = None;
        let mut title: Option<Expr> = None;
        let mut detail: Option<Expr> = None;
        let mut problem_type: Option<Expr> = None;
        let mut instance: Option<Expr> = None;
        let mut meta: Option<JsonValue> = None;

        for field in fields {
            match field.name.to_string().as_str() {
                "status" => {
                    if status.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "status",
                        ));
                    }
                    if let ProblemFieldValue::Expr(e) = field.value {
                        status = Some(e);
                    }
                },
                "title" => {
                    if title.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "title",
                        ));
                    }
                    if let ProblemFieldValue::Expr(e) = field.value {
                        title = Some(e);
                    }
                },
                "detail" => {
                    if detail.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "detail",
                        ));
                    }
                    if let ProblemFieldValue::Expr(e) = field.value {
                        detail = Some(e);
                    }
                },
                "problem_type" | "type" => {
                    if problem_type.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "problem_type/type",
                        ));
                    }
                    if let ProblemFieldValue::Expr(e) = field.value {
                        problem_type = Some(e);
                    }
                },
                "instance" => {
                    if instance.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "instance",
                        ));
                    }
                    if let ProblemFieldValue::Expr(e) = field.value {
                        instance = Some(e);
                    }
                },
                "meta" => {
                    if meta.is_some() {
                        return Err(crate::errors::duplicate_field_error(
                            field.name.span(),
                            "meta",
                        ));
                    }
                    if let ProblemFieldValue::Json(j) = field.value {
                        meta = Some(j);
                    }
                },
                other => {
                    let suggestion = did_you_mean(other, VALID_ERROR_FIELDS);
                    return Err(syn::Error::new_spanned(
                        field.name,
                        format!(
                            "Unknown field '{other}' in error! macro.{suggestion}\n\
                             \n\
                             Valid fields:\n\
                             - status: <u16> (required) - HTTP status code\n\
                             - title: <string> (optional) - Short summary\n\
                             - detail: <string> (optional) - Detailed explanation\n\
                             - problem_type: <string> (optional) - URI reference for problem type\n\
                             - type: <string> (optional) - Alias for problem_type\n\
                             - instance: <string> (optional) - URI reference for this occurrence\n\
                             - meta: {{ ... }} (optional) - Additional custom fields\n\
                             \n\
                             Example:\n\
                             error! {{\n\
                                 status: 422,\n\
                                 title: \"Validation Error\",\n\
                                 detail: \"Invalid email format\",\n\
                                 meta: {{ \"field\": \"email\" }}\n\
                             }}"
                        ),
                    ));
                },
            }
        }

        let status = status.ok_or_else(|| {
            syn::Error::new(
                input.span(),
                "Missing required 'status' field in error! macro.\n\
                     \n\
                     The error! macro requires at minimum a status code:\n\
                     error! {{ status: 404 }}\n\
                     \n\
                     Full example:\n\
                     error! {{\n\
                         status: 404,\n\
                         title: \"Not Found\",\n\
                         detail: \"User not found\"\n\
                     }}",
            )
        })?;

        Ok(Self {
            status,
            title,
            detail,
            problem_type,
            instance,
            meta,
        })
    }
}

/// Return RFC 7807 Problem Details error response.
///
/// Produces: `{"type": "...", "title": "...", "status": N, "detail": "...", ...}`
///
/// # Examples
///
/// ```ignore
/// // Basic usage
/// error! { status: 404, title: "Not Found", detail: "User not found" }
///
/// // Full RFC 7807
/// error! {
///     status: 404,
///     title: "Not Found",
///     detail: "User not found",
///     problem_type: "urn:problem:user-not-found",  // optional
///     instance: "/users/123",                       // optional
/// }
///
/// // With custom meta fields (RFC 7807 extension members)
/// error! {
///     status: 422,
///     title: "Validation Error",
///     detail: "Invalid input data",
///     meta: {
///         "field": "email",
///         "reason": "Invalid format"
///     }
/// }
/// ```
pub fn error_impl(input: TokenStream) -> TokenStream {
    let problem = parse_macro_input!(input as ProblemDetails);

    let status = &problem.status;
    let title = problem
        .title
        .as_ref()
        .map(|t| quote! { .set("title", json::str(#t)) });
    let detail = problem
        .detail
        .as_ref()
        .map(|d| quote! { .set("detail", json::str(#d)) });
    let problem_type = problem.problem_type.as_ref().map_or_else(
        || quote! { .set("type", json::str("about:blank")) },
        |t| quote! { .set("type", json::str(#t)) },
    );
    let instance = problem
        .instance
        .as_ref()
        .map(|i| quote! { .set("instance", json::str(#i)) });

    // Meta fields get merged into the response body
    let meta_fields: Vec<TokenStream2> = if let Some(JsonValue::Object(fields)) = &problem.meta {
        fields
            .iter()
            .map(|(k, v)| {
                let val = json_value_to_tokens(v);
                quote! { .set(#k, #val) }
            })
            .collect()
    } else {
        vec![]
    };

    let tokens = quote! {
        {
            let __mik_sdk_body = json::obj()
                #problem_type
                #title
                .set("status", json::int(#status as i64))
                #detail
                #instance
                #(#meta_fields)*;
            handler::Response {
                status: #status as u16,
                headers: vec![
                    (
                        ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                        ::mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                    )
                ],
                body: Some(__mik_sdk_body.to_bytes()),
            }
        }
    };

    TokenStream::from(tokens)
}

/// Create a 201 Created response with optional Location header and body.
///
/// # Examples
///
/// ```ignore
/// // With location and body
/// created!("/users/123", { "id": "123" })
///
/// // With just location
/// created!("/users/123")
/// ```
struct CreatedInput {
    location: Expr,
    body: Option<JsonValue>,
}

impl Parse for CreatedInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let location: Expr = input.parse()?;

        let body = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(Self { location, body })
    }
}

pub fn created_impl(input: TokenStream) -> TokenStream {
    let CreatedInput { location, body } = parse_macro_input!(input as CreatedInput);

    let body_expr = body.map_or_else(
        || quote! { None },
        |b| {
            let json_tokens = json_value_to_tokens(&b);
            quote! { Some(#json_tokens.to_bytes()) }
        },
    );

    let tokens = quote! {
        handler::Response {
            status: 201,
            headers: vec![
                (
                    ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                    ::mik_sdk::constants::MIME_JSON.to_string()
                ),
                ("location".to_string(), #location.to_string())
            ],
            body: #body_expr,
        }
    };

    TokenStream::from(tokens)
}

/// Create a 204 No Content response.
///
/// # Examples
///
/// ```ignore
/// fn delete_user(req: &Request) -> handler::Response {
///     // ... delete logic ...
///     no_content!()
/// }
/// ```
pub fn no_content_impl(_input: TokenStream) -> TokenStream {
    let tokens = quote! {
        handler::Response {
            status: 204,
            headers: vec![],
            body: None,
        }
    };

    TokenStream::from(tokens)
}

/// Create a redirect response (302 Found by default, or custom status).
///
/// # Examples
///
/// ```ignore
/// // 302 Found (default)
/// redirect!("/login")
///
/// // 301 Moved Permanently
/// redirect!(301, "/new-location")
///
/// // 307 Temporary Redirect
/// redirect!(307, "/maintenance")
/// ```
enum RedirectInput {
    Simple(Expr),
    WithStatus(LitInt, Expr),
}

impl Parse for RedirectInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        // Check if first token is a number (status code)
        if input.peek(LitInt) {
            let status: LitInt = input.parse()?;
            input.parse::<Token![,]>()?;
            let location: Expr = input.parse()?;
            Ok(Self::WithStatus(status, location))
        } else {
            let location: Expr = input.parse()?;
            Ok(Self::Simple(location))
        }
    }
}

pub fn redirect_impl(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as RedirectInput);

    let (status, location) = match parsed {
        RedirectInput::Simple(loc) => (quote! { 302 }, loc),
        RedirectInput::WithStatus(s, loc) => (quote! { #s }, loc),
    };

    let tokens = quote! {
        handler::Response {
            status: #status as u16,
            headers: vec![
                ("location".to_string(), #location.to_string())
            ],
            body: None,
        }
    };

    TokenStream::from(tokens)
}

// ============================================================================
// SIMPLE ERROR RESPONSE BUILDER (shared by not_found!, conflict!, etc.)
// ============================================================================

/// Configuration for simple error response macros.
struct SimpleErrorConfig {
    status: u16,
    default_detail: &'static str,
}

/// Build a simple RFC 7807 error response with configurable status and detail.
fn simple_error_response(config: SimpleErrorConfig, input: TokenStream) -> TokenStream {
    let status = config.status;
    let detail = if input.is_empty() {
        let default = config.default_detail;
        quote! { #default }
    } else {
        let expr = parse_macro_input!(input as Expr);
        quote! { #expr }
    };

    let tokens = quote! {
        {
            let __mik_sdk_body = json::obj()
                .set("type", json::str("about:blank"))
                .set("title", json::str(::mik_sdk::constants::status_title(#status)))
                .set("status", json::int(#status as i64))
                .set("detail", json::str(#detail));
            handler::Response {
                status: #status,
                headers: vec![
                    (
                        ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                        ::mik_sdk::constants::MIME_PROBLEM_JSON.to_string()
                    )
                ],
                body: Some(__mik_sdk_body.to_bytes()),
            }
        }
    };

    TokenStream::from(tokens)
}

pub fn not_found_impl(input: TokenStream) -> TokenStream {
    simple_error_response(
        SimpleErrorConfig {
            status: 404,
            default_detail: "Not Found",
        },
        input,
    )
}

/// Return 409 Conflict response.
///
/// # Examples
///
/// ```ignore
/// // Generic conflict
/// conflict!()
///
/// // With detail message
/// conflict!("User already exists")
///
/// // With dynamic message
/// conflict!(format!("Email {} is taken", email))
/// ```
pub fn conflict_impl(input: TokenStream) -> TokenStream {
    simple_error_response(
        SimpleErrorConfig {
            status: 409,
            default_detail: "Conflict",
        },
        input,
    )
}

/// Return 403 Forbidden response.
///
/// # Examples
///
/// ```ignore
/// // Generic forbidden
/// forbidden!()
///
/// // With detail message
/// forbidden!("You don't have permission to access this resource")
///
/// // With dynamic message
/// forbidden!(format!("Access denied for user {}", user_id))
/// ```
pub fn forbidden_impl(input: TokenStream) -> TokenStream {
    simple_error_response(
        SimpleErrorConfig {
            status: 403,
            default_detail: "Forbidden",
        },
        input,
    )
}

/// Return 400 Bad Request response.
///
/// # Examples
///
/// ```ignore
/// // Generic bad request
/// bad_request!()
///
/// // With detail message
/// bad_request!("Invalid input")
///
/// // With dynamic message
/// bad_request!(format!("Field {} is required", field))
/// ```
pub fn bad_request_impl(input: TokenStream) -> TokenStream {
    simple_error_response(
        SimpleErrorConfig {
            status: 400,
            default_detail: "Bad Request",
        },
        input,
    )
}

/// Return 202 Accepted response.
///
/// Used for asynchronous operations where the request has been accepted
/// but processing is not complete.
///
/// # Examples
///
/// ```ignore
/// // Simple accepted with JSON body
/// accepted!({ "status": "processing", "job_id": str(job_id) })
///
/// // Accepted with no body
/// accepted!()
/// ```
pub fn accepted_impl(input: TokenStream) -> TokenStream {
    if input.is_empty() {
        // No body - return 202 with empty response
        let tokens = quote! {
            handler::Response {
                status: 202,
                headers: vec![],
                body: None,
            }
        };
        return TokenStream::from(tokens);
    }

    // Parse JSON body
    let json_value = parse_macro_input!(input as JsonValue);
    let json_tokens = json_value_to_tokens(&json_value);

    let tokens = quote! {
        {
            let __mik_sdk_body = #json_tokens;
            handler::Response {
                status: 202,
                headers: vec![
                    (
                        ::mik_sdk::constants::HEADER_CONTENT_TYPE.to_string(),
                        ::mik_sdk::constants::MIME_JSON.to_string()
                    )
                ],
                body: Some(__mik_sdk_body.to_bytes()),
            }
        }
    };

    TokenStream::from(tokens)
}
