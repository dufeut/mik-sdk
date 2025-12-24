//! HTTP request builder for outbound requests.

use super::error::{Error, Result};
use super::response::Response;
use super::ssrf::{is_private_address, validate_authority, validate_percent_encoding};

// Re-export Method from request module (single source of truth)
pub use crate::request::Method;

/// URL scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// HTTP (unencrypted).
    Http,
    /// HTTPS (TLS encrypted).
    Https,
}

/// HTTP request builder.
///
/// Build a request and then send it using `send_with()` with your WASI bindings.
///
/// # Example
///
/// ```ignore
/// use bindings::wasi::http::outgoing_handler;
/// use mik_sdk::http_client;
///
/// let response = http_client::get("https://api.example.com/data")
///     .header("Accept", "application/json")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[derive(Debug, Clone)]
pub struct ClientRequest {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    timeout_ns: Option<u64>,
    deny_private_ips: bool,
}

impl ClientRequest {
    /// Create a new request with the given method and URL.
    #[must_use]
    pub fn new(method: Method, url: &str) -> Self {
        Self {
            method,
            url: url.to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ns: None,
            deny_private_ips: false,
        }
    }

    /// Add a header to the request.
    ///
    /// # Panics
    ///
    /// Panics if the header value contains CR (`\r`) or LF (`\n`) characters,
    /// which could enable header injection attacks.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        assert!(
            !value.contains('\r') && !value.contains('\n'),
            "Header value must not contain CR or LF characters (header injection)"
        );
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Forward trace ID header to outgoing request.
    ///
    /// If `trace_id` is `None`, no header is added.
    /// Use with `Request::trace_id()` to propagate trace context.
    ///
    /// ```ignore
    /// let response = fetch!(GET "https://api.example.com/data")
    ///     .with_trace_id(req.trace_id())
    ///     .send_with(&handler)?;
    /// ```
    #[must_use]
    pub fn with_trace_id(self, trace_id: Option<&str>) -> Self {
        use crate::constants::HEADER_TRACE_ID_TITLE;
        match trace_id {
            Some(id) => self.header(HEADER_TRACE_ID_TITLE, id),
            None => self,
        }
    }

    /// Set the request body.
    #[must_use]
    pub fn body(mut self, body: &[u8]) -> Self {
        self.body = Some(body.to_vec());
        self
    }

    /// Set JSON body (also sets Content-Type header).
    #[must_use]
    pub fn json(mut self, body: &[u8]) -> Self {
        use crate::constants::{HEADER_CONTENT_TYPE_TITLE, MIME_JSON};
        self.headers
            .push((HEADER_CONTENT_TYPE_TITLE.to_string(), MIME_JSON.to_string()));
        self.body = Some(body.to_vec());
        self
    }

    /// Set request timeout in milliseconds.
    ///
    /// Values over ~18 trillion ms are clamped to `u64::MAX` nanoseconds.
    #[must_use]
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ns = Some(ms.saturating_mul(1_000_000));
        self
    }

    /// Set request timeout in nanoseconds.
    #[must_use]
    pub fn timeout_ns(mut self, ns: u64) -> Self {
        self.timeout_ns = Some(ns);
        self
    }

    /// Deny requests to private/internal IP addresses (SSRF protection).
    ///
    /// When enabled, requests to the following will be rejected:
    /// - `localhost`, `127.x.x.x` (loopback)
    /// - `10.x.x.x` (private class A)
    /// - `172.16.x.x` - `172.31.x.x` (private class B)
    /// - `192.168.x.x` (private class C)
    /// - `169.254.x.x` (link-local)
    /// - `::1`, `fe80::` (IPv6 loopback/link-local)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Protect against SSRF when URL comes from user input
    /// let response = http_client::get(&user_provided_url)
    ///     .deny_private_ips()
    ///     .send_with(&outgoing_handler::handle)?;
    /// ```
    #[must_use]
    pub fn deny_private_ips(mut self) -> Self {
        self.deny_private_ips = true;
        self
    }

    /// Check if private IP denial is enabled.
    #[must_use]
    pub fn denies_private_ips(&self) -> bool {
        self.deny_private_ips
    }

    /// Get the HTTP method.
    #[must_use]
    pub fn method(&self) -> Method {
        self.method
    }

    /// Get the URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the headers.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Get the body.
    #[must_use]
    pub fn get_body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// Get the timeout in nanoseconds.
    #[must_use]
    pub fn timeout(&self) -> Option<u64> {
        self.timeout_ns
    }

    /// Check if private IPs are denied.
    #[must_use]
    pub fn is_private_ips_denied(&self) -> bool {
        self.deny_private_ips
    }

    // =========================================================================
    // Sending
    // =========================================================================

    /// Send the request using a custom sender function.
    ///
    /// This method allows you to integrate with any HTTP client by providing
    /// a sender function that takes the request data and returns a response.
    ///
    /// # Type Parameters
    ///
    /// * `F` - A function that sends the request and returns a `Result<Response, Error>`
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bindings::wasi::http::outgoing_handler;
    /// use mik_sdk::http_client::{self, Error, Response};
    ///
    /// // Define a sender that uses WASI HTTP
    /// fn wasi_send(req: &http_client::ClientRequest) -> Result<Response, Error> {
    ///     // Convert to WASI types and send...
    ///     todo!("Implement WASI HTTP sending")
    /// }
    ///
    /// let response = http_client::get("https://api.example.com/users")
    ///     .send_with(wasi_send)?;
    /// ```
    ///
    /// # For WASI HTTP
    ///
    /// When using with `wasi:http/outgoing-handler`, you need to implement
    /// the conversion between `ClientRequest` and WASI HTTP types.
    /// See the external-api example for a complete implementation.
    pub fn send_with<F>(self, sender: F) -> Result<Response>
    where
        F: FnOnce(&Self) -> Result<Response>,
    {
        // Validate URL before sending
        let _ = self.parse_url()?;
        sender(&self)
    }

    /// Parse the URL into scheme, authority, and path components.
    ///
    /// Returns `(scheme, authority, path_with_query)` tuple.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if:
    /// - URL doesn't start with `http://` or `https://`
    /// - Host is missing
    /// - Authority contains invalid characters
    /// - Port number is invalid (non-numeric or out of range)
    /// - SSRF protection is enabled and URL points to a private/internal address
    pub fn parse_url(&self) -> Result<(Scheme, String, String)> {
        // Parse scheme
        let (scheme, rest) = if self.url.starts_with("https://") {
            (Scheme::Https, &self.url[8..])
        } else if self.url.starts_with("http://") {
            (Scheme::Http, &self.url[7..])
        } else {
            return Err(Error::InvalidUrl(format!(
                "URL must start with http:// or https://: {}",
                self.url
            )));
        };

        // Split authority and path
        let (authority, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, "/"),
        };

        if authority.is_empty() {
            return Err(Error::InvalidUrl("Missing host in URL".to_string()));
        }

        // Validate authority (host and optional port)
        validate_authority(authority)?;

        // Check for private IPs if SSRF protection is enabled
        if self.deny_private_ips && is_private_address(authority) {
            return Err(Error::InvalidUrl(format!(
                "Request to private/internal address denied: {}",
                authority
            )));
        }

        // Validate percent-encoding in path
        validate_percent_encoding(path)?;

        Ok((scheme, authority.to_string(), path.to_string()))
    }
}

// ============================================================================
// Convenience constructors
// ============================================================================

/// Create a GET request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::get("https://api.example.com/users")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn get(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Get, url)
}

/// Create a POST request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::post("https://api.example.com/users")
///     .json(b"{\"name\":\"Alice\"}")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn post(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Post, url)
}

/// Create a PUT request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::put("https://api.example.com/users/123")
///     .json(b"{\"name\":\"Alice Updated\"}")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn put(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Put, url)
}

/// Create a DELETE request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::delete("https://api.example.com/users/123")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn delete(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Delete, url)
}

/// Create a PATCH request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::patch("https://api.example.com/users/123")
///     .json(b"{\"name\":\"Updated Name\"}")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn patch(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Patch, url)
}

/// Create a HEAD request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::head("https://api.example.com/large-file")
///     .send_with(&outgoing_handler::handle)?;
///
/// let content_length = response.header("content-length");
/// ```
#[must_use]
pub fn head(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Head, url)
}

/// Create an OPTIONS request.
///
/// # Example
///
/// ```ignore
/// let response = http_client::options("https://api.example.com/users")
///     .send_with(&outgoing_handler::handle)?;
///
/// let allowed = response.header("allow");
/// ```
#[must_use]
pub fn options(url: &str) -> ClientRequest {
    ClientRequest::new(Method::Options, url)
}

/// Create a request with a custom method.
///
/// # Example
///
/// ```ignore
/// let response = http_client::request(http_client::Method::Post, "https://api.example.com/data")
///     .header("Content-Type", "text/plain")
///     .body(b"Hello, World!")
///     .send_with(&outgoing_handler::handle)?;
/// ```
#[must_use]
pub fn request(method: Method, url: &str) -> ClientRequest {
    ClientRequest::new(method, url)
}
