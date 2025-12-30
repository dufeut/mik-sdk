//! HTTP response from outbound requests.

use std::collections::HashMap;

/// HTTP response from an outbound request.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Response body bytes.
    body: Vec<u8>,
    /// Index map for O(1) header lookup (lowercase keys -> indices in headers vec).
    header_index: HashMap<String, Vec<usize>>,
}

impl Response {
    /// Create a new response.
    #[must_use]
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        // Build index map for O(1) header lookups
        let mut header_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, (k, _)) in headers.iter().enumerate() {
            header_index.entry(k.to_lowercase()).or_default().push(i);
        }

        Self {
            status,
            headers,
            body,
            header_index,
        }
    }

    /// Get response body as bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// Get response body as UTF-8 string (borrowed).
    ///
    /// # Returns
    ///
    /// - `Some(&str)` - Body successfully decoded as UTF-8
    /// - `None` - Body is not valid UTF-8
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }

    /// Check if response is successful (2xx).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Check if response is a client error (4xx).
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Check if response is a server error (5xx).
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }

    /// Get the HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Get response body as owned bytes (consuming).
    ///
    /// Use this when you need to own the body bytes. For borrowing, use [`bytes()`](Self::bytes).
    #[must_use]
    pub fn body(self) -> Vec<u8> {
        self.body
    }

    /// Get response headers as slice of (name, value) tuples.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Get a header value by name (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        // Fast path: if name is already lowercase, avoid allocation
        let indices = if name.bytes().all(|b| !b.is_ascii_uppercase()) {
            self.header_index.get(name)
        } else {
            self.header_index.get(&name.to_lowercase())
        };

        indices
            .and_then(|idx| idx.first())
            .and_then(|&i| self.headers.get(i))
            .map(|(_, v)| v.as_str())
    }

    /// Get all header values by name (case-insensitive).
    #[must_use]
    pub fn header_all(&self, name: &str) -> Vec<&str> {
        // Fast path: if name is already lowercase, avoid allocation
        let indices = if name.bytes().all(|b| !b.is_ascii_uppercase()) {
            self.header_index.get(name)
        } else {
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
}
