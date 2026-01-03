//! Bridge component - Translates WASI HTTP to mik handler interface.
//!
//! This component:
//! - Imports mik:core/handler (the user's handler)
//! - Exports wasi:http/incoming-handler (standard WASI HTTP interface)
//!
//! After composition, the final component runs on any WASI P2 HTTP runtime.
//!
//! ## Configuration
//!
//! The following environment variables can be used to configure the bridge:
//!
//! - `MIK_MAX_BODY_SIZE`: Maximum request body size in bytes (default: 10MB)
//!   Example: `MIK_MAX_BODY_SIZE=52428800` for 50MB
//!
//! ## Security Considerations
//!
//! ### Rate Limiting
//!
//! **This component does not implement rate limiting.** Rate limiting should be
//! handled at the infrastructure layer by your WASI HTTP runtime or a reverse proxy.
//!
//! For production deployments, always use a reverse proxy or API gateway that
//! provides:
//! - Rate limiting (requests per second/minute)
//! - Connection limits
//! - Request timeout enforcement
//! - DDoS protection
//!
//! ### Request Size Limits
//!
//! The bridge enforces `MIK_MAX_BODY_SIZE` to prevent memory exhaustion from
//! large request bodies. Requests exceeding this limit receive a 413 response.

#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::mik::core::handler::{self, Method, RequestData};
use bindings::wasi::cli::environment;
use bindings::wasi::cli::stderr;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};
use std::sync::OnceLock;

/// Default maximum request body size (10MB).
const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

// ============================================================================
// HTTP CONSTANTS (centralized for consistency)
// ============================================================================

/// Content-Type header name.
const HEADER_CONTENT_TYPE: &str = "content-type";

/// RFC 7807 Problem Details MIME type.
const MIME_PROBLEM_JSON: &[u8] = b"application/problem+json";

/// Returns the standard title for an HTTP status code.
#[inline]
const fn status_title(code: u16) -> &'static str {
    match code {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

/// Cached max body size from environment.
static MAX_BODY_SIZE: OnceLock<usize> = OnceLock::new();

/// Returns the maximum allowed request body size in bytes.
///
/// Reads from `MIK_MAX_BODY_SIZE` environment variable on first call.
/// Falls back to [`DEFAULT_MAX_BODY_SIZE`] (10MB) if not set or invalid.
///
/// The value is cached for the lifetime of the component, so environment
/// changes after first access have no effect.
///
/// # Example Environment Configuration
///
/// ```bash
/// # 50MB limit
/// MIK_MAX_BODY_SIZE=52428800
///
/// # 1MB limit
/// MIK_MAX_BODY_SIZE=1048576
/// ```
fn get_max_body_size() -> usize {
    *MAX_BODY_SIZE.get_or_init(|| {
        environment::get_environment()
            .into_iter()
            .find(|(k, _)| k == "MIK_MAX_BODY_SIZE")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_BODY_SIZE)
    })
}

/// Bridge component implementing WASI HTTP to mik handler translation.
///
/// This is the core component that enables portable HTTP handlers:
/// - Receives WASI HTTP requests via `wasi:http/incoming-handler`
/// - Converts them to `mik:core/handler` format
/// - Invokes the user's handler
/// - Converts the response back to WASI HTTP format
///
/// After WAC composition with a user handler, the combined component
/// runs on any WASI P2 HTTP runtime.
struct Bridge;

/// WASI HTTP handler implementation.
///
/// Implements the `wasi:http/incoming-handler` interface, which is the
/// standard entry point for HTTP requests in WASI HTTP runtimes.
///
/// ## Request Flow
///
/// 1. Extract path, method, headers from WASI HTTP request
/// 2. Read body with size limit enforcement (413 if exceeded)
/// 3. Convert to `mik:core/handler::RequestData`
/// 4. Call user's `handler::handle()` function
/// 5. Convert response and send via WASI HTTP
///
/// ## Error Handling
///
/// - Unsupported HTTP methods (CONNECT, TRACE) → 501 Not Implemented
/// - Body exceeds `MIK_MAX_BODY_SIZE` → 413 Payload Too Large
/// - Invalid status codes are clamped to 500 with error logging
/// - Invalid UTF-8 headers are silently dropped with error logging
impl Guest for Bridge {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        // 1. Extract data from WASI HTTP request
        let path = request.path_with_query().unwrap_or_default();

        // Check for unsupported HTTP methods first
        let method = match convert_method(request.method()) {
            Some(m) => m,
            None => {
                // Return 501 Not Implemented for unsupported methods
                send_error_response(response_out, 501, status_title(501), &path);
                return;
            }
        };
        let headers = extract_headers(&request);

        // 2. Read body with size limit check
        let body = match read_body(&request) {
            BodyResult::Ok(body) => body,
            BodyResult::TooLarge => {
                // Return 413 Payload Too Large
                send_error_response(response_out, 413, status_title(413), &path);
                return;
            }
        };

        // 3. Build mik request-data
        let mik_request = RequestData {
            method,
            path,
            headers,
            body,
        };

        // 4. Call the user's handler
        let mik_response = handler::handle(&mik_request);

        // 5. Convert to WASI HTTP response and send
        // Note: Fields resource is consumed by OutgoingResponse::new().
        // We scope it explicitly to ensure proper WASI resource lifecycle.
        let outgoing = {
            let headers = Fields::new();
            for (name, value) in mik_response.headers {
                let _ = headers.append(&name, &value.into_bytes());
            }
            // Ownership of headers transfers to OutgoingResponse here
            OutgoingResponse::new(headers)
        };
        // Validate and clamp status code to valid HTTP range (100-599)
        let status_code = if mik_response.status < 100 {
            log_error(&format!(
                "Invalid HTTP status code {}: must be >= 100, using 500",
                mik_response.status
            ));
            500
        } else if mik_response.status >= 600 {
            log_error(&format!(
                "Invalid HTTP status code {}: must be < 600, using 500",
                mik_response.status
            ));
            500
        } else {
            mik_response.status
        };
        let _ = outgoing.set_status_code(status_code);

        // Get body handle and ensure it's always finished per WASI HTTP spec.
        // OutgoingBody::finish() must be called whenever we successfully get a body handle.
        match outgoing.body() {
            Ok(body_handle) => {
                // Write body bytes if present
                if let Some(body_bytes) = mik_response.body.as_ref() {
                    match body_handle.write() {
                        Ok(stream) => {
                            if let Err(e) = stream.blocking_write_and_flush(body_bytes) {
                                log_error(&format!("Failed to write response body: {:?}", e));
                            }
                            // Explicitly drop stream before calling finish (WASI resource cleanup)
                            drop(stream);
                        }
                        Err(e) => {
                            log_error(&format!("Failed to get body write stream: {:?}", e));
                        }
                    }
                }
                // Always finish the body handle (required by WASI HTTP spec)
                if let Err(e) = OutgoingBody::finish(body_handle, None) {
                    log_error(&format!("Failed to finish response body: {:?}", e));
                }
            }
            Err(e) => {
                // body() failed - no body handle to finish, but log for debugging
                log_error(&format!("Failed to get response body handle: {:?}", e));
            }
        }

        // Set response exactly once at the end
        ResponseOutparam::set(response_out, Ok(outgoing));
    }
}

/// Log an error message to stderr.
fn log_error(msg: &str) {
    let stream = stderr::get_stderr();
    let _ = stream.blocking_write_and_flush(format!("[mik-bridge] ERROR: {}\n", msg).as_bytes());
}

/// Convert WASI HTTP method to mik HTTP method.
///
/// Returns `None` for unsupported methods (Connect, Trace, Other).
/// Callers should return 501 Not Implemented for these.
fn convert_method(m: bindings::wasi::http::types::Method) -> Option<Method> {
    use bindings::wasi::http::types::Method as WasiMethod;
    match m {
        WasiMethod::Get => Some(Method::Get),
        WasiMethod::Post => Some(Method::Post),
        WasiMethod::Put => Some(Method::Put),
        WasiMethod::Delete => Some(Method::Delete),
        WasiMethod::Patch => Some(Method::Patch),
        WasiMethod::Head => Some(Method::Head),
        WasiMethod::Options => Some(Method::Options),
        // Unsupported methods - return None to trigger 501
        WasiMethod::Connect | WasiMethod::Trace | WasiMethod::Other(_) => None,
    }
}

/// Extracts HTTP headers from a WASI HTTP request.
///
/// Converts WASI `Fields` resource to a list of (name, value) string pairs.
/// Headers with invalid UTF-8 values are silently dropped and logged.
///
/// # Behavior
///
/// - Header names are preserved as-is (typically lowercase per HTTP/2)
/// - Header values must be valid UTF-8 (per HTTP semantics)
/// - Invalid UTF-8 values are dropped with a single log message per request
/// - Multiple headers with the same name are preserved as separate entries
///
/// # Returns
///
/// Vector of (header_name, header_value) pairs with valid UTF-8 encoding.
fn extract_headers(req: &IncomingRequest) -> Vec<(String, String)> {
    let headers = req.headers();
    let mut result = Vec::new();
    let mut invalid_count = 0;

    for (name, value) in headers.entries() {
        match String::from_utf8(value) {
            Ok(v) => result.push((name, v)),
            Err(_) => invalid_count += 1,
        }
    }

    // Log once per request if any headers were dropped (avoid log flooding)
    if invalid_count > 0 {
        log_error(&format!(
            "Dropped {} header(s) with invalid UTF-8 encoding",
            invalid_count
        ));
    }

    result
}

/// Result of reading request body.
enum BodyResult {
    /// Body read successfully (may be None if empty).
    Ok(Option<Vec<u8>>),
    /// Body exceeded size limit.
    TooLarge,
}

fn read_body(req: &IncomingRequest) -> BodyResult {
    let body = match req.consume() {
        Ok(b) => b,
        Err(_) => return BodyResult::Ok(None),
    };
    let stream = match body.stream() {
        Ok(s) => s,
        Err(_) => return BodyResult::Ok(None),
    };
    let max_size = get_max_body_size();

    // Pre-allocate based on Content-Length header if available, otherwise use chunk size.
    // This avoids multiple Vec reallocations for large bodies.
    // Search headers manually to avoid String allocation from .get(&String).
    let content_length_hint = req
        .headers()
        .entries()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| std::str::from_utf8(value).ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(64 * 1024); // Default to one chunk size

    // Cap pre-allocation at max_size to avoid memory exhaustion from malicious headers
    let initial_capacity = content_length_hint.min(max_size);

    let mut bytes = Vec::with_capacity(initial_capacity);
    let mut too_large = false;
    loop {
        match stream.blocking_read(64 * 1024) {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => {
                // Check size limit before extending (use checked arithmetic to prevent overflow)
                let new_len = match bytes.len().checked_add(chunk.len()) {
                    Some(len) => len,
                    None => {
                        too_large = true;
                        break;
                    }
                };
                if new_len > max_size {
                    too_large = true;
                    break;
                }
                bytes.extend(chunk);
            }
            Err(_) => break,
        }
    }
    // Explicitly drop stream to ensure WASI resource cleanup before returning
    drop(stream);

    if too_large {
        return BodyResult::TooLarge;
    }

    if bytes.is_empty() {
        BodyResult::Ok(None)
    } else {
        BodyResult::Ok(Some(bytes))
    }
}

/// Escape a string for JSON output per RFC 7159.
///
/// Handles quotes, backslashes, and control characters.
///
/// # Note: Intentional Duplication
///
/// This function is duplicated in `mik-sdk/src/log.rs` as `__escape_json()`.
/// This is intentional because:
/// - `mik-bridge` is a standalone WASM component that cannot depend on `mik-sdk`
/// - `mik-sdk` cannot depend on `mik-bridge` (it's the other way around)
/// - Creating a shared crate for ~20 lines of code adds unnecessary complexity
///
/// If you modify this function, please update the duplicate in `mik-sdk` too.
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                // Escape control characters as \uXXXX
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Send an RFC 7807 error response.
///
/// Includes the request path in the `instance` field per RFC 7807 for debugging.
fn send_error_response(response_out: ResponseOutparam, status: u16, title: &str, instance: &str) {
    // Escape the instance path for JSON per RFC 7158
    let escaped_instance = escape_json_string(instance);

    let body_json = format!(
        r#"{{"type":"about:blank","title":"{}","status":{},"instance":"{}"}}"#,
        title, status, escaped_instance
    );

    let headers = Fields::new();
    let _ = headers.append(HEADER_CONTENT_TYPE, MIME_PROBLEM_JSON);

    let outgoing = OutgoingResponse::new(headers);
    let _ = outgoing.set_status_code(status);

    // Get body handle and ensure it's always finished per WASI HTTP spec.
    // OutgoingBody::finish() must be called whenever we successfully get a body handle.
    if let Ok(body_handle) = outgoing.body() {
        if let Ok(stream) = body_handle.write() {
            let _ = stream.blocking_write_and_flush(body_json.as_bytes());
            drop(stream);
        }
        // Always finish the body handle (required by WASI HTTP spec)
        let _ = OutgoingBody::finish(body_handle, None);
    }

    ResponseOutparam::set(response_out, Ok(outgoing));
}

bindings::export!(Bridge with_types_in bindings);

// ============================================================================
// UNIT TESTS
// ============================================================================
// Note: These tests run on native (not WASM) and test pure functions only.
// Integration tests for the full WASI HTTP flow require a WASM runtime.

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // status_title tests
    // ========================================================================

    #[test]
    fn test_status_title_common_codes() {
        assert_eq!(status_title(400), "Bad Request");
        assert_eq!(status_title(401), "Unauthorized");
        assert_eq!(status_title(403), "Forbidden");
        assert_eq!(status_title(404), "Not Found");
        assert_eq!(status_title(405), "Method Not Allowed");
        assert_eq!(status_title(413), "Payload Too Large");
        assert_eq!(status_title(422), "Unprocessable Entity");
        assert_eq!(status_title(429), "Too Many Requests");
        assert_eq!(status_title(500), "Internal Server Error");
        assert_eq!(status_title(501), "Not Implemented");
        assert_eq!(status_title(502), "Bad Gateway");
        assert_eq!(status_title(503), "Service Unavailable");
    }

    #[test]
    fn test_status_title_unknown_codes() {
        assert_eq!(status_title(418), "Error"); // I'm a teapot
        assert_eq!(status_title(599), "Error");
        assert_eq!(status_title(100), "Error");
        assert_eq!(status_title(200), "Error");
        assert_eq!(status_title(301), "Error");
    }

    // ========================================================================
    // escape_json_string tests
    // ========================================================================

    #[test]
    fn test_escape_json_simple() {
        assert_eq!(escape_json_string("hello"), "hello");
        assert_eq!(escape_json_string("hello world"), "hello world");
    }

    #[test]
    fn test_escape_json_quotes() {
        assert_eq!(escape_json_string(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn test_escape_json_backslash() {
        assert_eq!(escape_json_string(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn test_escape_json_newlines_tabs() {
        assert_eq!(escape_json_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_json_string("col1\tcol2"), "col1\\tcol2");
        assert_eq!(escape_json_string("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_json_control_chars() {
        let input = "\x00\x01\x1f";
        let escaped = escape_json_string(input);
        assert!(escaped.contains("\\u0000"));
        assert!(escaped.contains("\\u0001"));
        assert!(escaped.contains("\\u001f"));
    }

    #[test]
    fn test_escape_json_unicode() {
        // Unicode should pass through unchanged
        assert_eq!(escape_json_string("日本語"), "日本語");
        assert_eq!(escape_json_string("emoji: 🎉"), "emoji: 🎉");
    }

    #[test]
    fn test_escape_json_empty() {
        assert_eq!(escape_json_string(""), "");
    }

    #[test]
    fn test_escape_json_mixed() {
        let input = "Hello \"World\"\nPath: C:\\test";
        let escaped = escape_json_string(input);
        assert!(escaped.contains("\\\""));
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\\\"));
    }

    // ========================================================================
    // Edge case tests for request path handling
    // ========================================================================

    #[test]
    fn test_escape_json_url_paths() {
        // Paths that might appear in RFC 7807 instance field
        assert_eq!(escape_json_string("/api/users/123"), "/api/users/123");
        assert_eq!(
            escape_json_string("/api/search?q=hello world"),
            "/api/search?q=hello world"
        );
        // Path with special chars that need escaping
        assert_eq!(
            escape_json_string("/api/data?filter=\"active\""),
            r#"/api/data?filter=\"active\""#
        );
    }

    #[test]
    fn test_escape_json_malicious_input() {
        // Attempt to break out of JSON string
        let malicious = r#"</script><script>alert('xss')</script>"#;
        let escaped = escape_json_string(malicious);
        // Should not contain unescaped quotes
        assert!(!escaped.contains(r#"""#) || escaped.contains(r#"\""#));
    }

    // ========================================================================
    // DEFAULT_MAX_BODY_SIZE constant test
    // ========================================================================

    #[test]
    fn test_default_max_body_size() {
        assert_eq!(DEFAULT_MAX_BODY_SIZE, 10 * 1024 * 1024); // 10MB
    }
}
