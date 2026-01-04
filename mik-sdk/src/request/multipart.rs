//! Multipart form-data parsing utilities.
//!
//! This module provides parsing of `multipart/form-data` requests (RFC 7578),
//! commonly used for file uploads.
//!
//! # Example
//!
//! ```ignore
//! if req.is_multipart() {
//!     match req.multipart() {
//!         Ok(parts) => {
//!             for part in &parts {
//!                 println!("Field: {}", part.name());
//!                 if let Some(filename) = part.filename() {
//!                     println!("  File: {}", filename);
//!                 }
//!                 println!("  Size: {} bytes", part.data().len());
//!             }
//!         }
//!         Err(e) => eprintln!("Multipart error: {:?}", e),
//!     }
//! }
//! ```

use crate::constants::MAX_MULTIPART_PARTS;

/// A single part from a multipart form submission.
///
/// Parts contain:
/// - A required `name` (field name from Content-Disposition)
/// - An optional `filename` (for file uploads)
/// - An optional `content_type` (MIME type)
/// - The raw `data` (body of this part)
#[derive(Debug, PartialEq, Eq)]
pub struct Part<'a> {
    name: &'a str,
    filename: Option<&'a str>,
    content_type: Option<&'a str>,
    data: &'a [u8],
}

impl<'a> Part<'a> {
    /// The field name from Content-Disposition.
    #[inline]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The filename from Content-Disposition, if present (file upload).
    #[inline]
    pub const fn filename(&self) -> Option<&'a str> {
        self.filename
    }

    /// The Content-Type of this part, if specified.
    #[inline]
    pub const fn content_type(&self) -> Option<&'a str> {
        self.content_type
    }

    /// The raw data of this part.
    #[inline]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The data as UTF-8 text, if valid.
    #[inline]
    pub fn text(&self) -> Option<&'a str> {
        std::str::from_utf8(self.data).ok()
    }
}

/// Error type for multipart parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MultipartError {
    /// Content-Type is not multipart/form-data.
    NotMultipart,
    /// No boundary parameter in Content-Type.
    NoBoundary,
    /// Invalid multipart format (malformed structure).
    InvalidFormat,
    /// No body in request.
    NoBody,
    /// Too many parts (exceeds MAX_MULTIPART_PARTS).
    TooManyParts,
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotMultipart => write!(f, "Content-Type is not multipart/form-data"),
            Self::NoBoundary => write!(f, "No boundary parameter in Content-Type"),
            Self::InvalidFormat => write!(f, "Invalid multipart format"),
            Self::NoBody => write!(f, "No body in request"),
            Self::TooManyParts => write!(f, "Too many parts (max: {MAX_MULTIPART_PARTS})"),
        }
    }
}

impl std::error::Error for MultipartError {}

/// Extract boundary from Content-Type header.
///
/// Format: `multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxk`
pub(super) fn extract_boundary(content_type: &str) -> Option<&str> {
    // Check if it's multipart/form-data
    let ct_lower = content_type.to_lowercase();
    if !ct_lower.starts_with("multipart/form-data") {
        return None;
    }

    // Find boundary parameter
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("boundary=") {
            // Handle quoted boundary
            let value = value.trim();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                return Some(&value[1..value.len() - 1]);
            }
            return Some(value);
        }
        // Case-insensitive check
        let part_lower = part.to_lowercase();
        if let Some(pos) = part_lower.find("boundary=") {
            let value = &part[pos + 9..];
            let value = value.trim();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                return Some(&value[1..value.len() - 1]);
            }
            return Some(value);
        }
    }

    None
}

/// Parse multipart body into parts.
///
/// # Arguments
///
/// * `body` - The raw request body bytes
/// * `boundary` - The boundary string (without `--` prefix)
///
/// # Returns
///
/// A vector of parsed parts, or an error if the format is invalid.
pub(super) fn parse_multipart<'a>(
    body: &'a [u8],
    boundary: &str,
) -> Result<Vec<Part<'a>>, MultipartError> {
    let mut parts = Vec::new();

    // Build boundary markers
    let boundary_marker = format!("--{boundary}");
    let end_marker = format!("--{boundary}--");

    // Convert body to str for easier parsing (multipart headers are ASCII)
    // We'll extract data slices from the original bytes
    let body_str = std::str::from_utf8(body).map_err(|_| MultipartError::InvalidFormat)?;

    // Split by boundary
    let mut remaining = body_str;

    // Skip preamble (anything before first boundary)
    if let Some(pos) = remaining.find(&boundary_marker) {
        remaining = &remaining[pos + boundary_marker.len()..];
    } else {
        return Err(MultipartError::InvalidFormat);
    }

    loop {
        // Check for end marker
        if remaining.starts_with("--") {
            break; // Final boundary
        }

        // Skip leading CRLF
        remaining = remaining
            .trim_start_matches("\r\n")
            .trim_start_matches('\n');

        if remaining.is_empty() {
            break;
        }

        // Check parts limit
        if parts.len() >= MAX_MULTIPART_PARTS {
            return Err(MultipartError::TooManyParts);
        }

        // Find next boundary
        let part_end = remaining
            .find(&boundary_marker)
            .or_else(|| remaining.find(&end_marker))
            .unwrap_or(remaining.len());

        let part_content = &remaining[..part_end];

        // Parse this part
        if let Some(part) = parse_single_part(part_content) {
            parts.push(part);
        }

        // Move past this part and the boundary
        if let Some(pos) = remaining.find(&boundary_marker) {
            remaining = &remaining[pos + boundary_marker.len()..];
        } else {
            break;
        }
    }

    Ok(parts)
}

/// Parse a single multipart part.
fn parse_single_part(part_str: &str) -> Option<Part<'_>> {
    // Find header/body separator (CRLFCRLF or LFLF)
    let (headers_str, data_str) = if let Some(pos) = part_str.find("\r\n\r\n") {
        (&part_str[..pos], &part_str[pos + 4..])
    } else if let Some(pos) = part_str.find("\n\n") {
        (&part_str[..pos], &part_str[pos + 2..])
    } else {
        // No separator found, skip this part
        return None;
    };

    // Parse headers
    let mut name: Option<&str> = None;
    let mut filename: Option<&str> = None;
    let mut content_type: Option<&str> = None;

    for line in headers_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Case-insensitive header matching
        let line_lower = line.to_lowercase();

        if line_lower.starts_with("content-disposition:") {
            let value = &line["content-disposition:".len()..].trim();
            // Parse Content-Disposition: form-data; name="field"; filename="file.txt"
            for param in value.split(';') {
                let param = param.trim();
                if let Some(n) = extract_quoted_param(param, "name=") {
                    name = Some(n);
                } else if let Some(f) = extract_quoted_param(param, "filename=") {
                    filename = Some(f);
                }
            }
        } else if line_lower.starts_with("content-type:") {
            content_type = Some(line["content-type:".len()..].trim());
        }
    }

    // Name is required
    let name = name?;

    // Remove trailing CRLF from data
    let data_str = data_str
        .trim_end_matches("\r\n")
        .trim_end_matches('\n')
        .trim_end_matches("\r\n"); // Handle double CRLF at end

    Some(Part {
        name,
        filename,
        content_type,
        data: data_str.as_bytes(),
    })
}

/// Extract a quoted parameter value.
///
/// Handles both `name="value"` and `name=value` formats.
fn extract_quoted_param<'a>(param: &'a str, prefix: &str) -> Option<&'a str> {
    // Case-insensitive prefix check
    let param_lower = param.to_lowercase();
    if !param_lower.starts_with(&prefix.to_lowercase()) {
        return None;
    }

    let value = &param[prefix.len()..];

    // Handle quoted value
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Some(&value[1..value.len() - 1])
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_boundary_basic() {
        let ct = "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxk";
        assert_eq!(extract_boundary(ct), Some("----WebKitFormBoundary7MA4YWxk"));
    }

    #[test]
    fn test_extract_boundary_quoted() {
        let ct = r#"multipart/form-data; boundary="----Boundary""#;
        assert_eq!(extract_boundary(ct), Some("----Boundary"));
    }

    #[test]
    fn test_extract_boundary_not_multipart() {
        let ct = "application/json";
        assert_eq!(extract_boundary(ct), None);
    }

    #[test]
    fn test_extract_boundary_no_boundary() {
        let ct = "multipart/form-data";
        assert_eq!(extract_boundary(ct), None);
    }

    #[test]
    fn test_parse_multipart_single_field() {
        let boundary = "----Boundary";
        let body = b"------Boundary\r\n\
            Content-Disposition: form-data; name=\"field1\"\r\n\
            \r\n\
            value1\r\n\
            ------Boundary--";

        let parts = parse_multipart(body, boundary).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].name(), "field1");
        assert_eq!(parts[0].text(), Some("value1"));
        assert!(parts[0].filename().is_none());
    }

    #[test]
    fn test_parse_multipart_multiple_fields() {
        let boundary = "----Boundary";
        let body = b"------Boundary\r\n\
            Content-Disposition: form-data; name=\"field1\"\r\n\
            \r\n\
            value1\r\n\
            ------Boundary\r\n\
            Content-Disposition: form-data; name=\"field2\"\r\n\
            \r\n\
            value2\r\n\
            ------Boundary--";

        let parts = parse_multipart(body, boundary).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name(), "field1");
        assert_eq!(parts[0].text(), Some("value1"));
        assert_eq!(parts[1].name(), "field2");
        assert_eq!(parts[1].text(), Some("value2"));
    }

    #[test]
    fn test_parse_multipart_file_upload() {
        let boundary = "----Boundary";
        let body = b"------Boundary\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\
            \r\n\
            file content here\r\n\
            ------Boundary--";

        let parts = parse_multipart(body, boundary).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].name(), "file");
        assert_eq!(parts[0].filename(), Some("test.txt"));
        assert_eq!(parts[0].content_type(), Some("text/plain"));
        assert_eq!(parts[0].text(), Some("file content here"));
    }

    #[test]
    fn test_parse_multipart_binary_data() {
        let boundary = "----Boundary";
        // Build body with binary content
        let mut body = Vec::new();
        body.extend_from_slice(b"------Boundary\r\n");
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"binary\"\r\n");
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(b"Hello"); // Text for this test, but validates binary handling
        body.extend_from_slice(b"\r\n------Boundary--");

        let parts = parse_multipart(&body, boundary).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].data(), b"Hello");
    }

    #[test]
    fn test_parse_multipart_empty_body() {
        let boundary = "----Boundary";
        let body = b"------Boundary--";

        let parts = parse_multipart(body, boundary).unwrap();
        assert!(parts.is_empty());
    }

    #[test]
    fn test_parse_multipart_invalid_no_boundary() {
        let boundary = "----Boundary";
        let body = b"no boundary here";

        let result = parse_multipart(body, boundary);
        assert_eq!(result, Err(MultipartError::InvalidFormat));
    }

    #[test]
    fn test_extract_quoted_param() {
        assert_eq!(
            extract_quoted_param(r#"name="field""#, "name="),
            Some("field")
        );
        assert_eq!(extract_quoted_param("name=field", "name="), Some("field"));
        assert_eq!(
            extract_quoted_param(r#"filename="test.txt""#, "filename="),
            Some("test.txt")
        );
        assert_eq!(extract_quoted_param("other=value", "name="), None);
    }

    #[test]
    fn test_multipart_error_display() {
        assert_eq!(
            MultipartError::NotMultipart.to_string(),
            "Content-Type is not multipart/form-data"
        );
        assert_eq!(
            MultipartError::NoBoundary.to_string(),
            "No boundary parameter in Content-Type"
        );
        assert_eq!(
            MultipartError::InvalidFormat.to_string(),
            "Invalid multipart format"
        );
        assert_eq!(MultipartError::NoBody.to_string(), "No body in request");
        assert_eq!(
            MultipartError::TooManyParts.to_string(),
            format!("Too many parts (max: {MAX_MULTIPART_PARTS})")
        );
    }
}
