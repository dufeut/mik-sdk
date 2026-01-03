//! HTTP client error types and WASI error mapping.

/// HTTP client error types for WASI HTTP outbound requests.
///
/// These errors are returned when making outbound HTTP requests via `wasi:http/outgoing-handler`.
/// The WASI HTTP specification defines error codes that are mapped to these typed variants
/// for better error handling in Rust code.
///
/// # WASI HTTP Error Mapping
///
/// WASI runtimes return errors as strings or error codes.
/// Use [`map_wasi_error`] to convert these to typed `Error` variants:
///
/// ```
/// use mik_sdk::http_client::{Error, map_wasi_error};
///
/// // Convert WASI error string to typed error
/// let typed_error = map_wasi_error("DNS lookup failed: NXDOMAIN");
/// assert!(matches!(typed_error, Error::DnsError(_)));
/// ```
///
/// # Error Handling Example
///
/// ```no_run
/// # use mik_sdk::http_client::{self, Error, Response};
/// # fn send(_req: &http_client::ClientRequest) -> Result<Response, Error> {
/// #     Ok(Response::new(200, vec![], vec![]))
/// # }
/// # fn main() {
/// let result = http_client::get("https://api.example.com/data")
///     .send_with(send);
///
/// match result {
///     Ok(response) => println!("Got response: {}", response.status),
///     Err(Error::DnsError(msg)) => eprintln!("DNS failed: {}", msg),
///     Err(Error::ConnectionError(msg)) => eprintln!("Connection failed: {}", msg),
///     Err(Error::Timeout { timeout_ms }) => {
///         match timeout_ms {
///             Some(ms) => eprintln!("Request timed out after {}ms", ms),
///             None => eprintln!("Request timed out"),
///         }
///     }
///     Err(Error::TlsError(msg)) => eprintln!("TLS/SSL error: {}", msg),
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// DNS resolution failed - the hostname could not be resolved to an IP address.
    ///
    /// This error occurs when:
    /// - The hostname does not exist (NXDOMAIN)
    /// - DNS server is unreachable
    /// - DNS query timed out
    /// - The hostname format is invalid
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `DnsError`:
    /// - `"DNS lookup failed"` / `"dns error"`
    /// - `"NXDOMAIN"` / `"no such host"`
    /// - `"DNS resolution failed"` / `"name resolution failed"`
    /// - `"getaddrinfo failed"` / `"could not resolve host"`
    ///
    /// # Example Error Messages
    ///
    /// - `"DNS error: NXDOMAIN for api.invalid-domain.xyz"`
    /// - `"Failed to resolve hostname: connection refused to DNS server"`
    DnsError(String),

    /// TCP connection failed - could not establish connection to the server.
    ///
    /// This error occurs when:
    /// - The server is not accepting connections (connection refused)
    /// - The connection attempt timed out before completing
    /// - The network is unreachable
    /// - The server reset the connection
    /// - The host is down or not responding
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `ConnectionError`:
    /// - `"connection refused"` / `"ECONNREFUSED"`
    /// - `"connection reset"` / `"ECONNRESET"`
    /// - `"network unreachable"` / `"ENETUNREACH"`
    /// - `"host unreachable"` / `"EHOSTUNREACH"`
    /// - `"connection failed"` / `"failed to connect"`
    /// - `"socket error"` / `"I/O error"`
    ///
    /// # Example Error Messages
    ///
    /// - `"Connection refused: 192.168.1.100:8080"`
    /// - `"Network is unreachable"`
    /// - `"Connection reset by peer"`
    ConnectionError(String),

    /// Request timed out before completion.
    ///
    /// This error occurs when:
    /// - The connection was established but no response arrived within the timeout
    /// - The server took too long to send headers or body
    /// - The overall request duration exceeded the configured timeout
    ///
    /// Note: Connection timeouts during TCP handshake typically result in
    /// [`ConnectionError`](Error::ConnectionError) instead.
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `Timeout`:
    /// - `"timeout"` / `"timed out"` / `"request timeout"`
    /// - `"deadline exceeded"` / `"operation timed out"`
    /// - `"ETIMEDOUT"`
    ///
    /// # Timeout Configuration
    ///
    /// Set timeouts using [`super::ClientRequest::timeout_ms`]:
    /// ```no_run
    /// # use mik_sdk::http_client::{self, Response, Error};
    /// # fn send(_req: &http_client::ClientRequest) -> Result<Response, Error> {
    /// #     Ok(Response::new(200, vec![], vec![]))
    /// # }
    /// # fn main() -> Result<(), Error> {
    /// let response = http_client::get("https://slow-api.example.com")
    ///     .timeout_ms(5000)  // 5 second timeout
    ///     .send_with(send)?;
    /// # Ok(())
    /// # }
    /// ```
    Timeout {
        /// The configured timeout in milliseconds, if known.
        /// This is `None` when the timeout is detected from WASI error strings
        /// where the configured duration isn't available.
        timeout_ms: Option<u64>,
    },

    /// TLS/SSL handshake or certificate verification failed.
    ///
    /// This error occurs when:
    /// - The server's SSL certificate is invalid or expired
    /// - The certificate doesn't match the hostname
    /// - The certificate chain is incomplete or untrusted
    /// - The TLS handshake failed (protocol mismatch, cipher issues)
    /// - Client certificate authentication failed
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `TlsError`:
    /// - `"certificate"` / `"cert"` (anywhere in message)
    /// - `"SSL"` / `"TLS"` / `"handshake failed"`
    /// - `"CERTIFICATE_VERIFY_FAILED"` / `"certificate verify failed"`
    /// - `"self signed certificate"` / `"expired certificate"`
    /// - `"hostname mismatch"` / `"invalid certificate"`
    ///
    /// # Example Error Messages
    ///
    /// - `"TLS error: certificate has expired"`
    /// - `"SSL handshake failed: certificate verify failed"`
    /// - `"TLS error: hostname mismatch, expected api.example.com"`
    TlsError(String),

    /// The URL format is invalid or uses an unsupported scheme.
    ///
    /// This error occurs when:
    /// - The URL doesn't start with `http://` or `https://`
    /// - The URL is missing a hostname
    /// - The URL contains invalid characters
    /// - The port number is out of range
    ///
    /// # Example Error Messages
    ///
    /// - `"Invalid URL: URL must start with http:// or https://: ftp://example.com"`
    /// - `"Invalid URL: Missing host in URL"`
    InvalidUrl(String),

    /// The request configuration is invalid.
    ///
    /// This error occurs when:
    /// - Headers contain invalid characters
    /// - The HTTP method is not supported
    /// - Request body is too large
    /// - Required request parameters are missing
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `InvalidRequest`:
    /// - `"invalid header"` / `"bad header"`
    /// - `"invalid method"` / `"unsupported method"`
    /// - `"request too large"` / `"body too large"`
    ///
    /// # Example Error Messages
    ///
    /// - `"Invalid request: header name contains invalid characters"`
    /// - `"Invalid request: request body exceeds maximum size"`
    InvalidRequest(String),

    /// Failed to read or process the response.
    ///
    /// This error occurs when:
    /// - The response body exceeds size limits
    /// - The response stream was unexpectedly closed
    /// - The response body could not be read from the stream
    /// - The response contained malformed data
    ///
    /// # Common WASI Error Patterns
    ///
    /// These WASI error strings map to `ResponseError`:
    /// - `"response"` / `"body"` (in error context)
    /// - `"stream"` / `"read error"`
    /// - `"content too large"` / `"payload too large"`
    ///
    /// # Example Error Messages
    ///
    /// - `"Response error: body stream closed unexpectedly"`
    /// - `"Response error: content length exceeds limit"`
    ResponseError(String),

    /// Request blocked due to SSRF protection.
    ///
    /// This error occurs when:
    /// - `deny_private_ips()` was called and the URL points to a private/internal address
    /// - The target IP is in a private range (127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
    /// - The target is localhost or a loopback address
    ///
    /// This protection prevents Server-Side Request Forgery (SSRF) attacks where
    /// user-controlled URLs could be used to access internal services.
    ///
    /// # Example Error Messages
    ///
    /// - `"SSRF blocked: Request to private/internal address blocked: 192.168.1.1"`
    /// - `"SSRF blocked: Request to private/internal address blocked: localhost"`
    SsrfBlocked(String),

    /// An error that doesn't fit other categories.
    ///
    /// This is a catch-all for WASI HTTP errors that don't match known patterns.
    /// The string contains the original error message from the WASI runtime.
    ///
    /// If you encounter an `Other` error frequently, please file an issue so
    /// we can add proper mapping for that error type.
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DnsError(msg) => write!(f, "dns error: {msg}"),
            Self::ConnectionError(msg) => write!(f, "connection error: {msg}"),
            Self::Timeout { timeout_ms: None } => write!(f, "request timeout"),
            Self::Timeout {
                timeout_ms: Some(ms),
            } => write!(f, "request timeout after {ms}ms"),
            Self::TlsError(msg) => write!(f, "tls error: {msg}"),
            Self::InvalidUrl(msg) => write!(f, "invalid url: {msg}"),
            Self::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
            Self::ResponseError(msg) => write!(f, "response error: {msg}"),
            Self::SsrfBlocked(msg) => write!(f, "ssrf blocked: {msg}"),
            Self::Other(msg) => write!(f, "http client error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a DNS error.
    #[inline]
    #[must_use]
    pub fn dns(msg: impl Into<String>) -> Self {
        Self::DnsError(msg.into())
    }

    /// Create a connection error.
    #[inline]
    #[must_use]
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::ConnectionError(msg.into())
    }

    /// Create a timeout error without a known duration.
    ///
    /// Use this when the timeout is detected from WASI error strings
    /// where the configured duration isn't available.
    #[inline]
    #[must_use]
    pub const fn timeout() -> Self {
        Self::Timeout { timeout_ms: None }
    }

    /// Create a timeout error with a known duration.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::timeout_with_duration(5000);
    /// assert_eq!(err.to_string(), "request timeout after 5000ms");
    /// ```
    #[inline]
    #[must_use]
    pub const fn timeout_with_duration(ms: u64) -> Self {
        Self::Timeout {
            timeout_ms: Some(ms),
        }
    }

    /// Create a TLS/certificate error.
    #[inline]
    #[must_use]
    pub fn tls(msg: impl Into<String>) -> Self {
        Self::TlsError(msg.into())
    }

    /// Create an invalid URL error.
    #[inline]
    #[must_use]
    pub fn invalid_url(msg: impl Into<String>) -> Self {
        Self::InvalidUrl(msg.into())
    }

    /// Create an invalid request error.
    #[inline]
    #[must_use]
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::InvalidRequest(msg.into())
    }

    /// Create a response error.
    #[inline]
    #[must_use]
    pub fn response(msg: impl Into<String>) -> Self {
        Self::ResponseError(msg.into())
    }

    /// Create an SSRF blocked error.
    #[inline]
    #[must_use]
    pub fn ssrf_blocked(msg: impl Into<String>) -> Self {
        Self::SsrfBlocked(msg.into())
    }

    /// Create a generic HTTP error.
    #[inline]
    #[must_use]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    // ========================================================================
    // Classification helpers
    // ========================================================================

    /// Returns `true` if this is a timeout error.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::timeout();
    /// assert!(err.is_timeout());
    ///
    /// let err = Error::DnsError("failed".into());
    /// assert!(!err.is_timeout());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Returns `true` if this error is likely transient and worth retrying.
    ///
    /// Retryable errors include:
    /// - [`Timeout`](Self::Timeout) - request timed out
    /// - [`ConnectionError`](Self::ConnectionError) - network connectivity issues
    /// - [`DnsError`](Self::DnsError) - DNS resolution failures (may be transient)
    ///
    /// Non-retryable errors (client configuration issues):
    /// - [`InvalidUrl`](Self::InvalidUrl) - malformed URL
    /// - [`InvalidRequest`](Self::InvalidRequest) - bad request configuration
    /// - [`SsrfBlocked`](Self::SsrfBlocked) - blocked by SSRF protection
    /// - [`TlsError`](Self::TlsError) - certificate issues (usually persistent)
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// fn should_retry(err: &Error) -> bool {
    ///     err.is_retryable()
    /// }
    ///
    /// assert!(Error::timeout().is_retryable());
    /// assert!(Error::ConnectionError("refused".into()).is_retryable());
    /// assert!(!Error::InvalidUrl("bad".into()).is_retryable());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. } | Self::ConnectionError(_) | Self::DnsError(_)
        )
    }

    /// Returns `true` if this is a client-side configuration error.
    ///
    /// These errors indicate problems with the request setup, not network issues.
    /// Retrying won't help - the request configuration needs to be fixed.
    ///
    /// Includes:
    /// - [`InvalidUrl`](Self::InvalidUrl) - malformed URL
    /// - [`InvalidRequest`](Self::InvalidRequest) - bad headers, method, etc.
    /// - [`SsrfBlocked`](Self::SsrfBlocked) - URL blocked by SSRF protection
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::InvalidUrl("missing scheme".into());
    /// assert!(err.is_client_error());
    ///
    /// let err = Error::timeout();
    /// assert!(!err.is_client_error());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidUrl(_) | Self::InvalidRequest(_) | Self::SsrfBlocked(_)
        )
    }

    /// Returns `true` if this is a TLS/SSL certificate error.
    ///
    /// TLS errors are usually not retryable as they indicate certificate
    /// issues that won't resolve on their own.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::TlsError("certificate expired".into());
    /// assert!(err.is_tls_error());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_tls_error(&self) -> bool {
        matches!(self, Self::TlsError(_))
    }

    /// Returns `true` if this error was caused by SSRF protection.
    ///
    /// This indicates the request was blocked because the URL points to
    /// a private/internal address and `deny_private_ips()` was enabled.
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::SsrfBlocked("localhost blocked".into());
    /// assert!(err.is_ssrf_blocked());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ssrf_blocked(&self) -> bool {
        matches!(self, Self::SsrfBlocked(_))
    }

    // ========================================================================
    // Data extraction
    // ========================================================================

    /// Returns the configured timeout duration if this is a `Timeout` error.
    ///
    /// Returns `None` if:
    /// - This is not a `Timeout` error
    /// - This is a `Timeout` but the duration wasn't known (e.g., from WASI error parsing)
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::timeout_with_duration(5000);
    /// assert_eq!(err.timeout_ms(), Some(5000));
    ///
    /// let err = Error::timeout();
    /// assert_eq!(err.timeout_ms(), None);
    ///
    /// let err = Error::DnsError("failed".into());
    /// assert_eq!(err.timeout_ms(), None);
    /// ```
    #[inline]
    #[must_use]
    pub const fn timeout_ms(&self) -> Option<u64> {
        match self {
            Self::Timeout { timeout_ms } => *timeout_ms,
            _ => None,
        }
    }

    /// Returns the error message for errors that carry a message string.
    ///
    /// Returns `Some(&str)` for all variants except `Timeout` (which has no message).
    ///
    /// # Example
    ///
    /// ```
    /// use mik_sdk::http_client::Error;
    ///
    /// let err = Error::DnsError("NXDOMAIN".into());
    /// assert_eq!(err.message(), Some("NXDOMAIN"));
    ///
    /// let err = Error::timeout();
    /// assert_eq!(err.message(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::DnsError(msg)
            | Self::ConnectionError(msg)
            | Self::TlsError(msg)
            | Self::InvalidUrl(msg)
            | Self::InvalidRequest(msg)
            | Self::ResponseError(msg)
            | Self::SsrfBlocked(msg)
            | Self::Other(msg) => Some(msg),
            Self::Timeout { .. } => None,
        }
    }
}

// ============================================================================
// WASI Error Mapping
// ============================================================================

/// Pattern categories for efficient error classification.
/// Patterns are grouped by error type and checked using case-insensitive byte matching.
static DNS_PATTERNS: &[&[u8]] = &[
    b"dns",
    b"nxdomain",
    b"no such host",
    b"name resolution",
    b"resolve",
    b"getaddrinfo",
    b"could not resolve",
];

static TIMEOUT_PATTERNS: &[&[u8]] = &[b"timeout", b"timed out", b"deadline exceeded", b"etimedout"];

static TLS_PATTERNS: &[&[u8]] = &[
    b"certificate",
    b"ssl",
    b"tls",
    b"handshake failed",
    b"handshake error",
];

static CONNECTION_PATTERNS: &[&[u8]] = &[
    b"connection refused",
    b"econnrefused",
    b"connection reset",
    b"econnreset",
    b"network unreachable",
    b"enetunreach",
    b"host unreachable",
    b"ehostunreach",
    b"connection failed",
    b"failed to connect",
    b"socket error",
    b"i/o error",
    b"io error",
    b"connect error",
];

static REQUEST_PATTERNS: &[&[u8]] = &[
    b"invalid header",
    b"bad header",
    b"invalid method",
    b"unsupported method",
    b"request too large",
    b"body too large",
];

static RESPONSE_PATTERNS: &[&[u8]] = &[
    b"response error",
    b"body error",
    b"stream error",
    b"read error",
    b"payload too large",
    b"content too large",
];

/// Check if haystack contains needle (case-insensitive, no allocation).
#[inline]
fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Check if any pattern matches (case-insensitive).
#[inline]
fn matches_any(error_bytes: &[u8], patterns: &[&[u8]]) -> bool {
    patterns.iter().any(|p| contains_ci(error_bytes, p))
}

/// Maps WASI HTTP error strings to typed [`Error`] variants.
///
/// WASI HTTP runtimes return errors as strings.
/// This function parses common error patterns and maps them to the appropriate
/// typed error variant for better error handling.
///
/// # Supported Error Patterns
///
/// The function recognizes these error patterns (case-insensitive):
///
/// | Pattern | Maps To |
/// |---------|---------|
/// | `dns`, `resolve`, `nxdomain`, `no such host`, `getaddrinfo` | [`Error::DnsError`] |
/// | `timeout`, `timed out`, `deadline exceeded`, `etimedout` | [`Error::Timeout`] |
/// | `certificate`, `cert`, `ssl`, `tls`, `handshake` | [`Error::TlsError`] |
/// | `connection refused`, `econnrefused`, `connection reset`, `econnreset` | [`Error::ConnectionError`] |
/// | `network unreachable`, `host unreachable`, `enetunreach`, `ehostunreach` | [`Error::ConnectionError`] |
/// | `connection`, `socket`, `connect`, `i/o error` | [`Error::ConnectionError`] |
/// | `invalid header`, `bad header`, `invalid method` | [`Error::InvalidRequest`] |
/// | `response`, `body`, `stream`, `payload too large` | [`Error::ResponseError`] |
///
/// # Example
///
/// ```
/// use mik_sdk::http_client::{map_wasi_error, Error};
///
/// // DNS errors
/// let err = map_wasi_error("DNS lookup failed: NXDOMAIN");
/// assert!(matches!(err, Error::DnsError(_)));
///
/// // Timeout errors
/// let err = map_wasi_error("request timed out after 5000ms");
/// assert!(matches!(err, Error::Timeout { .. }));
///
/// // TLS errors
/// let err = map_wasi_error("TLS handshake failed: certificate has expired");
/// assert!(matches!(err, Error::TlsError(_)));
///
/// // Connection errors
/// let err = map_wasi_error("connection refused: 127.0.0.1:8080");
/// assert!(matches!(err, Error::ConnectionError(_)));
///
/// // Unknown errors fall through to Other
/// let err = map_wasi_error("something unexpected happened");
/// assert!(matches!(err, Error::Other(_)));
/// ```
///
/// # Usage with WASI HTTP
///
/// When integrating with `wasi:http/outgoing-handler`, use this function to
/// convert error strings to typed errors:
///
/// ```no_run
/// use mik_sdk::http_client::{map_wasi_error, Error, Response};
///
/// // In your send_with implementation:
/// fn handle_wasi_error(wasi_error: &str) -> Result<Response, Error> {
///     Err(map_wasi_error(wasi_error))
/// }
/// ```
#[must_use]
pub fn map_wasi_error(wasi_error: &str) -> Error {
    let error_bytes = wasi_error.as_bytes();

    // Order matters: more specific patterns should come before general ones

    // DNS errors - check first as they're specific
    if matches_any(error_bytes, DNS_PATTERNS) {
        return Error::DnsError(wasi_error.to_string());
    }

    // Timeout errors - check before connection errors
    if matches_any(error_bytes, TIMEOUT_PATTERNS) {
        return Error::Timeout { timeout_ms: None };
    }

    // TLS/SSL errors - check before connection errors
    // Special case: "cert " or ends with "cert"
    if matches_any(error_bytes, TLS_PATTERNS)
        || contains_ci(error_bytes, b"cert ")
        || error_bytes.len() >= 4
            && error_bytes[error_bytes.len() - 4..].eq_ignore_ascii_case(b"cert")
    {
        return Error::TlsError(wasi_error.to_string());
    }

    // Connection errors - various network-level failures
    if matches_any(error_bytes, CONNECTION_PATTERNS)
        || (contains_ci(error_bytes, b"connection") && !contains_ci(error_bytes, b"response"))
    {
        return Error::ConnectionError(wasi_error.to_string());
    }

    // Invalid request errors
    if matches_any(error_bytes, REQUEST_PATTERNS) {
        return Error::InvalidRequest(wasi_error.to_string());
    }

    // Response errors
    if matches_any(error_bytes, RESPONSE_PATTERNS)
        || (contains_ci(error_bytes, b"response") && contains_ci(error_bytes, b"failed"))
    {
        return Error::ResponseError(wasi_error.to_string());
    }

    // Default: unknown error
    Error::Other(wasi_error.to_string())
}

/// Result type for HTTP operations.
pub type Result<T> = std::result::Result<T, Error>;
