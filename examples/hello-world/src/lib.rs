//! Hello World - Clean DX with typed inputs and proc-macros!
//!
//! New simplified 2-component architecture:
//! - JSON/time/random are pure Rust in mik-sdk
//! - Only need bridge + handler composition
//! - Type-safe inputs with derive macros

#[allow(warnings)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use mik_sdk::prelude::*;

// ============================================================================
// TYPE DEFINITIONS - Input and Output types with derive macros
// ============================================================================

#[derive(Type)]
pub struct HomeResponse {
    pub message: String,
    pub version: String,
    pub endpoints: Vec<String>,
}

#[derive(Type)]
pub struct HelloResponse {
    pub greeting: String,
    pub name: String,
}

/// Custom path parameter - demonstrates #[derive(Path)]
#[derive(Path)]
pub struct HelloPath {
    pub name: String,
}

#[derive(Type)]
pub struct EchoInput {
    #[field(min = 1, docs = "Message to echo back")]
    pub message: String,
}

#[derive(Type)]
pub struct EchoResponse {
    pub echo: String,
    pub length: i64,
}

/// Query parameters - demonstrates #[derive(Query)]
#[derive(Query)]
pub struct SearchQuery {
    /// Search term (optional)
    pub q: Option<String>,

    /// Page number with default
    #[field(default = 1)]
    pub page: u32,

    /// Items per page with default and max
    #[field(default = 10, max = 100)]
    pub limit: u32,
}

#[derive(Type)]
pub struct SearchResponse {
    pub query: Option<String>,
    pub page: i64,
    pub limit: i64,
    pub message: String,
}

// ============================================================================
// ROUTES - Flat syntax with typed inputs
// ============================================================================

routes! {
    GET "/" | "" => home -> HomeResponse,
    GET "/hello/{name}" => hello(path: HelloPath) -> HelloResponse,
    POST "/echo" => echo(body: EchoInput) -> EchoResponse,
    GET "/search" => search(query: SearchQuery) -> SearchResponse,
}

// ============================================================================
// HANDLERS - Receive typed, parsed inputs
// ============================================================================

fn home(_req: &Request) -> Response {
    ok!({
        "message": "Welcome to mik-sdk!",
        "version": "0.1.0",
        "endpoints": ["/", "/hello/{name}", "/echo", "/search", "/__schema"]
    })
}

fn hello(path: HelloPath, _req: &Request) -> Response {
    // path.name is extracted from {name} in the route
    let greeting = format!("Hello, {}!", path.name);
    ok!({
        "greeting": greeting,
        "name": path.name
    })
}

fn echo(body: EchoInput, _req: &Request) -> Response {
    // body is already parsed and validated!
    let len = body.message.len();
    ok!({
        "echo": body.message,
        "length": len
    })
}

fn search(query: SearchQuery, _req: &Request) -> Response {
    // query.q is Option<String>, query.page defaults to 1, query.limit defaults to 10
    let message = match &query.q {
        Some(q) => format!("Searching for '{}' on page {}", q, query.page),
        None => format!("Listing all items on page {}", query.page),
    };

    ok!({
        "query": query.q,       // Option<String> -> null or string
        "page": query.page,     // u32 -> integer
        "limit": query.limit,   // u32 -> integer
        "message": message      // String -> string
    })
}
