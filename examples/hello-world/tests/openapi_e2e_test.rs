#![allow(clippy::unwrap_used, clippy::expect_used, clippy::doc_markdown)]
//! End-to-end OpenAPI schema validation test.
//!
//! This test validates that the actual generated openapi.json from the routes! macro
//! is structurally valid OpenAPI 3.0 and contains expected content.
//!
//! Run with: `cargo test -p hello-world openapi_e2e`

use openapiv3::OpenAPI;

/// Test that the generated openapi.json is valid OpenAPI 3.0.
#[test]
fn test_generated_schema_is_valid_openapi() {
    // Read the generated schema
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path)
        .expect("openapi.json should exist - run `cargo test __mik_write_schema` first");

    // Parse with openapiv3 - this validates structural correctness
    let spec: OpenAPI = serde_json::from_str(&json).expect("Schema should be valid OpenAPI 3.0");

    // Verify basic structure
    assert_eq!(spec.openapi, "3.0.0");
    assert_eq!(spec.info.title, "hello-world");
    assert_eq!(spec.info.version, "0.0.2");
}

/// Test that all expected paths are present.
#[test]
fn test_schema_has_expected_paths() {
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path).expect("openapi.json should exist");
    let spec: OpenAPI = serde_json::from_str(&json).expect("Schema should be valid");

    let paths = &spec.paths.paths;
    assert!(paths.contains_key("/"), "Should have root path");
    assert!(
        paths.contains_key("/hello/{name}"),
        "Should have /hello/{{name}} path"
    );
    assert!(paths.contains_key("/echo"), "Should have /echo path");
    assert!(paths.contains_key("/search"), "Should have /search path");
}

/// Test that components contain all expected schemas.
#[test]
fn test_schema_has_expected_components() {
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path).expect("openapi.json should exist");
    let spec: OpenAPI = serde_json::from_str(&json).expect("Schema should be valid");

    let schemas = spec.components.expect("Should have components").schemas;

    // Type schemas
    assert!(
        schemas.contains_key("HomeResponse"),
        "Should have HomeResponse"
    );
    assert!(
        schemas.contains_key("HelloResponse"),
        "Should have HelloResponse"
    );
    assert!(schemas.contains_key("EchoInput"), "Should have EchoInput");
    assert!(
        schemas.contains_key("EchoResponse"),
        "Should have EchoResponse"
    );
    assert!(
        schemas.contains_key("SearchResponse"),
        "Should have SearchResponse"
    );

    // RFC 7807 error schema
    assert!(
        schemas.contains_key("ProblemDetails"),
        "Should have ProblemDetails for error responses"
    );
}

/// Test that operations have tags from path prefixes.
#[test]
fn test_schema_has_tags() {
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path).expect("openapi.json should exist");

    // Check that tags are present in the JSON
    assert!(json.contains(r#""tags":["Echo"]"#), "Should have Echo tag");
    assert!(
        json.contains(r#""tags":["Hello"]"#),
        "Should have Hello tag"
    );
    assert!(
        json.contains(r#""tags":["Search"]"#),
        "Should have Search tag"
    );
}

/// Test that operations have summaries from doc comments.
#[test]
fn test_schema_has_summaries() {
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path).expect("openapi.json should exist");

    // Check that summaries are present
    assert!(
        json.contains("Greet a user by name"),
        "Should have hello summary"
    );
    assert!(
        json.contains("Echo back a message with its length"),
        "Should have echo summary"
    );
    assert!(
        json.contains("Search with pagination"),
        "Should have search summary"
    );
}

/// Test that error responses use ProblemDetails.
#[test]
fn test_schema_has_error_responses() {
    let schema_path = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");
    let json = std::fs::read_to_string(schema_path).expect("openapi.json should exist");

    // Check 4XX and 5XX responses reference ProblemDetails
    assert!(json.contains(r#""4XX""#), "Should have 4XX error response");
    assert!(json.contains(r#""5XX""#), "Should have 5XX error response");
    assert!(
        json.contains("application/problem+json"),
        "Should use RFC 7807 content type"
    );
}
