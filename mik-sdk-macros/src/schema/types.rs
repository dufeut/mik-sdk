//! Route types and parsing for the routes macro.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Attribute, Ident, LitStr, Result, Token,
    parse::{Parse, ParseStream},
};

use crate::constants::VALID_HTTP_METHODS;
use crate::errors::did_you_mean;

/// Valid input sources for route handlers.
const VALID_INPUT_SOURCES: &[&str] = &["path", "body", "query"];

// =============================================================================
// TYPES
// =============================================================================

#[derive(Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Patch => "patch",
            Self::Delete => "delete",
            Self::Head => "head",
            Self::Options => "options",
        }
    }

    pub(crate) fn to_method_check(&self) -> TokenStream2 {
        match self {
            Self::Get => quote! { mik_sdk::Method::Get },
            Self::Post => quote! { mik_sdk::Method::Post },
            Self::Put => quote! { mik_sdk::Method::Put },
            Self::Patch => quote! { mik_sdk::Method::Patch },
            Self::Delete => quote! { mik_sdk::Method::Delete },
            Self::Head => quote! { mik_sdk::Method::Head },
            Self::Options => quote! { mik_sdk::Method::Options },
        }
    }
}

/// Input source for typed parameters
#[derive(Clone)]
pub enum InputSource {
    Path,  // from URL path params
    Body,  // from JSON body
    Query, // from query string
}

/// A typed input parameter for a handler
#[derive(Clone)]
pub struct TypedInput {
    pub(crate) source: InputSource,
    pub(crate) type_name: Ident,
}

/// A route definition
pub struct RouteDef {
    pub(crate) method: HttpMethod,
    pub(crate) patterns: Vec<String>,
    pub(crate) handler: Ident,
    pub(crate) inputs: Vec<TypedInput>,
    pub(crate) output_type: Option<Ident>,
    /// Operation summary from doc comment
    pub(crate) summary: Option<String>,
    /// Tag override from #[tag = "..."] attribute
    pub(crate) tag_override: Option<String>,
    /// Mark operation as deprecated in OpenAPI schema
    pub(crate) deprecated: bool,
    /// HTTP status code for success response (default: 200)
    pub(crate) status_code: u16,
}

/// All routes in the macro
pub struct RoutesDef {
    pub(crate) routes: Vec<RouteDef>,
    /// Global tag for all routes (from #[tag = "..."] at top of block)
    pub(crate) default_tag: Option<String>,
}

impl RouteDef {
    /// Get the effective tag for this route.
    ///
    /// Priority: route override > global default > auto-generated from path
    pub(crate) fn effective_tag(&self, default_tag: Option<&str>) -> String {
        if let Some(ref tag) = self.tag_override {
            return tag.clone();
        }
        if let Some(tag) = default_tag {
            return tag.to_string();
        }
        // Auto-generate from first path segment
        self.patterns
            .first()
            .and_then(|p| {
                p.trim_start_matches('/')
                    .split('/')
                    .next()
                    .filter(|s| !s.is_empty() && !s.starts_with('{'))
            })
            .map_or_else(
                || "Default".to_string(),
                |s| {
                    // Capitalize first letter
                    let mut chars = s.chars();
                    chars.next().map_or_else(
                        || s.to_string(),
                        |c| c.to_uppercase().collect::<String>() + chars.as_str(),
                    )
                },
            )
    }
}

// =============================================================================
// PARSING
// =============================================================================

impl Parse for RoutesDef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut routes = Vec::new();
        let mut default_tag = None;

        // Check for global #[tag = "..."] at the start
        while input.peek(Token![#]) {
            let attrs: Vec<Attribute> = input.call(Attribute::parse_outer)?;
            for attr in attrs {
                if attr.path().is_ident("tag") {
                    let value: LitStr = attr.parse_args()?;
                    default_tag = Some(value.value());
                }
            }
        }

        while !input.is_empty() {
            let route = parse_route(input)?;
            routes.push(route);

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            routes,
            default_tag,
        })
    }
}

#[allow(clippy::too_many_lines)] // Complex route parsing with many input variants
fn parse_route(input: ParseStream<'_>) -> Result<RouteDef> {
    // Parse doc comments (/// ...) and attributes (#[tag = "..."], #[deprecated], #[status(code)]) before the route
    let mut summary = None;
    let mut tag_override = None;
    let mut deprecated = false;
    let mut status_code: u16 = 200; // Default status code

    // Parse outer attributes (doc comments become #[doc = "..."])
    let attrs: Vec<Attribute> = input.call(Attribute::parse_outer)?;
    for attr in attrs {
        if attr.path().is_ident("doc") {
            // Extract doc comment text
            if let syn::Meta::NameValue(meta) = &attr.meta
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) = &meta.value
            {
                let text = lit_str.value().trim().to_string();
                if !text.is_empty() {
                    // Append to summary (multiple /// lines get combined)
                    summary = Some(match summary {
                        Some(existing) => format!("{existing} {text}"),
                        None => text,
                    });
                }
            }
        } else if attr.path().is_ident("tag") {
            let value: LitStr = attr.parse_args()?;
            tag_override = Some(value.value());
        } else if attr.path().is_ident("deprecated") {
            deprecated = true;
        } else if attr.path().is_ident("status") {
            let code: syn::LitInt = attr.parse_args()?;
            status_code = code.base10_parse().map_err(|_| {
                syn::Error::new_spanned(
                    &code,
                    "status code must be a valid HTTP status code (100-599)",
                )
            })?;
            if !(100..=599).contains(&status_code) {
                return Err(syn::Error::new_spanned(
                    &code,
                    format!(
                        "Invalid HTTP status code: {status_code}. Must be between 100 and 599.\n\
                         \n\
                         Common status codes:\n\
                         #[status(200)] - OK (default)\n\
                         #[status(201)] - Created\n\
                         #[status(202)] - Accepted\n\
                         #[status(204)] - No Content"
                    ),
                ));
            }
        }
    }

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
        other => {
            let suggestion = did_you_mean(other, VALID_HTTP_METHODS);
            return Err(syn::Error::new_spanned(
                &method_ident,
                format!(
                    "Invalid HTTP method '{method_ident}'.{suggestion}\n\
                     \n\
                     Valid methods: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS\n\
                     \n\
                     Example: GET \"/users\" => list_users"
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
                "Expected route path (string literal) after HTTP method '{method_str}'.\n\
                 \n\
                 Correct syntax: {method_str} \"/path\" => handler\n\
                 \n\
                 Common mistakes:\n\
                 - Path must be a string literal: {method_str} \"/users\" ✓ not {method_str} /users ✗\n\
                 - Path should start with /: {method_str} \"/users\" ✓ not {method_str} \"users\" ✗\n\
                 \n\
                 Original error: {e}"
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
                     Correct syntax: {method_str} \"/path\" | \"/alt-path\" => handler\n\
                     \n\
                     Original error: {e}"
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
                patterns
                    .first()
                    .map_or("/path", std::string::String::as_str),
                method_str,
                patterns
                    .first()
                    .map_or("/path", std::string::String::as_str),
                method_str,
                patterns
                    .first()
                    .map_or("/path", std::string::String::as_str),
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
                patterns
                    .first()
                    .map_or("/path", std::string::String::as_str),
                method_str,
                patterns
                    .first()
                    .map_or("/path", std::string::String::as_str),
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
                    patterns
                        .first()
                        .map_or("/path", std::string::String::as_str),
                    handler,
                    method_str,
                    patterns
                        .first()
                        .map_or("/path", std::string::String::as_str),
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
        summary,
        tag_override,
        deprecated,
        status_code,
    })
}

fn parse_typed_inputs(
    input: ParseStream<'_>,
    method_str: &str,
    patterns: &[String],
    handler: &Ident,
) -> Result<Vec<TypedInput>> {
    let mut inputs = Vec::new();
    let path = patterns
        .first()
        .map_or("/path", std::string::String::as_str);

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
                     {method_str} \"{path}\" => {handler}(path: UserId, body: CreateUser, query: Pagination) -> User\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        let source = match source_ident.to_string().as_str() {
            "path" => InputSource::Path,
            "body" => InputSource::Body,
            "query" => InputSource::Query,
            other => {
                let suggestion = did_you_mean(other, VALID_INPUT_SOURCES);
                return Err(syn::Error::new_spanned(
                    &source_ident,
                    format!(
                        "Invalid input source '{other}'.{suggestion}\n\
                         \n\
                         Valid sources:\n\
                         - path  - URL path parameters (e.g., /users/{{id}})\n\
                         - body  - JSON request body\n\
                         - query - Query string parameters\n\
                         \n\
                         Example:\n\
                         {method_str} \"{path}\" => {handler}(path: Id, body: CreateUser) -> User"
                    ),
                ));
            },
        };

        // Parse colon
        input.parse::<Token![:]>().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected ':' after input source '{source_ident}'.\n\
                     \n\
                     Correct syntax: {source_ident}: TypeName\n\
                     \n\
                     Example:\n\
                     {method_str} \"{path}\" => {handler}({source_ident}: UserId)\n\
                     \n\
                     Original error: {e}"
                ),
            )
        })?;

        // Parse type name
        let type_name: Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!(
                    "Expected type name after '{source_ident}: '.\n\
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
                     {method_str} \"{path}\" => {handler}({source_ident}: UserId)\n\
                     \n\
                     Original error: {e}"
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
