//! Body, text, and json_with tests

use super::super::*;
use std::collections::HashMap;

#[test]
fn test_body_empty() {
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![],
        Some(vec![]),
        HashMap::new(),
    );

    assert_eq!(req.body(), Some(&[][..]));
    assert_eq!(req.text(), Some(""));
    assert!(!req.has_body()); // Empty body returns false
}

#[test]
fn test_body_none() {
    let req = Request::new(Method::Get, "/".to_string(), vec![], None, HashMap::new());

    assert_eq!(req.body(), None);
    assert_eq!(req.text(), None);
    assert!(!req.has_body());
}

#[test]
fn test_body_invalid_utf8() {
    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![],
        Some(vec![0xFF, 0xFE, 0x00, 0x01]), // Invalid UTF-8
        HashMap::new(),
    );

    assert!(req.body().is_some());
    assert_eq!(req.text(), None); // Should return None for invalid UTF-8
    assert!(req.has_body());
}

#[test]
fn test_large_body_1mb() {
    // 1MB body - typical large JSON payload
    let size = 1024 * 1024;
    let body: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![("content-length".to_string(), size.to_string())],
        Some(body),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), size);
}

#[test]
fn test_large_body_json_text() {
    // Large valid UTF-8 JSON body
    let size = 512 * 1024; // 512KB
    let json_body = format!(
        r#"{{"data": "{}"}}"#,
        "x".repeat(size - 15) // Subtract JSON overhead
    );
    let body_bytes = json_body.as_bytes().to_vec();

    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("content-length".to_string(), body_bytes.len().to_string()),
        ],
        Some(body_bytes),
        HashMap::new(),
    );

    assert!(req.is_json());
    assert!(req.text().is_some());
    assert!(req.text().unwrap().starts_with(r#"{"data": ""#));
}

#[test]
fn test_large_body_binary() {
    // Large binary body (invalid UTF-8)
    let size = 256 * 1024; // 256KB
    let body: Vec<u8> = (0..size).map(|i| ((i * 7) % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload/binary".to_string(),
        vec![
            (
                "content-type".to_string(),
                "application/octet-stream".to_string(),
            ),
            ("content-length".to_string(), size.to_string()),
        ],
        Some(body),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), size);
    // text() should return None for binary data
    assert!(req.text().is_none());
}

#[test]
fn test_body_boundary_sizes() {
    // Test at common buffer size boundaries
    let sizes = [
        0, 1, 63, 64, 65, // 64-byte boundary
        255, 256, 257, // 256-byte boundary
        1023, 1024, 1025, // 1KB boundary
        4095, 4096, 4097, // 4KB (page size) boundary
        65535, 65536, 65537, // 64KB boundary
    ];

    for size in sizes {
        let body: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let req = Request::new(
            Method::Post,
            "/test".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        let body_opt = req.body();
        assert!(body_opt.is_some(), "Body should exist for size {size}");
        assert_eq!(
            body_opt.unwrap().len(),
            size,
            "Body size mismatch for {size}"
        );
    }
}

#[test]
fn test_body_all_byte_values() {
    // Ensure all 256 byte values are handled correctly
    let body: Vec<u8> = (0..=255u8).collect();

    let req = Request::new(
        Method::Post,
        "/binary".to_string(),
        vec![],
        Some(body),
        HashMap::new(),
    );

    let received = req.body().unwrap();
    assert_eq!(received.len(), 256);
    for (i, &byte) in received.iter().enumerate() {
        assert_eq!(byte, i as u8, "Byte mismatch at position {i}");
    }
}

#[test]
fn test_body_repeated_pattern() {
    // Test with repeated patterns that might cause issues with compression/dedup
    let pattern = b"ABCDEFGH".repeat(10000); // 80KB of repeated pattern

    let req = Request::new(
        Method::Post,
        "/pattern".to_string(),
        vec![],
        Some(pattern.clone()),
        HashMap::new(),
    );

    assert_eq!(req.body().unwrap(), pattern.as_slice());
}

#[test]
fn test_malformed_body_not_utf8() {
    // Body with invalid UTF-8 sequences
    let invalid_utf8 = vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0x81];

    let req = Request::new(
        Method::Post,
        "/".to_string(),
        vec![("content-type".to_string(), "text/plain".to_string())],
        Some(invalid_utf8),
        HashMap::new(),
    );

    // body() works, text() returns None
    assert!(req.body().is_some());
    assert!(req.text().is_none());
}

#[test]
fn test_malformed_body_truncated_utf8() {
    // Truncated UTF-8 sequences
    let truncated_sequences = [
        vec![0xC2],             // Truncated 2-byte sequence
        vec![0xE0, 0xA0],       // Truncated 3-byte sequence
        vec![0xF0, 0x90, 0x80], // Truncated 4-byte sequence
    ];

    for body in truncated_sequences {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic, text() returns None for invalid UTF-8
        assert!(req.body().is_some());
        assert!(
            req.text().is_none(),
            "Should return None for truncated UTF-8: {body:?}"
        );
    }
}

#[test]
fn test_malformed_overlong_utf8() {
    // Overlong UTF-8 encodings (security issue in some parsers)
    let overlong_sequences = [
        vec![0xC0, 0xAF],       // Overlong '/' (should be 0x2F)
        vec![0xE0, 0x80, 0xAF], // Overlong '/' 3-byte
        vec![0xC1, 0xBF],       // Overlong (invalid)
    ];

    for body in overlong_sequences {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic
        let _body = req.body();
        let _text = req.text();
    }
}

#[test]
fn test_malformed_surrogate_pairs() {
    // Invalid surrogate pairs in body
    let invalid_surrogates = [
        vec![0xED, 0xA0, 0x80], // High surrogate alone (U+D800)
        vec![0xED, 0xBF, 0xBF], // Low surrogate alone (U+DFFF)
    ];

    for body in invalid_surrogates {
        let req = Request::new(
            Method::Post,
            "/".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        // Should not panic, text() returns None
        assert!(req.body().is_some());
        assert!(req.text().is_none());
    }
}

#[test]
fn test_garbage_binary_body() {
    // Random garbage bytes
    let garbage: Vec<u8> = (0..1000).map(|i| ((i * 17 + 31) % 256) as u8).collect();

    let req = Request::new(
        Method::Post,
        "/upload".to_string(),
        vec![(
            "content-type".to_string(),
            "application/octet-stream".to_string(),
        )],
        Some(garbage),
        HashMap::new(),
    );

    assert!(req.has_body());
    assert_eq!(req.body().unwrap().len(), 1000);
}

#[test]
fn test_json_with_success() {
    // Test that json_with returns Some when parsing succeeds
    let json_body = br#"{"name": "test", "value": 42}"#;

    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![("content-type".to_string(), "application/json".to_string())],
        Some(json_body.to_vec()),
        HashMap::new(),
    );

    // Simple parser that extracts the raw bytes
    let result = req.json_with(|bytes| {
        // Verify we got the right bytes and return them
        if bytes == json_body {
            Some(bytes.to_vec())
        } else {
            None
        }
    });

    // json_with must return Some when the parser returns Some
    assert!(
        result.is_some(),
        "json_with should return Some on successful parse"
    );
    assert_eq!(result.unwrap(), json_body.to_vec());
}

#[test]
fn test_json_with_parser_returns_value() {
    // Test that the parsed value is correctly returned
    let json_body = b"123";

    let req = Request::new(
        Method::Post,
        "/api/number".to_string(),
        vec![],
        Some(json_body.to_vec()),
        HashMap::new(),
    );

    // Parser that extracts a number
    let result = req.json_with(|bytes| std::str::from_utf8(bytes).ok()?.parse::<i32>().ok());

    assert_eq!(
        result,
        Some(123),
        "json_with should return the parsed value"
    );
}

#[test]
fn test_json_with_no_body() {
    // Test that json_with returns None when there's no body
    let req = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    let result = req.json_with(|_| Some(42));
    assert!(
        result.is_none(),
        "json_with should return None when body is missing"
    );
}

#[test]
fn test_json_with_parser_returns_none() {
    // Test that json_with returns None when parser returns None
    let req = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![],
        Some(b"invalid".to_vec()),
        HashMap::new(),
    );

    let result = req.json_with(|_| None::<i32>);
    assert!(
        result.is_none(),
        "json_with should return None when parser returns None"
    );
}

#[test]
fn test_body_exactly_at_common_limits() {
    // Test bodies at exact power-of-2 boundaries
    for size in [1024, 4096, 8192, 16384, 32768, 65536] {
        let body = vec![b'x'; size];
        let req = Request::new(
            Method::Post,
            "/upload".to_string(),
            vec![],
            Some(body.clone()),
            HashMap::new(),
        );

        assert_eq!(req.body().map(<[u8]>::len), Some(size));
        assert_eq!(req.text().map(str::len), Some(size));
    }
}

#[test]
fn test_body_just_under_and_over_limits() {
    // Test bodies at boundary-1 and boundary+1
    for boundary in [4096i32, 65536] {
        for offset in [-1i32, 0, 1] {
            let size = (boundary + offset) as usize;
            let body = vec![b'a'; size];
            let req = Request::new(
                Method::Post,
                "/upload".to_string(),
                vec![],
                Some(body),
                HashMap::new(),
            );

            assert_eq!(req.body().map(<[u8]>::len), Some(size));
        }
    }
}
