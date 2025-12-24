//! Bridge component - Translates WASI HTTP to mik handler interface.
//!
//! This component:
//! - Imports mik:core/handler (the user's handler)
//! - Exports wasi:http/incoming-handler (standard WASI HTTP interface)
//!
//! After composition, the final component runs on any WASI HTTP runtime:
//! Spin, wasmCloud, wasmtime serve, etc.
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
//! handled at the infrastructure layer by your WASI HTTP runtime:
//!
//! - **Spin**: Configure rate limiting in `spin.toml` or use a reverse proxy
//! - **wasmCloud**: Use the built-in rate limiting capabilities or a gateway
//! - **wasmtime serve**: Place behind a reverse proxy (nginx, Caddy, etc.)
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

/// Get the maximum body size, reading from environment on first call.
fn get_max_body_size() -> usize {
    *MAX_BODY_SIZE.get_or_init(|| {
        environment::get_environment()
            .into_iter()
            .find(|(k, _)| k == "MIK_MAX_BODY_SIZE")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_BODY_SIZE)
    })
}

struct Bridge;

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

/// Escape a string for JSON output per RFC 7158.
///
/// Handles quotes, backslashes, and control characters.
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
