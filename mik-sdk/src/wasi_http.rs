//! WASI bindings and HTTP client implementation.
//!
//! This module provides WASI functionality using native bindings:
//! - HTTP client with `.send()` method
//! - Random number generation via `wasi:random/random`
//! - Wall clock access via `wasi:clocks/wall-clock`
//!
//! Only available when the `wasi-http` feature is enabled.

#![allow(warnings)]

// Generate bindings for WASI interfaces
// This creates public modules: wasi::http, wasi::random, wasi::clocks, etc.
wit_bindgen::generate!({
    path: "wit",
    world: "sdk",
    generate_all,
});

use wasi::http::outgoing_handler;
use wasi::http::types as http_types;
use wasi::io::streams::StreamError;

use crate::http_client::{ClientRequest, Error, Method, Response, Result, Scheme};

impl ClientRequest {
    /// Send the HTTP request using WASI HTTP.
    ///
    /// This method uses `wasi:http/outgoing-handler` to make the actual HTTP request.
    /// It requires the `wasi-http` feature to be enabled.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // This example requires a WASI runtime environment
    /// use mik_sdk::http_client;
    ///
    /// let response = http_client::get("https://api.example.com/users")
    ///     .header("Authorization", "Bearer token")
    ///     .send()?;
    ///
    /// if response.is_success() {
    ///     let body = response.body();
    ///     // Process response...
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL is invalid
    /// - DNS resolution fails
    /// - The connection cannot be established
    /// - The request times out
    /// - TLS handshake fails
    /// - SSRF protection blocks a private IP address
    pub fn send(self) -> Result<Response> {
        // Validate URL and check for private IPs if configured
        let (scheme, authority, path) = self.parse_url()?;

        // Check SSRF protection
        if self.is_private_ips_denied() {
            // Extract host from authority (remove port if present)
            let host = authority.split(':').next().unwrap_or(&authority);
            if crate::http_client::is_private_address(host) {
                return Err(Error::SsrfBlocked(format!(
                    "Request to private/internal address blocked: {}",
                    host
                )));
            }
        }

        // Create headers
        let headers = http_types::Fields::new();
        for (name, value) in self.headers() {
            headers
                .append(
                    &http_types::FieldKey::from(name.as_str()),
                    &value.as_bytes().to_vec(),
                )
                .map_err(|e| Error::InvalidRequest(format!("Invalid header: {:?}", e)))?;
        }

        // Create outgoing request
        let outgoing_req = http_types::OutgoingRequest::new(headers);

        // Set method
        let wasi_method = match self.method() {
            Method::Get => http_types::Method::Get,
            Method::Post => http_types::Method::Post,
            Method::Put => http_types::Method::Put,
            Method::Delete => http_types::Method::Delete,
            Method::Patch => http_types::Method::Patch,
            Method::Head => http_types::Method::Head,
            Method::Options => http_types::Method::Options,
        };
        outgoing_req
            .set_method(&wasi_method)
            .map_err(|()| Error::InvalidRequest("Failed to set method".into()))?;

        // Set scheme
        let wasi_scheme = match scheme {
            Scheme::Http => http_types::Scheme::Http,
            Scheme::Https => http_types::Scheme::Https,
        };
        outgoing_req
            .set_scheme(Some(&wasi_scheme))
            .map_err(|()| Error::InvalidRequest("Failed to set scheme".into()))?;

        // Set authority (host:port)
        outgoing_req
            .set_authority(Some(&authority))
            .map_err(|()| Error::InvalidRequest("Failed to set authority".into()))?;

        // Set path with query
        outgoing_req
            .set_path_with_query(Some(&path))
            .map_err(|()| Error::InvalidRequest("Failed to set path".into()))?;

        // Set body if present
        if let Some(body_bytes) = self.body_bytes() {
            let body = outgoing_req
                .body()
                .map_err(|()| Error::InvalidRequest("Failed to get body handle".into()))?;
            let stream = body
                .write()
                .map_err(|()| Error::InvalidRequest("Failed to get write stream".into()))?;
            stream
                .blocking_write_and_flush(body_bytes)
                .map_err(|e| Error::InvalidRequest(format!("Failed to write body: {:?}", e)))?;
            drop(stream);
            http_types::OutgoingBody::finish(body, None)
                .map_err(|e| Error::InvalidRequest(format!("Failed to finish body: {:?}", e)))?;
        }

        // Build request options with timeout
        let options = if let Some(timeout_ns) = self.timeout() {
            let opts = http_types::RequestOptions::new();
            opts.set_connect_timeout(Some(timeout_ns))
                .map_err(|()| Error::InvalidRequest("Failed to set connect timeout".into()))?;
            opts.set_first_byte_timeout(Some(timeout_ns))
                .map_err(|()| Error::InvalidRequest("Failed to set first byte timeout".into()))?;
            Some(opts)
        } else {
            None
        };

        // Send request
        let future_response = outgoing_handler::handle(outgoing_req, options)
            .map_err(|e| Error::ConnectionError(format!("Failed to send request: {:?}", e)))?;

        // Wait for response (blocking)
        let incoming_response = loop {
            match future_response.get() {
                Some(result) => {
                    break result
                        .map_err(|()| Error::ConnectionError("Response already consumed".into()))?
                        .map_err(|e| crate::http_client::map_wasi_error(&format!("{:?}", e)))?;
                },
                None => {
                    // Poll again
                    future_response.subscribe().block();
                },
            }
        };

        // Read response status
        let status = incoming_response.status();

        // Read response headers
        let response_headers = incoming_response.headers();
        let header_entries: Vec<(String, String)> = response_headers
            .entries()
            .into_iter()
            .filter_map(|(k, v)| String::from_utf8(v).ok().map(|v| (k, v)))
            .collect();

        // Read response body
        let body = incoming_response
            .consume()
            .map_err(|()| Error::ResponseError("Failed to consume body".into()))?;
        let body_stream = body
            .stream()
            .map_err(|()| Error::ResponseError("Failed to get body stream".into()))?;

        let mut body_bytes = Vec::new();
        loop {
            match body_stream.read(64 * 1024) {
                Ok(chunk) => {
                    if chunk.is_empty() {
                        break;
                    }
                    body_bytes.extend_from_slice(&chunk);
                },
                Err(StreamError::Closed) => break,
                Err(e) => {
                    return Err(Error::ResponseError(format!(
                        "Failed to read body: {:?}",
                        e
                    )));
                },
            }
        }

        Ok(Response::new(status, header_entries, body_bytes))
    }
}
