//! Centralized constants for the mik-sdk crate.
//!
//! All limits, sizes, and magic numbers are defined here for easy tuning
//! and consistent behavior across the SDK.

// ============================================================================
// TIME CONSTANTS
// ============================================================================

/// Seconds in a day (24 * 60 * 60).
pub const SECONDS_PER_DAY: u64 = 86400;

/// Seconds in an hour (60 * 60).
pub const SECONDS_PER_HOUR: u64 = 3600;

/// Seconds in a minute.
pub const SECONDS_PER_MINUTE: u64 = 60;

// ============================================================================
// JSON LIMITS
// ============================================================================

/// Maximum JSON input size (1MB) - prevents memory exhaustion.
pub const MAX_JSON_SIZE: usize = 1_000_000;

/// Maximum JSON nesting depth - prevents stack overflow.
///
/// Set conservatively low (20) because:
/// 1. Real-world JSON rarely exceeds 10 levels of nesting
/// 2. miniserde uses recursive parsing which consumes stack per level
/// 3. WASM environments may have limited stack space
pub const MAX_JSON_DEPTH: usize = 20;

// ============================================================================
// HTTP REQUEST LIMITS
// ============================================================================

/// Maximum decoded URL length (64KB).
/// Prevents DoS via extremely long encoded URLs.
pub const MAX_URL_DECODED_LEN: usize = 65536;

/// Maximum number of form fields.
/// Prevents DoS via forms with thousands of tiny fields.
pub const MAX_FORM_FIELDS: usize = 1000;

/// Maximum individual header value length (8KB).
/// Prevents memory exhaustion from single large headers.
pub const MAX_HEADER_VALUE_LEN: usize = 8192;

/// Maximum total size of all headers combined (1MB).
/// Prevents memory exhaustion from many headers.
pub const MAX_TOTAL_HEADERS_SIZE: usize = 1024 * 1024;

// ============================================================================
// ENCODING
// ============================================================================

/// Hex character lookup table for fast byte-to-hex conversion.
pub const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

// ============================================================================
// COMMON HEADER NAMES
// ============================================================================

/// Content-Type header name (lowercase for lookups).
pub const HEADER_CONTENT_TYPE: &str = "content-type";

/// Content-Type header name (title-case for setting headers).
pub const HEADER_CONTENT_TYPE_TITLE: &str = "Content-Type";

/// Authorization header name (lowercase for lookups).
pub const HEADER_AUTHORIZATION: &str = "authorization";

/// Trace ID header name (lowercase for lookups).
pub const HEADER_TRACE_ID: &str = "x-trace-id";

/// Trace ID header name (title-case for setting headers).
pub const HEADER_TRACE_ID_TITLE: &str = "X-Trace-Id";

// ============================================================================
// COMMON MIME TYPES
// ============================================================================

/// JSON MIME type.
pub const MIME_JSON: &str = "application/json";

/// RFC 7807 Problem Details MIME type.
pub const MIME_PROBLEM_JSON: &str = "application/problem+json";

/// HTML MIME type.
pub const MIME_HTML: &str = "text/html";

/// Form URL-encoded MIME type.
pub const MIME_FORM_URLENCODED: &str = "application/x-www-form-urlencoded";

// ============================================================================
// HTTP STATUS TITLES
// ============================================================================

/// Returns the standard title for an HTTP status code.
///
/// This centralizes status code → title mapping for RFC 7807 Problem Details
/// responses and logging.
///
/// # Examples
///
/// ```
/// use mik_sdk::constants::status_title;
///
/// assert_eq!(status_title(200), "OK");
/// assert_eq!(status_title(404), "Not Found");
/// assert_eq!(status_title(500), "Internal Server Error");
/// assert_eq!(status_title(999), "Error"); // Unknown codes
/// ```
#[inline]
pub const fn status_title(code: u16) -> &'static str {
    match code {
        // 2xx Success
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        // 3xx Redirection
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        // 4xx Client Errors
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        // 5xx Server Errors
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        // Fallback for unknown codes
        _ => "Error",
    }
}
