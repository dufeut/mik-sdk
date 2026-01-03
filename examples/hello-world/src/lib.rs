#![allow(missing_docs)]
#![allow(clippy::exhaustive_structs)]
#![allow(unsafe_code)]

#[allow(warnings, unsafe_code)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use mik_sdk::prelude::*;

#[derive(Type)]
pub struct HomeResponse {
    #[field(x_example = "Welcome to mik-sdk!")]
    pub message: String,
    #[field(x_example = "0.1.0")]
    pub version: String,
    pub endpoints: Vec<String>,
}

#[derive(Type)]
pub struct HelloResponse {
    #[field(x_example = "Hello, Alice!")]
    pub greeting: String,
    #[field(x_example = "Alice")]
    pub name: String,
}

#[derive(Path)]
pub struct HelloPath {
    pub name: String,
}

#[derive(Type)]
pub struct EchoInput {
    #[field(min = 1, docs = "Message to echo back", x_example = "Hello, World!")]
    pub message: String,
}

#[derive(Type)]
pub struct EchoResponse {
    #[field(x_example = "Hello, World!")]
    pub echo: String,
    #[field(x_example = 13)]
    pub length: i64,
}

#[derive(Query)]
pub struct SearchQuery {
    #[field(x_example = "rust wasm")]
    pub q: Option<String>,
    #[field(default = 1, x_example = 1)]
    pub page: u32,
    #[field(default = 10, max = 100, x_example = 20)]
    pub limit: u32,
}

#[derive(Type)]
pub struct SearchResponse {
    #[field(x_example = "rust wasm")]
    pub query: Option<String>,
    #[field(x_example = 1)]
    pub page: i64,
    #[field(x_example = 20)]
    pub limit: i64,
    #[field(x_example = "Searching for 'rust wasm' on page 1")]
    pub message: String,
}

routes! {
    /// Welcome page with API information
    GET "/" | "" => home -> HomeResponse,

    /// Greet a user by name
    GET "/hello/{name}" => hello(path: HelloPath) -> HelloResponse,

    /// Echo back a message with its length
    POST "/echo" => echo(body: EchoInput) -> EchoResponse,

    /// Search with pagination
    GET "/search" => search(query: SearchQuery) -> SearchResponse,
}

fn home(_req: &Request) -> Response {
    ok!({
        "message": "Welcome to mik-sdk!",
        "version": "0.1.0",
        "endpoints": ["/", "/hello/{name}", "/echo", "/search"]
    })
}

fn hello(path: HelloPath, _req: &Request) -> Response {
    log!(info, "hello called", name: &path.name);
    let greeting = format!("Hello, {}!", path.name);
    ok!({
        "greeting": greeting,
        "name": path.name
    })
}

fn echo(body: EchoInput, _req: &Request) -> Response {
    let len = body.message.len();
    ok!({
        "echo": body.message,
        "length": len
    })
}

fn search(query: SearchQuery, _req: &Request) -> Response {
    let message = match &query.q {
        Some(q) => format!("Searching for \'{}\' on page {}", q, query.page),
        None => format!("Listing all items on page {}", query.page),
    };

    ok!({
        "query": query.q,
        "page": query.page,
        "limit": query.limit,
        "message": message
    })
}
