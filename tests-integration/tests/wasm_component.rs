//! # Integration Tests for WASI Components
//!
//! ## Prerequisites
//!
//! These tests require built WASM components. Build them first:
//!
//! ```bash
//! cd examples/hello-world && cargo component build --release
//! cd ../crud-api && cargo component build --release
//! cd ../../mik-bridge && cargo component build --release
//! ```
//!
//! Tests will FAIL (not skip) if components are not found.
//!
//! ## Running
//!
//! ```bash
//! cd tests-integration
//! cargo test
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

/// Find a compiled component in the target directory.
fn find_component(name: &str) -> Result<PathBuf> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    // Try different possible locations
    let paths = [
        workspace_root.join(format!(
            "target/wasm32-wasip2/release/{}.wasm",
            name
        )),
        workspace_root.join(format!(
            "examples/{}/target/wasm32-wasip2/release/{}.wasm",
            name, name
        )),
        workspace_root.join(format!(
            "{}/target/wasm32-wasip2/release/{}.wasm",
            name, name
        )),
    ];

    for path in &paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    anyhow::bail!(
        "Component '{}' not found. Tried:\n  {}",
        name,
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    )
}

/// Create a test engine with component model enabled.
fn create_engine() -> Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    Engine::new(&config).context("Failed to create wasmtime engine")
}

/// Test that the router component can be loaded.
#[test]
fn test_router_loads() -> Result<()> {
    let engine = create_engine()?;

    let router_path = find_component("router")
        .expect("Router component not found. Run: cd examples/hello-world && cargo component build --release");

    let component = Component::from_file(&engine, router_path)?;

    // Better assertions
    let exports: Vec<_> = component.component_type().exports(&engine).collect();
    assert!(!exports.is_empty(), "Component should have exports");

    Ok(())
}

/// Test that the bridge component can be loaded.
#[test]
fn test_bridge_loads() -> Result<()> {
    let engine = create_engine()?;

    let bridge_path = find_component("mik-sdk-bridge")
        .expect("Bridge component not found. Run: cd mik-bridge && cargo component build --release");

    let component = Component::from_file(&engine, bridge_path)?;

    // Better assertions
    let exports: Vec<_> = component.component_type().exports(&engine).collect();
    assert!(!exports.is_empty(), "Component should have exports");

    Ok(())
}

// =============================================================================
// Environment Variable Configuration Tests
// =============================================================================

// NOTE: These are unit tests for env parsing logic.
// Consider moving to mik-bridge/src/lib.rs as #[cfg(test)] tests.

/// Test environment variable configuration parsing.
#[test]
fn test_env_var_parsing() {
    let env_vars = vec![
        ("MIKROZEN_MAX_BODY_SIZE".to_string(), "52428800".to_string()),
        ("OTHER_VAR".to_string(), "value".to_string()),
    ];

    let max_size: usize = env_vars
        .iter()
        .find(|(k, _)| k == "MIKROZEN_MAX_BODY_SIZE")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(10 * 1024 * 1024);

    assert_eq!(max_size, 52428800); // 50MB
}

/// Test default body size when env var is not set.
#[test]
fn test_default_body_size() {
    let env_vars: Vec<(String, String)> = vec![];

    let max_size: usize = env_vars
        .iter()
        .find(|(k, _)| k == "MIKROZEN_MAX_BODY_SIZE")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(10 * 1024 * 1024);

    assert_eq!(max_size, 10 * 1024 * 1024); // 10MB default
}

/// Test invalid env var value falls back to default.
#[test]
fn test_invalid_body_size_env_var() {
    let env_vars = vec![(
        "MIKROZEN_MAX_BODY_SIZE".to_string(),
        "not_a_number".to_string(),
    )];

    let max_size: usize = env_vars
        .iter()
        .find(|(k, _)| k == "MIKROZEN_MAX_BODY_SIZE")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(10 * 1024 * 1024);

    assert_eq!(max_size, 10 * 1024 * 1024); // Falls back to default
}

/// Test zero body size is valid (effectively disables body).
#[test]
fn test_zero_body_size() {
    let env_vars = vec![("MIKROZEN_MAX_BODY_SIZE".to_string(), "0".to_string())];

    let max_size: usize = env_vars
        .iter()
        .find(|(k, _)| k == "MIKROZEN_MAX_BODY_SIZE")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(10 * 1024 * 1024);

    assert_eq!(max_size, 0);
}

/// Test very large body size.
#[test]
fn test_large_body_size() {
    let env_vars = vec![(
        "MIKROZEN_MAX_BODY_SIZE".to_string(),
        "1073741824".to_string(), // 1GB
    )];

    let max_size: usize = env_vars
        .iter()
        .find(|(k, _)| k == "MIKROZEN_MAX_BODY_SIZE")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(10 * 1024 * 1024);

    assert_eq!(max_size, 1073741824);
}

// =============================================================================
// Body Size Limit Logic Tests
// =============================================================================

/// Simulates the body reading logic from bridge component.
fn simulate_body_read(chunks: &[&[u8]], max_size: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();

    for chunk in chunks {
        if chunk.is_empty() {
            break;
        }
        // Check size limit before extending
        if bytes.len() + chunk.len() > max_size {
            return None; // Body too large
        }
        bytes.extend(*chunk);
    }

    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

#[test]
fn test_body_within_limit() {
    let chunks: &[&[u8]] = &[b"hello", b" ", b"world"];
    let result = simulate_body_read(chunks, 1024);
    assert_eq!(result, Some(b"hello world".to_vec()));
}

#[test]
fn test_body_exactly_at_limit() {
    let chunks: &[&[u8]] = &[b"12345"];
    let result = simulate_body_read(chunks, 5);
    assert_eq!(result, Some(b"12345".to_vec()));
}

#[test]
fn test_body_exceeds_limit() {
    let chunks: &[&[u8]] = &[b"12345", b"67890"];
    let result = simulate_body_read(chunks, 5);
    assert_eq!(result, None); // Rejected
}

#[test]
fn test_body_exceeds_on_first_chunk() {
    let chunks: &[&[u8]] = &[b"this is too long"];
    let result = simulate_body_read(chunks, 5);
    assert_eq!(result, None);
}

#[test]
fn test_empty_body() {
    let chunks: &[&[u8]] = &[b""];
    let result = simulate_body_read(chunks, 1024);
    assert_eq!(result, None); // Empty returns None
}

#[test]
fn test_zero_limit_rejects_all() {
    let chunks: &[&[u8]] = &[b"a"];
    let result = simulate_body_read(chunks, 0);
    assert_eq!(result, None);
}

// =============================================================================
// BodyResult Pattern Tests (matching bridge/src/lib.rs)
// =============================================================================

/// Simulates the BodyResult enum from bridge component.
#[derive(Debug, PartialEq)]
enum BodyResult {
    Ok(Option<Vec<u8>>),
    TooLarge,
}

/// Simulates body reading with BodyResult pattern (matching bridge impl).
fn simulate_body_read_v2(chunks: &[&[u8]], max_size: usize) -> BodyResult {
    let mut bytes = Vec::new();

    for chunk in chunks {
        if chunk.is_empty() {
            break;
        }
        if bytes.len() + chunk.len() > max_size {
            return BodyResult::TooLarge;
        }
        bytes.extend(*chunk);
    }

    if bytes.is_empty() {
        BodyResult::Ok(None)
    } else {
        BodyResult::Ok(Some(bytes))
    }
}

#[test]
fn test_body_result_ok() {
    let chunks: &[&[u8]] = &[b"hello"];
    let result = simulate_body_read_v2(chunks, 1024);
    assert_eq!(result, BodyResult::Ok(Some(b"hello".to_vec())));
}

#[test]
fn test_body_result_too_large() {
    let chunks: &[&[u8]] = &[b"this is way too long for the limit"];
    let result = simulate_body_read_v2(chunks, 5);
    assert_eq!(result, BodyResult::TooLarge);
}

#[test]
fn test_body_result_empty() {
    let chunks: &[&[u8]] = &[b""];
    let result = simulate_body_read_v2(chunks, 1024);
    assert_eq!(result, BodyResult::Ok(None));
}

// =============================================================================
// JSON Size Limit Tests (matching router MAX_JSON_SIZE = 1_000_000)
// =============================================================================

const MAX_JSON_SIZE: usize = 1_000_000;

/// Simulates JSON parsing with size limit check (matching router impl).
fn simulate_json_parse(data: &[u8]) -> Option<&str> {
    if data.len() > MAX_JSON_SIZE {
        return None;
    }
    std::str::from_utf8(data).ok()
}

#[test]
fn test_json_within_limit() {
    let json = b"{\"key\": \"value\"}";
    assert!(simulate_json_parse(json).is_some());
}

#[test]
fn test_json_at_limit() {
    let json = vec![b'a'; MAX_JSON_SIZE];
    assert!(simulate_json_parse(&json).is_some());
}

#[test]
fn test_json_exceeds_limit() {
    let json = vec![b'a'; MAX_JSON_SIZE + 1];
    assert!(simulate_json_parse(&json).is_none());
}

#[test]
fn test_json_empty() {
    let json = b"";
    assert_eq!(simulate_json_parse(json), Some(""));
}
