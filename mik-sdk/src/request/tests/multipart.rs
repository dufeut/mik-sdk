//! Multipart form-data Request integration tests

use super::super::*;
use std::collections::HashMap;

#[allow(clippy::type_complexity)]
fn multipart_body(boundary: &str, parts: &[(&str, Option<&str>, Option<&str>, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, filename, content_type, data) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        if let Some(filename) = filename {
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
                )
                .as_bytes(),
            );
        } else {
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
            );
        }
        if let Some(ct) = content_type {
            body.extend_from_slice(format!("Content-Type: {ct}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--").as_bytes());
    body
}

#[test]
fn test_is_multipart() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "multipart/form-data; boundary=----Boundary".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert!(req.is_multipart());
}

#[test]
fn test_is_not_multipart() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        None,
        HashMap::new(),
    );

    assert!(!req.is_multipart());
}

#[test]
fn test_multipart_single_field() {
    let boundary = "----Boundary";
    let body = multipart_body(boundary, &[("field1", None, None, b"value1")]);

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let parts = req.multipart().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name(), "field1");
    assert_eq!(parts[0].text(), Some("value1"));
    assert!(parts[0].filename().is_none());
    assert!(parts[0].content_type().is_none());
}

#[test]
fn test_multipart_file_upload() {
    let boundary = "----Boundary";
    let body = multipart_body(
        boundary,
        &[(
            "file",
            Some("test.txt"),
            Some("text/plain"),
            b"file content",
        )],
    );

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let parts = req.multipart().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name(), "file");
    assert_eq!(parts[0].filename(), Some("test.txt"));
    assert_eq!(parts[0].content_type(), Some("text/plain"));
    assert_eq!(parts[0].text(), Some("file content"));
}

#[test]
fn test_multipart_multiple_parts() {
    let boundary = "----Boundary";
    let body = multipart_body(
        boundary,
        &[
            ("name", None, None, b"John"),
            (
                "file",
                Some("doc.pdf"),
                Some("application/pdf"),
                b"PDF data",
            ),
        ],
    );

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let parts = req.multipart().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].name(), "name");
    assert_eq!(parts[0].text(), Some("John"));
    assert_eq!(parts[1].name(), "file");
    assert_eq!(parts[1].filename(), Some("doc.pdf"));
}

#[test]
fn test_multipart_error_not_multipart() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        Some(b"{}".to_vec()),
        HashMap::new(),
    );

    let result = req.multipart();
    // application/json doesn't have a boundary, so NoBoundary is returned
    assert_eq!(result, Err(MultipartError::NoBoundary));
}

#[test]
fn test_multipart_error_no_content_type() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![],
        Some(b"data".to_vec()),
        HashMap::new(),
    );

    let result = req.multipart();
    assert_eq!(result, Err(MultipartError::NotMultipart));
}

#[test]
fn test_multipart_error_no_boundary() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "multipart/form-data".to_string(), // No boundary
        )],
        Some(b"data".to_vec()),
        HashMap::new(),
    );

    let result = req.multipart();
    assert_eq!(result, Err(MultipartError::NoBoundary));
}

#[test]
fn test_multipart_error_no_body() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "multipart/form-data; boundary=----Boundary".to_string(),
        )],
        None, // No body
        HashMap::new(),
    );

    let result = req.multipart();
    assert_eq!(result, Err(MultipartError::NoBody));
}

#[test]
fn test_multipart_is_multipart_case_insensitive() {
    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "Content-Type".to_string(),
            "Multipart/Form-Data; boundary=----Boundary".to_string(),
        )],
        None,
        HashMap::new(),
    );

    assert!(req.is_multipart());
}

#[test]
fn test_multipart_binary_data_valid_utf8() {
    // Test with data that happens to be valid UTF-8 (null bytes are valid UTF-8)
    let boundary = "----Boundary";
    let body = multipart_body(boundary, &[("data", None, None, b"\x00\x01\x02\x03")]);

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    // Valid UTF-8 binary data should parse successfully
    let parts = req.multipart().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].data(), b"\x00\x01\x02\x03");
    // text() returns None for non-printable but still valid UTF-8
    assert!(parts[0].text().is_some());
}

#[test]
fn test_multipart_binary_data_invalid_utf8() {
    // Test with truly invalid UTF-8 data
    let boundary = "----Boundary";
    // Build body manually with invalid UTF-8 in the content
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"data\"\r\n\r\n");
    body.extend_from_slice(&[0x80, 0x81, 0x82]); // Invalid UTF-8 (continuation bytes without lead)
    body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    // Invalid UTF-8 in body causes InvalidFormat because we parse as str
    let result = req.multipart();
    assert_eq!(result, Err(MultipartError::InvalidFormat));
}

#[test]
fn test_multipart_empty_parts() {
    let boundary = "----Boundary";
    let body = format!("--{boundary}--").into_bytes();

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let parts = req.multipart().unwrap();
    assert!(parts.is_empty());
}

#[test]
fn test_multipart_too_many_parts() {
    use crate::constants::MAX_MULTIPART_PARTS;

    let boundary = "----Boundary";

    // Build body with MAX_MULTIPART_PARTS + 1 parts
    let mut body = Vec::new();
    for i in 0..=MAX_MULTIPART_PARTS {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"field{i}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(format!("value{i}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}--").as_bytes());

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let result = req.multipart();
    assert_eq!(result, Err(MultipartError::TooManyParts));
}

#[test]
fn test_multipart_exactly_max_parts() {
    use crate::constants::MAX_MULTIPART_PARTS;

    let boundary = "----Boundary";

    // Build body with exactly MAX_MULTIPART_PARTS parts (should succeed)
    let mut body = Vec::new();
    for i in 0..MAX_MULTIPART_PARTS {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"field{i}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(format!("value{i}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}--").as_bytes());

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={boundary}"),
        )],
        Some(body),
        HashMap::new(),
    );

    let parts = req.multipart().unwrap();
    assert_eq!(parts.len(), MAX_MULTIPART_PARTS);
}
