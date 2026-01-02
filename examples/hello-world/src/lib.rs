#![allow(missing_docs)]
#![allow(clippy::exhaustive_structs)]
#![allow(unsafe_code)]

#[allow(warnings, unsafe_code)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use mik_sdk::prelude::*;

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

#[derive(Query)]
pub struct SearchQuery {
    pub q: Option<String>,
    #[field(default = 1)]
    pub page: u32,
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

routes! {
    GET "/" | "" => home -> HomeResponse,
    GET "/hello/{name}" => hello(path: HelloPath) -> HelloResponse,
    POST "/echo" => echo(body: EchoInput) -> EchoResponse,
    GET "/search" => search(query: SearchQuery) -> SearchResponse,
}

fn home(_req: &Request) -> Response {
    ok!({
        "message": "Welcome to mik-sdk!",
        "version": "0.1.0",
        "endpoints": ["/", "/hello/{name}", "/echo", "/search", "/__schema"]
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
