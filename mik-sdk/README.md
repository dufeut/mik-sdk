# mik-sdk

[![Crates.io](https://img.shields.io/crates/v/mik-sdk.svg)](https://crates.io/crates/mik-sdk)
[![Documentation](https://docs.rs/mik-sdk/badge.svg)](https://docs.rs/mik-sdk)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)

Ergonomic SDK for building WASI HTTP handlers with pure Rust.

> **Experimental** - This is version 0.0.1. The API may change between releases.

## Features

- **Type-Safe Routing** - `routes!` macro with path, query, and body extraction
- **Derive Macros** - `#[derive(Type)]`, `#[derive(Query)]`, `#[derive(Path)]`
- **Response Helpers** - `ok!`, `error!` with RFC 7807 support
- **SQL Builder** - `sql_read!`, `sql_create!` with cursor pagination
- **Minimal** - ~200KB composed component size

## Quick Start

```rust
use mik_sdk::prelude::*;

// Define typed inputs with derive macros
#[derive(Type)]
pub struct HelloResponse {
    pub greeting: String,
    pub name: String,
}

#[derive(Path)]
pub struct HelloPath {
    pub name: String,
}

#[derive(Query)]
pub struct SearchQuery {
    pub q: Option<String>,
    #[field(default = 1)]
    pub page: u32,
    #[field(default = 10, max = 100)]
    pub limit: u32,
}

// Define routes with typed inputs
routes! {
    GET "/" => home,
    GET "/hello/{name}" => hello(path: HelloPath) -> HelloResponse,
    GET "/search" => search(query: SearchQuery),
}

fn home(_req: &Request) -> Response {
    ok!({ "message": "Welcome!" })
}

fn hello(path: HelloPath, _req: &Request) -> Response {
    ok!({
        "greeting": str(format!("Hello, {}!", path.name)),
        "name": str(&path.name)
    })
}

fn search(query: SearchQuery, _req: &Request) -> Response {
    ok!({
        "query": query.q.as_ref().map(json::str).unwrap_or(json::null()),
        "page": int(query.page),
        "limit": int(query.limit)
    })
}
```

## Core Macros

### Response Macros

```rust
ok!({ "data": value })                    // 200 OK with JSON
error! { status: 404, title: "Not Found" } // RFC 7807 error
created!("/users/123", { "id": "123" })   // 201 Created with Location
no_content!()                              // 204 No Content
```

### Routing

```rust
routes! {
    GET "/" => home,
    GET "/users/{id}" => get_user(path: Id) -> User,
    POST "/users" => create_user(body: CreateInput) -> User,
    GET "/search" => search(query: SearchQuery),
}
```

### DX Macros

```rust
guard!(!name.is_empty(), 400, "Name required");      // Early return validation
let user = ensure!(find_user(id), 404, "Not found"); // Unwrap or return error
```

For JSON body parsing, use typed inputs with `#[derive(Type)]` - the body is parsed automatically in the route handler.

### SQL Builder

```rust
let (sql, params) = sql_read!(users {
    select: [id, name, email],
    filter: { active: true },
    order: [-created_at, id],
    after: cursor,
    limit: 20,
});
```

## Request Helpers

```rust
req.param("id")          // Path parameter: Option<&str>
req.query("page")        // Query parameter: Option<&str>
req.header("auth")       // Header (case-insensitive): Option<&str>
req.body()               // Raw body: Option<&[u8]>
req.text()               // Body as UTF-8: Option<&str>
req.is_json()            // Content-Type is JSON: bool
req.is_html()            // Content-Type is HTML: bool
req.is_form()            // Content-Type is form: bool
req.accepts("json")      // Accept header check: bool
```

## Type Hints

Use in `ok!`, `json!`, and `error!` macros:

- `str(expr)` - JSON string
- `int(expr)` - JSON integer
- `float(expr)` - JSON float
- `bool(expr)` - JSON boolean

## API Reference

### Modules

| Module | Purpose |
|--------|---------|
| `json` | JSON building and lazy parsing |
| `time` | UTC timestamps and ISO 8601 |
| `random` | UUIDs, tokens, random bytes |
| `log` | Structured logging to stderr |
| `env` | Environment variable access |
| `status` | HTTP status code constants |

### Response Macros

| Macro | Status | Description |
|-------|--------|-------------|
| `ok!({ ... })` | 200 | JSON response |
| `created!(loc, { ... })` | 201 | With Location header |
| `accepted!()` | 202 | Accepted |
| `no_content!()` | 204 | No Content |
| `redirect!(url)` | 302 | Redirect |
| `bad_request!(msg)` | 400 | Bad Request |
| `forbidden!(msg)` | 403 | Forbidden |
| `not_found!(msg)` | 404 | Not Found |
| `conflict!(msg)` | 409 | Conflict |
| `error! { ... }` | any | RFC 7807 |

### DX Macros

| Macro | Purpose |
|-------|---------|
| `guard!(cond, status, msg)` | Early return if false |
| `ensure!(expr, status, msg)` | Unwrap or return error |
| `fetch!(METHOD url, ...)` | HTTP client request |
| `ids!(collection)` | Extract IDs for batching |

### SQL Macros

| Macro | Purpose |
|-------|---------|
| `sql_read!(table { ... })` | SELECT |
| `sql_create!(table { ... })` | INSERT |
| `sql_update!(table { ... })` | UPDATE |
| `sql_delete!(table { ... })` | DELETE |

### time Module

| Function | Returns |
|----------|---------|
| `time::now()` | `u64` - Unix seconds |
| `time::now_millis()` | `u64` - Unix milliseconds |
| `time::now_iso()` | `String` - ISO 8601 |

### random Module

| Function | Returns |
|----------|---------|
| `random::uuid()` | `String` - UUID v4 |
| `random::hex(n)` | `String` - n bytes as hex |
| `random::bytes(n)` | `Vec<u8>` - n random bytes |
| `random::u64()` | `u64` - Random integer |

### Request Methods

| Method | Returns |
|--------|---------|
| `param(name)` | `Option<&str>` |
| `query(name)` | `Option<&str>` |
| `query_all(name)` | `&[String]` |
| `header(name)` | `Option<&str>` |
| `header_all(name)` | `Vec<&str>` |
| `trace_id()` | `Option<&str>` |
| `body()` | `Option<&[u8]>` |
| `text()` | `Option<&str>` |
| `json_with(parser)` | `Option<T>` |
| `form(name)` | `Option<&str>` |
| `form_all(name)` | `&[String]` |
| `is_json()` | `bool` |
| `is_form()` | `bool` |
| `is_html()` | `bool` |
| `accepts(mime)` | `bool` |
| `has_body()` | `bool` |
| `content_type()` | `Option<&str>` |

### Logging

```rust
// Format-string style
log::info!("User {} logged in", id);
log::warn!("Cache miss: {}", key);
log::error!("Failed: {}", err);
log::debug!("Debug: {:?}", data);  // Compiled out in release

// Structured style (JSON output)
log!(info, "user created", id: user_id, email: &email);
```

## Requirements

- Rust 1.85+ (Edition 2024)
- Target: `wasm32-wasip2`
- Build tool: `cargo-component`

## License

Licensed under MIT license. See [LICENSE-MIT](LICENSE-MIT).
