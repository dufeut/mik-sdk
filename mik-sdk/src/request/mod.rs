//! HTTP Request wrapper for `mik_sdk` handlers.
//!
//! This module provides the `Request` struct that wraps raw `request-data` from WIT
//! and provides convenient accessors for path parameters, query strings, headers, and body.

mod cookie;
mod multipart;
mod parsing;

pub use cookie::{SameSite, SetCookie};
pub use multipart::{MultipartError, Part};
use parsing::contains_ignore_ascii_case;
pub use parsing::{DecodeError, url_decode};

use crate::constants::{
    HEADER_AUTHORIZATION, HEADER_COOKIE, HEADER_TRACE_ID, MAX_FORM_FIELDS, MAX_HEADER_VALUE_LEN,
    MAX_TOTAL_HEADERS_SIZE, MAX_URL_DECODED_LEN, MIME_MULTIPART,
};
use crate::json::{self, JsonValue};
use std::cell::OnceCell;
use std::collections::HashMap;

/// HTTP method enum matching the WIT definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Method {
    /// HTTP GET method - retrieve a resource.
    Get,
    /// HTTP POST method - create a resource.
    Post,
    /// HTTP PUT method - replace a resource.
    Put,
    /// HTTP PATCH method - partially update a resource.
    Patch,
    /// HTTP DELETE method - remove a resource.
    Delete,
    /// HTTP HEAD method - retrieve headers only.
    Head,
    /// HTTP OPTIONS method - retrieve allowed methods.
    Options,
}

impl Method {
    /// Returns the method as an uppercase string (e.g., "GET", "POST").
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// HTTP Request wrapper providing convenient access to request data.
///
/// Created by the `routes!` macro from raw `request-data`. Provides:
/// - Path parameters extracted from route patterns (e.g., `/users/{id}`)
/// - Query string parsing
/// - Header access (case-insensitive)
/// - Body access (raw bytes, text, or parsed via external JSON)
///
/// # Example
///
/// ```ignore
/// fn get_user(req: &Request) -> Response {
///     let id = req.param_or("id", "0");
///     let page = req.query_or("page", "1");
///     let auth = req.header_or("authorization", "");
///     // ...
/// }
/// ```
///
/// # Implementation Notes
///
/// This struct stores headers with an index-based lookup optimization:
/// - `headers`: Original header pairs for `headers()` iteration
/// - `header_index`: Maps lowercase keys to indices in `headers` for O(1) lookups
///
/// This avoids cloning header values while providing:
/// - O(1) header lookups via `header()` and `header_all()`
/// - Original header iteration via `headers()`
#[non_exhaustive]
pub struct Request {
    method: Method,
    path: String,
    /// Original headers for iteration. See `headers()`.
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    /// Path parameters extracted by routes! macro.
    params: HashMap<String, String>,
    /// Lazily parsed query parameters (stores all values for each key).
    ///
    /// Uses `OnceCell` for lazy initialization - parsing only happens on first
    /// access via `query()` or `query_all()`. This avoids parsing overhead for
    /// handlers that don't use query parameters.
    query_cache: OnceCell<HashMap<String, Vec<String>>>,
    /// Lazily parsed form body (application/x-www-form-urlencoded).
    ///
    /// Uses `OnceCell` for lazy initialization - parsing only happens on first
    /// access via `form()` or `form_all()`. This avoids parsing overhead for
    /// handlers that don't read form data.
    form_cache: OnceCell<HashMap<String, Vec<String>>>,
    /// Lazily parsed cookies from the Cookie header.
    ///
    /// Uses `OnceCell` for lazy initialization - parsing only happens on first
    /// access via `cookie_or()` or `cookies()`.
    cookie_cache: OnceCell<Vec<(String, String)>>,
    /// Index map for O(1) header lookup (lowercase keys -> indices in headers vec).
    /// Supports multiple values per header (e.g., Set-Cookie).
    header_index: HashMap<String, Vec<usize>>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Design decision: Omit internal cache fields from Debug output.
        //
        // Excluded fields and rationale:
        // - `query_cache`: Lazy cache, populated on first query() call. Showing it
        //   would expose implementation details and vary based on access patterns.
        // - `form_cache`: Same as query_cache - lazy initialization detail.
        // - `header_index`: Internal O(1) lookup optimization. Users should see
        //   headers via `headers` field, not the index structure.
        //
        // This keeps Debug output focused on the actual request data that handlers
        // care about, not internal performance optimizations.
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("headers", &self.headers.len())
            .field("body", &self.body.as_ref().map(std::vec::Vec::len))
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl Request {
    /// Create a new Request from raw components.
    ///
    /// This is called by the `routes!` macro after pattern matching.
    /// Users don't typically call this directly.
    #[doc(hidden)]
    #[must_use]
    pub fn new(
        method: Method,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        params: HashMap<String, String>,
    ) -> Self {
        // Build index map: lowercase keys -> indices in headers vec
        // This avoids cloning header values (only lowercase keys are allocated)
        // Pre-allocate based on header count (most headers have unique names)
        let mut header_index: HashMap<String, Vec<usize>> = HashMap::with_capacity(headers.len());

        // Track header sizes for security limits
        let mut total_headers_size: usize = 0;
        let mut oversized_value_count = 0u32;
        let mut total_size_exceeded = false;

        for (i, (k, v)) in headers.iter().enumerate() {
            // Track total headers size (name + value)
            let header_size = k.len().saturating_add(v.len());
            total_headers_size = total_headers_size.saturating_add(header_size);

            // Check individual header value size
            if v.len() > MAX_HEADER_VALUE_LEN {
                oversized_value_count += 1;
            }

            // Check total headers size
            if total_headers_size > MAX_TOTAL_HEADERS_SIZE && !total_size_exceeded {
                total_size_exceeded = true;
            }

            header_index.entry(k.to_lowercase()).or_default().push(i);
        }

        // Log warnings for security limit violations (defense-in-depth)
        if oversized_value_count > 0 {
            crate::log_warn!(
                "Header value size limit exceeded: {} header(s) exceed {} bytes (max: {} bytes)",
                oversized_value_count,
                MAX_HEADER_VALUE_LEN,
                MAX_HEADER_VALUE_LEN
            );
        }
        if total_size_exceeded {
            crate::log_warn!(
                "Total headers size limit exceeded: {} bytes (max: {} bytes)",
                total_headers_size,
                MAX_TOTAL_HEADERS_SIZE
            );
        }

        Self {
            method,
            path,
            headers,
            body,
            params,
            query_cache: OnceCell::new(),
            form_cache: OnceCell::new(),
            cookie_cache: OnceCell::new(),
            header_index,
        }
    }

    /// HTTP method (GET, POST, etc.).
    #[inline]
    pub const fn method(&self) -> Method {
        self.method
    }

    /// Full request path including query string (e.g., "/users/123?page=1").
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Just the path portion without query string (e.g., "/users/123").
    #[inline]
    pub fn path_without_query(&self) -> &str {
        self.path.split('?').next().unwrap_or(&self.path)
    }

    /// Get a path parameter extracted from the route pattern, or a default.
    ///
    /// For route `/users/{id}` matching path `/users/123`, `param_or("id", "")` returns `"123"`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = req.param_or("id", "0");
    /// let format = req.param_or("format", "json");
    /// ```
    #[inline]
    pub fn param_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.param_opt(name).unwrap_or(default)
    }

    /// Internal: Get a path parameter (Option variant).
    #[inline]
    fn param_opt(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    /// Get the first query parameter value from the URL, or a default.
    ///
    /// For path `/users?page=2&limit=10`, `query_or("page", "1")` returns `"2"`.
    /// For multiple values with same key, use `query_all()`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let page = req.query_or("page", "1");
    /// let sort = req.query_or("sort", "created_at");
    /// ```
    #[inline]
    pub fn query_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.query_opt(name).unwrap_or(default)
    }

    /// Internal: Get the first query parameter value (Option variant).
    fn query_opt(&self, name: &str) -> Option<&str> {
        // Parse query string lazily on first access
        let cache = self.query_cache.get_or_init(|| self.parse_query());
        cache.get(name).and_then(|v| v.first()).map(String::as_str)
    }

    /// Get all query parameter values for a key.
    ///
    /// HTTP allows multiple query params with the same name (e.g., `?ids=1&ids=2&ids=3`).
    /// This returns all values for such parameters.
    ///
    /// ```ignore
    /// // For URL: /search?tag=rust&tag=wasm&tag=http
    /// let tags = req.query_all("tag");
    /// assert_eq!(tags, &["rust", "wasm", "http"]);
    /// ```
    pub fn query_all(&self, name: &str) -> &[String] {
        let cache = self.query_cache.get_or_init(|| self.parse_query());
        cache.get(name).map_or(&[], Vec::as_slice)
    }

    /// Get the first header value by name (case-insensitive), or a default.
    ///
    /// Uses pre-normalized `HashMap` for O(1) lookup. Avoids allocation when
    /// the header name is already lowercase (common case).
    /// For headers with multiple values (e.g., Set-Cookie), use `header_all()`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let request_id = req.header_or("x-request-id", "unknown");
    /// let content_type = req.header_or("content-type", "application/octet-stream");
    /// ```
    #[inline]
    pub fn header_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.header_opt(name).unwrap_or(default)
    }

    /// Internal: Get the first header value by name (Option variant).
    fn header_opt(&self, name: &str) -> Option<&str> {
        // Fast path: if name is already lowercase, avoid allocation
        let indices = if name.bytes().all(|b| !b.is_ascii_uppercase()) {
            self.header_index.get(name)
        } else {
            // Slow path: allocate lowercase key for mixed-case lookups
            self.header_index.get(&name.to_lowercase())
        };

        indices
            .and_then(|idx| idx.first())
            .and_then(|&i| self.headers.get(i))
            .map(|(_, v)| v.as_str())
    }

    /// Get the W3C traceparent header from the incoming request, or a default.
    ///
    /// Returns the value of the `traceparent` header if present.
    /// Use this with `ClientRequest::with_trace_id()` to propagate
    /// trace context to outgoing HTTP calls.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // For logging
    /// log!(info, "request received", traceparent: req.trace_id_or("none"));
    ///
    /// // For response headers
    /// ok!({ "data": result }, headers: {
    ///     "traceparent": req.trace_id_or("")
    /// })
    ///
    /// // For outgoing requests
    /// let response = fetch!(GET "https://api.example.com/data")
    ///     .with_trace_id(Some(req.trace_id_or("")))
    ///     .send()?;
    /// ```
    #[inline]
    pub fn trace_id_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.header_opt(HEADER_TRACE_ID).unwrap_or(default)
    }

    /// Get the Bearer token from the Authorization header, or a default.
    ///
    /// Extracts the token from `Authorization: Bearer <token>` header.
    /// Returns the default if the header is missing or doesn't start with "Bearer ".
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let token = req.bearer_token_or("");
    /// guard!(!token.is_empty(), 401, "Missing token");
    ///
    /// // Validate token
    /// let claims = validate_jwt(token)?;
    /// ```
    #[inline]
    pub fn bearer_token_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.header_opt(HEADER_AUTHORIZATION)
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or(default)
    }

    /// Get all values for a header (case-insensitive).
    ///
    /// HTTP allows multiple headers with the same name (e.g., Set-Cookie, Accept).
    /// This returns all values for such headers.
    ///
    /// ```ignore
    /// let cookies = req.header_all("set-cookie");
    /// for cookie in &cookies {
    ///     println!("Cookie: {}", cookie);
    /// }
    /// ```
    pub fn header_all(&self, name: &str) -> Vec<&str> {
        // Fast path: if name is already lowercase, avoid allocation
        let indices = if name.bytes().all(|b| !b.is_ascii_uppercase()) {
            self.header_index.get(name)
        } else {
            // Slow path: allocate lowercase key for mixed-case lookups
            self.header_index.get(&name.to_lowercase())
        };

        indices
            .map(|idx| {
                idx.iter()
                    .filter_map(|&i| self.headers.get(i).map(|(_, v)| v.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all headers as name-value pairs.
    ///
    /// Returns headers in their original form (before normalization).
    #[inline]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Raw request body bytes.
    ///
    /// Returns the raw bytes of the request body, or `None` if no body was provided.
    ///
    /// # Returns
    ///
    /// - `Some(&[u8])` - The raw body bytes
    /// - `None` - No body in request
    #[inline]
    #[must_use]
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// Request body as UTF-8 text.
    ///
    /// # Returns
    ///
    /// - `Some(&str)` - Body successfully decoded as UTF-8
    /// - `None` - No body, or body is not valid UTF-8
    #[inline]
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.body.as_ref().and_then(|b| std::str::from_utf8(b).ok())
    }

    /// Check if request has a body.
    #[inline]
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.body.as_ref().is_some_and(|b| !b.is_empty())
    }

    /// Content-Type header value, or a default.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ct = req.content_type_or("application/octet-stream");
    /// ```
    #[inline]
    #[must_use]
    pub fn content_type_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.content_type_opt().unwrap_or(default)
    }

    /// Internal: Content-Type header value (Option variant).
    #[inline]
    fn content_type_opt(&self) -> Option<&str> {
        use crate::constants::HEADER_CONTENT_TYPE;
        self.header_opt(HEADER_CONTENT_TYPE)
    }

    /// Check if Content-Type is JSON (case-insensitive).
    #[inline]
    #[must_use]
    pub fn is_json(&self) -> bool {
        use crate::constants::MIME_JSON;
        self.content_type_opt()
            .is_some_and(|ct| contains_ignore_ascii_case(ct, MIME_JSON))
    }

    /// Check if Content-Type is form-urlencoded (case-insensitive).
    #[inline]
    #[must_use]
    pub fn is_form(&self) -> bool {
        use crate::constants::MIME_FORM_URLENCODED;
        self.content_type_opt()
            .is_some_and(|ct| contains_ignore_ascii_case(ct, MIME_FORM_URLENCODED))
    }

    /// Check if Content-Type is HTML (case-insensitive).
    #[inline]
    #[must_use]
    pub fn is_html(&self) -> bool {
        use crate::constants::MIME_HTML;
        self.content_type_opt()
            .is_some_and(|ct| contains_ignore_ascii_case(ct, MIME_HTML))
    }

    /// Check if Content-Type is multipart/form-data (case-insensitive).
    #[inline]
    #[must_use]
    pub fn is_multipart(&self) -> bool {
        self.content_type_opt()
            .is_some_and(|ct| contains_ignore_ascii_case(ct, MIME_MULTIPART))
    }

    /// Parse multipart/form-data body into parts.
    ///
    /// Returns parsed parts for file uploads and form fields.
    /// Use `is_multipart()` to check content type first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if req.is_multipart() {
    ///     match req.multipart() {
    ///         Ok(parts) => {
    ///             for part in &parts {
    ///                 println!("Field: {}", part.name());
    ///                 if let Some(filename) = part.filename() {
    ///                     println!("  File: {}, {} bytes", filename, part.data().len());
    ///                 }
    ///             }
    ///         }
    ///         Err(e) => return bad_request!("Invalid multipart: {}", e),
    ///     }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `MultipartError` if:
    /// - Content-Type is not multipart/form-data
    /// - No boundary parameter in Content-Type
    /// - No body in request
    /// - Invalid multipart format
    /// - Too many parts (exceeds MAX_MULTIPART_PARTS)
    pub fn multipart(&self) -> Result<Vec<Part<'_>>, MultipartError> {
        // Get and validate content type
        let content_type = self
            .content_type_opt()
            .ok_or(MultipartError::NotMultipart)?;

        // Extract boundary
        let boundary =
            multipart::extract_boundary(content_type).ok_or(MultipartError::NoBoundary)?;

        // Get body
        let body = self.body().ok_or(MultipartError::NoBody)?;

        // Parse
        multipart::parse_multipart(body, boundary)
    }

    /// Check if client accepts a content type (via Accept header).
    ///
    /// Performs a simple case-insensitive substring match against the Accept header.
    /// Does not parse q-values; returns `true` if the MIME type is present at all.
    ///
    /// ```ignore
    /// // Accept: text/html, application/json
    /// req.accepts("json")  // true
    /// req.accepts("html")  // true
    /// req.accepts("xml")   // false
    /// ```
    pub fn accepts(&self, mime: &str) -> bool {
        self.header_opt("accept")
            .is_some_and(|accept| contains_ignore_ascii_case(accept, mime))
    }

    /// Get the first form field value from a form-urlencoded body, or a default.
    ///
    /// Parses `application/x-www-form-urlencoded` body data.
    /// For multiple values with same key, use `form_all()`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Body: name=Alice&email=alice%40example.com
    /// let name = req.form_or("name", "");  // "Alice"
    /// let email = req.form_or("email", ""); // "alice@example.com"
    /// let missing = req.form_or("missing", "default"); // "default"
    /// ```
    #[inline]
    #[must_use]
    pub fn form_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.form_opt(name).unwrap_or(default)
    }

    /// Internal: Get the first form field value (Option variant).
    fn form_opt(&self, name: &str) -> Option<&str> {
        self.form_cache()
            .get(name)
            .and_then(|v| v.first())
            .map(String::as_str)
    }

    /// Get all form field values for a key from a form-urlencoded body.
    ///
    /// ```ignore
    /// // Body: tags=rust&tags=wasm&tags=http
    /// let tags = req.form_all("tags"); // &["rust", "wasm", "http"]
    /// ```
    pub fn form_all(&self, name: &str) -> &[String] {
        self.form_cache().get(name).map_or(&[], Vec::as_slice)
    }

    /// Get a cookie value by name from the Cookie header, or a default.
    ///
    /// Parses the `Cookie` header lazily on first access.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Cookie: session=abc123; theme=dark
    /// let session = req.cookie_or("session", "");  // "abc123"
    /// let theme = req.cookie_or("theme", "light"); // "dark"
    /// let missing = req.cookie_or("missing", "default"); // "default"
    /// ```
    #[inline]
    #[must_use]
    pub fn cookie_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.cookie_opt(name).unwrap_or(default)
    }

    /// Internal: Get a cookie value by name (Option variant).
    fn cookie_opt(&self, name: &str) -> Option<&str> {
        self.cookie_cache()
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get all cookies from the Cookie header as name-value pairs.
    ///
    /// Parses the `Cookie` header lazily on first access.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Cookie: session=abc123; theme=dark
    /// for (name, value) in req.cookies() {
    ///     println!("{}: {}", name, value);
    /// }
    /// ```
    pub fn cookies(&self) -> &[(String, String)] {
        self.cookie_cache()
    }

    /// Parse request body as JSON using the provided parser.
    ///
    /// # Returns
    ///
    /// - `Some(T)` - Body successfully parsed by the provided function
    /// - `None` - No body, or parser returned `None`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let body = req.json_with(json::try_parse)?;
    /// let name = body.get("name").str_or("");
    /// ```
    #[must_use]
    pub fn json_with<T>(&self, parse: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
        self.body().and_then(parse)
    }

    /// Parse request body as JSON.
    ///
    /// Uses the built-in JSON parser. For custom parsers, use [`json_with`](Self::json_with).
    ///
    /// # Returns
    ///
    /// - `Some(JsonValue)` - Body successfully parsed as JSON
    /// - `None` - No body, or body is not valid JSON
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let body = req.json()?;
    /// let name = body.path_str(&["user", "name"]).unwrap_or("anonymous");
    /// ```
    #[must_use]
    pub fn json(&self) -> Option<JsonValue> {
        self.json_with(json::try_parse)
    }

    // --- Private helpers ---

    fn form_cache(&self) -> &HashMap<String, Vec<String>> {
        self.form_cache.get_or_init(|| self.parse_form())
    }

    fn cookie_cache(&self) -> &Vec<(String, String)> {
        self.cookie_cache.get_or_init(|| self.parse_cookies())
    }

    fn parse_cookies(&self) -> Vec<(String, String)> {
        self.header_opt(HEADER_COOKIE)
            .map(|h| {
                cookie::parse_cookie_header(h)
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parse_form(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        if let Some(body) = self.text() {
            let mut truncated = false;
            let mut decode_failures = 0u32;
            for pair in body.split('&') {
                // Defense-in-depth: limit number of form fields
                if map.len() >= MAX_FORM_FIELDS {
                    truncated = true;
                    break;
                }

                if let Some((key, value)) = pair.split_once('=') {
                    match (url_decode(key), url_decode(value)) {
                        (Ok(decoded_key), Ok(decoded_value)) => {
                            map.entry(decoded_key).or_default().push(decoded_value);
                        },
                        _ => {
                            decode_failures += 1;
                        },
                    }
                } else if !pair.is_empty() {
                    match url_decode(pair) {
                        Ok(decoded_key) => {
                            map.entry(decoded_key).or_default().push(String::new());
                        },
                        Err(_) => {
                            decode_failures += 1;
                        },
                    }
                }
            }

            if truncated {
                crate::log_warn!(
                    "Form field limit exceeded: dropped fields after {} (max: {})",
                    MAX_FORM_FIELDS,
                    MAX_FORM_FIELDS
                );
            }
            if decode_failures > 0 {
                crate::log_warn!(
                    "Form field decode failed: dropped {} field(s). Check for invalid percent-encoding (e.g., %ZZ) or values exceeding {} bytes after decoding",
                    decode_failures,
                    MAX_URL_DECODED_LEN
                );
            }
        }

        map
    }

    fn parse_query(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        let mut dropped_count = 0u32;

        if let Some(query_start) = self.path.find('?') {
            let query = &self.path[query_start + 1..];
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    // URL decode and store (supports multiple values per key)
                    match (url_decode(key), url_decode(value)) {
                        (Ok(decoded_key), Ok(decoded_value)) => {
                            map.entry(decoded_key).or_default().push(decoded_value);
                        },
                        _ => {
                            dropped_count += 1;
                        },
                    }
                } else if !pair.is_empty() {
                    // Key without value (e.g., "?flag")
                    match url_decode(pair) {
                        Ok(decoded_key) => {
                            map.entry(decoded_key).or_default().push(String::new());
                        },
                        Err(_) => {
                            dropped_count += 1;
                        },
                    }
                }
            }
        }

        if dropped_count > 0 {
            crate::log_warn!(
                "Query param decode failed: dropped {} param(s). Check for invalid percent-encoding (e.g., %ZZ) or values exceeding {} bytes after decoding",
                dropped_count,
                MAX_URL_DECODED_LEN
            );
        }

        map
    }
}

#[cfg(test)]
mod tests;
