# mik-sdk-macros

[![Crates.io](https://img.shields.io/crates/v/mik-sdk-macros.svg)](https://crates.io/crates/mik-sdk-macros)
[![Documentation](https://docs.rs/mik-sdk-macros/badge.svg)](https://docs.rs/mik-sdk-macros)

Procedural macros for [mik-sdk](https://crates.io/crates/mik-sdk).

## Usage

This crate is an implementation detail of `mik-sdk`. You should depend on `mik-sdk` directly:

```toml
[dependencies]
mik-sdk = "0.1"
```

## Macros Provided

| Macro | Purpose |
|-------|---------|
| `routes!` | Type-safe HTTP routing with path, query, body extraction |
| `ok!` | JSON response (200 OK) |
| `error!` | RFC 7807 error response |
| `created!` | 201 Created with Location header |
| `redirect!` | 302 redirect |
| `guard!` | Early return validation |
| `ensure!` | Unwrap Option/Result or return error |
| `fetch!` | HTTP client request builder |
| `json!` | JSON value builder |
| `log!` | Structured logging |
| `#[derive(Type)]` | JSON body/response with validation |
| `#[derive(Query)]` | Query string parameters |
| `#[derive(Path)]` | URL path parameters |

See the [mik-sdk documentation](https://docs.rs/mik-sdk) for usage examples.

## License

Licensed under MIT license.
