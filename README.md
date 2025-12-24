# mik-sdk

**Portable WASI HTTP SDK using Component Composition**

Write handlers once, run on any WASI-compliant runtime.

## Overview

mik-sdk provides an ergonomic way to build portable WebAssembly HTTP handlers. It uses a two-component architecture where your handler logic is composed with a bridge component that handles WASI HTTP translation.

```rust
use mik_sdk::prelude::*;

routes! {
    GET "/" => home,
    GET "/hello/{name}" => hello(path: HelloPath),
    POST "/users" => create_user(body: CreateInput),
}

fn home(_req: &Request) -> Response {
    ok!({ "message": "Welcome to mik-sdk!" })
}

fn hello(path: HelloPath, _req: &Request) -> Response {
    ok!({ "greeting": format!("Hello, {}!", path.name) })
}
```

## Features

- **Type-safe routing** with automatic path, query, and body extraction
- **Pure Rust JSON** parsing and building (no external calls)
- **Derive macros** for input types (`#[derive(Type)]`, `#[derive(Path)]`, `#[derive(Query)]`)
- **Response helpers** (`ok!`, `error!`, `created!`, `no_content!`, `redirect!`)
- **SQL query builder** with Mongo-style filter syntax
- **Cursor pagination** for efficient database queries
- **HTTP client** with SSRF protection
- **Structured logging** compatible with major log aggregators
- **Time and random utilities** using native WASI interfaces
- **Minimal compiled size** (~200KB composed component)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Your Handler Component                                 │
│  - Uses mik_sdk::prelude::*                             │
│  - Exports mik:core/handler interface                   │
└─────────────────────────────────────────────────────────┘
                          ↓ compose with
┌─────────────────────────────────────────────────────────┐
│  Bridge Component (mik-bridge)                          │
│  - Translates WASI HTTP to mik handler                  │
│  - Handles request/response conversion                  │
└─────────────────────────────────────────────────────────┘
                          ↓ runs on
┌─────────────────────────────────────────────────────────┐
│  Any WASI HTTP Runtime                                  │
└─────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.85+ with `wasm32-wasip2` target
- [cargo-component](https://github.com/bytecodealliance/cargo-component)
- [wac](https://github.com/bytecodealliance/wac) for component composition

### Installation

Add to your `Cargo.toml`:

```toml
[package]
name = "my-handler"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
mik-sdk = "0.1"

[package.metadata.component]
package = "my:handler"
```

### Build and Compose

```bash
# Build your handler
cargo component build --release

# Compose with bridge
wac plug mik-bridge.wasm --plug target/wasm32-wasip2/release/my_handler.wasm -o service.wasm
```

## Documentation

Full documentation is available at [docs/](./docs/).

- [Installation Guide](./docs/src/content/docs/guides/installation.mdx)
- [Quick Start](./docs/src/content/docs/guides/quickstart.mdx)
- [Routing](./docs/src/content/docs/guides/routing.mdx)
- [API Reference](./docs/src/content/docs/reference/)

## Examples

See the [examples/](./examples/) directory:

- **hello-world** - Minimal handler with typed inputs
- **crud-api** - REST API with SQL query builder

## Crate Structure

| Crate            | Description                                        |
| ---------------- | -------------------------------------------------- |
| `mik-sdk`        | Main SDK with routing, JSON, time, random, logging |
| `mik-sdk-macros` | Procedural macros for routing and derive           |
| `mik-sql`        | SQL query builder with Mongo-style filters         |
| `mik-sql-macros` | SQL CRUD macros                                    |
| `mik-bridge`     | WASI HTTP adapter component                        |
| `mik-wit`        | WIT interface definitions                          |

## License

MIT
