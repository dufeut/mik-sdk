// =============================================================================
// CRATE-LEVEL QUALITY LINTS
// =============================================================================
#![forbid(unsafe_code)]
#![deny(unused_must_use)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]
#![warn(unreachable_pub)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
// =============================================================================
// CLIPPY CONFIGURATION
// =============================================================================
// Pedantic lints - allow stylistic ones that don't affect correctness
#![allow(clippy::doc_markdown)] // Code in docs - extensive changes needed
#![allow(clippy::must_use_candidate)] // Not all returned values need must_use
#![allow(clippy::return_self_not_must_use)] // Builder pattern returns Self by design
#![allow(clippy::cast_possible_truncation)] // Intentional in WASM context
#![allow(clippy::cast_sign_loss)] // Intentional in WASM context
#![allow(clippy::cast_possible_wrap)] // Intentional in WASM context
#![allow(clippy::unreadable_literal)] // Bit patterns don't need separators
#![allow(clippy::items_after_statements)] // Const in functions for locality
#![allow(clippy::missing_errors_doc)] // # Errors sections - doc-heavy
#![allow(clippy::missing_panics_doc)] // # Panics sections - doc-heavy
#![allow(clippy::match_same_arms)] // Intentional for clarity
#![allow(clippy::format_push_string)] // String building style
#![allow(clippy::format_collect)]
// Iterator to string style
// Internal implementation where bounds/values are known at compile time or checked
#![allow(clippy::indexing_slicing)] // Fixed-size buffers and checked lengths
#![allow(clippy::unwrap_used)] // Used after explicit checks or with known values
#![allow(clippy::expect_used)] // Used for system-level guarantees (RNG, etc.)
#![allow(clippy::double_must_use)] // Builder methods can have their own docs

//! mik-sdk - Ergonomic SDK for WASI HTTP handlers
//!
//! # Overview
//!
//! mik-sdk provides a simple, ergonomic way to build portable WASI HTTP handlers.
//! Write your handler once, run it on wasmtime or any WASI P2 compliant runtime.
//!
//! Available on [crates.io](https://crates.io/crates/mik-sdk).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  Your Handler                                           │
//! │  ┌───────────────────────────────────────────────────┐  │
//! │  │  use mik_sdk::prelude::*;                         │  │
//! │  │                                                   │  │
//! │  │  routes! {                                        │  │
//! │  │      "/" => home,                                 │  │
//! │  │      "/users/{id}" => get_user,                   │  │
//! │  │  }                                                │  │
//! │  │                                                   │  │
//! │  │  fn get_user(req: &Request) -> Response {         │  │
//! │  │      let id = req.param("id").unwrap();           │  │
//! │  │      ok!({ "id": str(id) })                       │  │
//! │  │  }                                                │  │
//! │  └───────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓ compose with
//! ┌─────────────────────────────────────────────────────────┐
//! │  Router Component (provides JSON/HTTP utilities)        │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓ compose with
//! ┌─────────────────────────────────────────────────────────┐
//! │  Bridge Component (WASI HTTP adapter)                   │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓ runs on
//! ┌─────────────────────────────────────────────────────────┐
//! │  Any WASI P2 HTTP Runtime                               │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use bindings::exports::mik::core::handler::Guest;
//! use bindings::mik::core::{http, json};
//! use mik_sdk::prelude::*;
//!
//! routes! {
//!     GET "/" => home,
//!     GET "/hello/{name}" => hello(path: HelloPath),
//! }
//!
//! fn home(_req: &Request) -> http::Response {
//!     ok!({
//!         "message": "Welcome!",
//!         "version": "0.1.0"
//!     })
//! }
//!
//! fn hello(req: &Request) -> http::Response {
//!     let name = req.param("name").unwrap_or("world");
//!     ok!({
//!         "greeting": str(format!("Hello, {}!", name))
//!     })
//! }
//! ```
//!
//! # Configuration
//!
//! The SDK and bridge can be configured via environment variables:
//!
//! | Variable             | Default | Description                          |
//! |----------------------|---------|--------------------------------------|
//! | `MIK_MAX_JSON_SIZE`  | 1 MB    | Maximum JSON input size for parsing  |
//! | `MIK_MAX_BODY_SIZE`  | 10 MB   | Maximum request body size (bridge)   |
//!
//! ```bash
//! # Allow 5MB JSON payloads
//! MIK_MAX_JSON_SIZE=5000000
//!
//! # Allow 50MB request bodies
//! MIK_MAX_BODY_SIZE=52428800
//! ```
//!
//! # Core Macros
//!
//! - [`ok!`] - Return 200 OK with JSON body
//! - [`error!`] - Return RFC 7807 error response
//! - [`json!`] - Create a JSON value with type hints
//!
//! # DX Macros
//!
//! - [`guard!`] - Early return validation
//! - [`created!`] - 201 Created response with Location header
//! - [`no_content!`] - 204 No Content response
//! - [`redirect!`] - Redirect responses (301, 302, 307, etc.)
//!
//! # Request Helpers
//!
//! ```ignore
//! // Path parameters (from route pattern)
//! let id = req.param("id");              // Option<&str>
//!
//! // Query parameters
//! let page = req.query("page");          // Option<&str> - first value
//! let tags = req.query_all("tag");       // &[String] - all values
//!
//! // Example: /search?tag=rust&tag=wasm&tag=http
//! req.query("tag")      // → Some("rust")
//! req.query_all("tag")  // → &["rust", "wasm", "http"]
//!
//! // Headers (case-insensitive)
//! let auth = req.header("Authorization");    // Option<&str>
//! let cookies = req.header_all("Set-Cookie"); // &[String]
//!
//! // Body
//! let bytes = req.body();                // Option<&[u8]>
//! let text = req.text();                 // Option<&str>
//! let json = req.json_with(json::try_parse); // Option<JsonValue>
//! ```
//!
//! # DX Macro Examples
//!
//! ```ignore
//! // Early return validation
//! fn create_user(req: &Request) -> http::Response {
//!     let name = body.get("name").str_or("");
//!     guard!(!name.is_empty(), 400, "Name is required");
//!     guard!(name.len() <= 100, 400, "Name too long");
//!     created!("/users/123", { "id": "123", "name": str(name) })
//! }
//!
//! // Response shortcuts
//! fn delete_user(req: &Request) -> http::Response {
//!     no_content!()
//! }
//!
//! fn legacy_endpoint(req: &Request) -> http::Response {
//!     redirect!("/api/v2/users")  // 302 Found
//! }
//! ```
//!
//! # Type Hints
//!
//! Use type hints inside `ok!`, `json!`, and `error!` macros:
//! - `str(expr)` - Convert to JSON string
//! - `int(expr)` - Convert to JSON integer
//! - `float(expr)` - Convert to JSON float
//! - `bool(expr)` - Convert to JSON boolean
//!
//! # RFC 7807 Problem Details
//!
//! Error responses follow [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807.html):
//!
//! ```ignore
//! // Basic usage (only status is required)
//! error! { status: 400, title: "Bad Request", detail: "Missing field" }
//!
//! // Full RFC 7807 with extensions
//! error! {
//!     status: status::UNPROCESSABLE_ENTITY,
//!     title: "Validation Error",
//!     detail: "Invalid input",
//!     problem_type: "urn:problem:validation",
//!     instance: "/users/123",
//!     meta: { "field": "email" }
//! }
//! ```

pub mod constants;
mod request;
pub mod typed;

pub mod env;
pub mod http_client;
pub mod json;
pub mod log;
pub mod random;
pub mod time;

// WASI bindings (HTTP, random, clocks)
// Always included for WASM target, uses http-client feature for HTTP client on native
#[cfg(any(target_arch = "wasm32", feature = "http-client"))]
pub(crate) mod wasi_http;

// Query module - re-export from mik-sql when the sql feature is enabled
#[cfg(feature = "sql")]
pub use mik_sql as query;

pub use mik_sdk_macros::{
    // Derive macros for typed inputs
    Path,
    Query,
    Type,
    // Response macros
    accepted,
    bad_request,
    conflict,
    created,
    // DX macros
    ensure,
    // Core macros
    error,
    // HTTP client macro
    fetch,
    forbidden,
    guard,
    // Batched loading helper
    ids,
    json,
    no_content,
    not_found,
    ok,
    redirect,
    // Routing macros
    routes,
};

// SQL CRUD macros - re-exported from mik-sql-macros when sql feature is enabled
#[cfg(feature = "sql")]
pub use mik_sql_macros::{sql_create, sql_delete, sql_read, sql_update};

/// Helper trait for the `ensure!` macro to work with both Option and Result.
/// This is an implementation detail and should not be used directly.
#[doc(hidden)]
pub trait EnsureHelper<T> {
    fn into_option(self) -> Option<T>;
}

impl<T> EnsureHelper<T> for Option<T> {
    #[inline]
    fn into_option(self) -> Self {
        self
    }
}

impl<T, E> EnsureHelper<T> for Result<T, E> {
    #[inline]
    fn into_option(self) -> Option<T> {
        self.ok()
    }
}

/// Helper function for the `ensure!` macro.
/// This is an implementation detail and should not be used directly.
#[doc(hidden)]
#[inline]
pub fn __ensure_helper<T, H: EnsureHelper<T>>(value: H) -> Option<T> {
    value.into_option()
}

pub use request::{DecodeError, Method, Request, url_decode};

/// HTTP status code constants.
///
/// Use these instead of hardcoding status codes:
/// ```ignore
/// error! { status: status::NOT_FOUND, title: "Not Found", detail: "Resource not found" }
/// ```
pub mod status {
    // 2xx Success
    /// 200 OK - Request succeeded.
    pub const OK: u16 = 200;
    /// 201 Created - Resource created successfully.
    pub const CREATED: u16 = 201;
    /// 202 Accepted - Request accepted for processing.
    pub const ACCEPTED: u16 = 202;
    /// 204 No Content - Success with no response body.
    pub const NO_CONTENT: u16 = 204;

    // 3xx Redirection
    /// 301 Moved Permanently - Resource moved permanently.
    pub const MOVED_PERMANENTLY: u16 = 301;
    /// 302 Found - Resource temporarily at different URI.
    pub const FOUND: u16 = 302;
    /// 304 Not Modified - Resource not modified since last request.
    pub const NOT_MODIFIED: u16 = 304;
    /// 307 Temporary Redirect - Temporary redirect preserving method.
    pub const TEMPORARY_REDIRECT: u16 = 307;
    /// 308 Permanent Redirect - Permanent redirect preserving method.
    pub const PERMANENT_REDIRECT: u16 = 308;

    // 4xx Client Errors
    /// 400 Bad Request - Invalid request syntax or parameters.
    pub const BAD_REQUEST: u16 = 400;
    /// 401 Unauthorized - Authentication required.
    pub const UNAUTHORIZED: u16 = 401;
    /// 403 Forbidden - Access denied.
    pub const FORBIDDEN: u16 = 403;
    /// 404 Not Found - Resource not found.
    pub const NOT_FOUND: u16 = 404;
    /// 405 Method Not Allowed - HTTP method not supported.
    pub const METHOD_NOT_ALLOWED: u16 = 405;
    /// 406 Not Acceptable - Cannot produce acceptable response.
    pub const NOT_ACCEPTABLE: u16 = 406;
    /// 409 Conflict - Request conflicts with current state.
    pub const CONFLICT: u16 = 409;
    /// 410 Gone - Resource permanently removed.
    pub const GONE: u16 = 410;
    /// 422 Unprocessable Entity - Validation failed.
    pub const UNPROCESSABLE_ENTITY: u16 = 422;
    /// 429 Too Many Requests - Rate limit exceeded.
    pub const TOO_MANY_REQUESTS: u16 = 429;

    // 5xx Server Errors
    /// 500 Internal Server Error - Unexpected server error.
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    /// 501 Not Implemented - Feature not implemented.
    pub const NOT_IMPLEMENTED: u16 = 501;
    /// 502 Bad Gateway - Invalid upstream response.
    pub const BAD_GATEWAY: u16 = 502;
    /// 503 Service Unavailable - Server temporarily unavailable.
    pub const SERVICE_UNAVAILABLE: u16 = 503;
    /// 504 Gateway Timeout - Upstream server timeout.
    pub const GATEWAY_TIMEOUT: u16 = 504;
}

/// Prelude module for convenient imports.
///
/// # Usage
///
/// ```ignore
/// use mik_sdk::prelude::*;
/// ```
///
/// This imports:
/// - [`Request`] - HTTP request wrapper with convenient accessors
/// - [`Method`] - HTTP method enum (Get, Post, Put, etc.)
/// - [`status`] - HTTP status code constants
/// - [`mod@env`] - Environment variable access helpers
/// - [`http_client`] - HTTP client for outbound requests
/// - Core macros: [`ok!`], [`error!`], [`json!`], [`routes!`], [`log!`]
/// - DX macros: [`guard!`],
///   [`created!`], [`no_content!`], [`redirect!`], [`not_found!`],
///   [`conflict!`], [`forbidden!`], [`ensure!`], [`fetch!`]
pub mod prelude {
    pub use crate::env;
    pub use crate::http_client;
    pub use crate::json;
    pub use crate::json::ToJson;
    pub use crate::log;
    pub use crate::random;
    pub use crate::request::{DecodeError, Method, Request};
    pub use crate::status;
    pub use crate::time;
    // Typed input types
    pub use crate::typed::{
        FromJson, FromPath, FromQuery, Id, OpenApiSchema, ParseError, Validate, ValidationError,
    };
    // Core macros (json module already exported above)
    pub use crate::{error, ok, routes};
    // Derive macros for typed inputs
    pub use crate::{Path, Query, Type};
    // DX macros
    pub use crate::{
        accepted, bad_request, conflict, created, ensure, fetch, forbidden, guard, no_content,
        not_found, redirect,
    };

    // SQL macros and types - only when sql feature is enabled
    #[cfg(feature = "sql")]
    pub use crate::query::{Cursor, PageInfo, Value};
    #[cfg(feature = "sql")]
    pub use crate::{sql_create, sql_delete, sql_read, sql_update};
}

// ============================================================================
// API Contract Tests (compile-time assertions)
// ============================================================================

#[cfg(test)]
mod api_contracts {
    use static_assertions::{assert_impl_all, assert_not_impl_any};

    // ========================================================================
    // Request types
    // ========================================================================

    // Request is Debug but not Clone (body shouldn't be cloned)
    assert_impl_all!(crate::Request: std::fmt::Debug);
    assert_not_impl_any!(crate::Request: Clone);

    // Method is Copy, Clone, Debug, PartialEq, Eq, Hash
    assert_impl_all!(crate::Method: Copy, Clone, std::fmt::Debug, PartialEq, Eq, std::hash::Hash);

    // Id is Clone, Debug, PartialEq, Eq, Hash (can be map key)
    assert_impl_all!(crate::typed::Id: Clone, std::fmt::Debug, PartialEq, Eq, std::hash::Hash);

    // ========================================================================
    // JSON types
    // ========================================================================

    // JsonValue is Clone and Debug
    assert_impl_all!(crate::json::JsonValue: Clone, std::fmt::Debug);

    // JsonValue is NOT Send/Sync (uses Rc internally for WASM optimization)
    assert_not_impl_any!(crate::json::JsonValue: Send, Sync);

    // ========================================================================
    // Error types
    // ========================================================================

    // ParseError is Clone, Debug, PartialEq, Eq
    assert_impl_all!(crate::typed::ParseError: Clone, std::fmt::Debug, PartialEq, Eq);

    // ValidationError is Clone, Debug, PartialEq, Eq
    assert_impl_all!(crate::typed::ValidationError: Clone, std::fmt::Debug, PartialEq, Eq);

    // DecodeError is Copy, Clone, Debug, PartialEq, Eq
    assert_impl_all!(crate::DecodeError: Copy, Clone, std::fmt::Debug, PartialEq, Eq);

    // ========================================================================
    // HTTP Client types (when http-client feature is enabled)
    // ========================================================================

    #[cfg(feature = "http-client")]
    mod http_client_contracts {
        use static_assertions::assert_impl_all;

        // ClientRequest is Debug and Clone
        assert_impl_all!(crate::http_client::ClientRequest: Clone, std::fmt::Debug);

        // Response is Debug and Clone
        assert_impl_all!(crate::http_client::Response: Clone, std::fmt::Debug);

        // Error is Clone, Debug, PartialEq, Eq
        assert_impl_all!(crate::http_client::Error: Clone, std::fmt::Debug, PartialEq, Eq);
    }
}
