# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-12-27

Initial release of mik-sdk - a portable WASI HTTP SDK using component composition.

### Core SDK (mik-sdk)

#### Routing & Handlers
- `routes!` macro with typed inputs - path, query, and body extraction
- Derive macros: `#[derive(Type)]`, `#[derive(Query)]`, `#[derive(Path)]`
- Response macros: `ok!`, `error!` (RFC 7807), `created!`, `no_content!`, `redirect!`
- DX macros: `guard!`, `ensure!`
- Error shortcuts: `not_found!`, `conflict!`, `forbidden!`, `bad_request!`, `accepted!`

#### HTTP Client
- `fetch!` macro with headers, JSON body, timeout support
- `.send()` method using native `wasi:http/outgoing-handler`
- Trace ID propagation via `.with_trace_id()`
- SSRF protection via `.deny_private_ips()` - blocks localhost, private ranges, IPv6 loopback
- Header injection prevention (CR/LF validation)
- `http-client` feature flag (opt-in, adds ~78KB)

#### Request Helpers
- `param()`, `query()`, `query_all()` - URL parameters
- `header()`, `header_all()` - HTTP headers (case-insensitive)
- `body()`, `text()`, `json_with()` - body access
- `trace_id()` - distributed tracing support
- `is_json()`, `is_html()`, `is_form()`, `accepts()` - content negotiation

#### JSON (Pure Rust)
- `json::try_parse()` - lazy parsing, no tree built until needed
- `json::obj()`, `json::arr()` - builder pattern
- `path_str()`, `path_int()`, `path_float()`, `path_bool()` - ~100ns extraction
- `path_exists()`, `path_is_null()` - ~40ns checks
- **~33x faster** than tree traversal for 1-5 field extraction

#### Time Utilities
- Native `wasi:clocks/wall-clock` on WASM (automatic)
- `time::now()` - Unix seconds
- `time::now_millis()` - Unix milliseconds
- `time::now_iso()` - ISO 8601 string
- Howard Hinnant's algorithm for date formatting

#### Random Utilities
- Native `wasi:random/random` on WASM (automatic)
- `random::uuid()` - UUID v4 (RFC 4122)
- `random::hex(len)` - hex string
- `random::bytes(len)` - random bytes
- `random::u64()` - random integer
- Cryptographically secure on all platforms

#### Logging
- Structured JSON logging to stderr
- `log!(level, "msg", key: value)` macro
- `log::info!`, `log::warn!`, `log::error!`, `log::debug!`

#### Error Types
- `ParseError`: `MissingField`, `InvalidFormat`, `TypeMismatch`, `Custom`
- `ValidationError`: `Min`, `Max`, `Pattern`, `Format`, `Custom`
- `.field()`, `.message()`, `.with_path()` for nested context
- `#[non_exhaustive]` for forward compatibility

### SQL Query Builder (mik-sql)

- CRUD macros: `sql_read!`, `sql_create!`, `sql_update!`, `sql_delete!`
- Filter operators: `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`
- Text operators: `$like`, `$ilike`, `$starts_with`, `$ends_with`, `$contains`
- Logical operators: `$and`, `$or`, `$not`, `$between`
- Cursor-based pagination with multi-field keyset
- Offset pagination with `page` and `limit`
- Dialect support: Postgres (`$1`) and SQLite (`?1`)
- SQL injection prevention

### Bridge Component (mik-bridge)

- WASI HTTP adapter for `wasi:http/incoming-handler`
- Request/response translation
- ~72KB compiled size

### Architecture

- Two-component model: handler + bridge via WAC composition
- Handler: ~158KB (without HTTP client), ~236KB (with HTTP client)
- Total composed: ~230-308KB
- Runs on wasmtime, Spin, wasmCloud

### Security

- SSRF protection (private IP blocking)
- Header injection prevention
- SQL injection prevention
- Input validation (headers, JSON depth, body size)
- Cryptographically secure random
- RFC 7807 Problem Details errors

### Testing

- 415+ unit tests
- Property-based tests (proptest)
- Fuzz testing for JSON, request parsing, URL decoding
- ISO 8601 roundtrip validation

---

**Note:** This is v0.1.0 - an internal release. API may evolve.
