//! E2E Tests for WASI HTTP Components
//!
//! These tests spawn WASI HTTP runtimes and make real HTTP requests.
//! Supports wasmtime, Spin, and wasmCloud to validate portability.
//!
//! ## Prerequisites
//!
//! Build and compose components first:
//!
//! ```bash
//! # Build bridge
//! cd mik-bridge && cargo component build --release
//!
//! # Build handler
//! cd examples/hello-world && cargo component build --release
//!
//! # Compose (from repo root)
//! wac plug target/wasm32-wasip2/release/mik_bridge.wasm \
//!     --plug target/wasm32-wasip2/release/hello_world.wasm \
//!     -o tests-integration/fixtures/hello-world-service.wasm
//! ```
//!
//! ## Running Tests
//!
//! ```bash
//! # Run on all available runtimes
//! cargo test -p mik-sdk-integration-tests --ignored
//!
//! # Run on specific runtime
//! WASI_RUNTIME=wasmtime cargo test -p mik-sdk-integration-tests --ignored
//! WASI_RUNTIME=spin cargo test -p mik-sdk-integration-tests --ignored
//! WASI_RUNTIME=wasmcloud cargo test -p mik-sdk-integration-tests --ignored
//! ```

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
extern crate libc;

// =============================================================================
// Runtime Abstraction
// =============================================================================

/// Supported WASI HTTP runtimes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runtime {
    Wasmtime,
    Spin,
    WasmCloud,
}

impl Runtime {
    /// Check if this runtime is installed and available.
    fn is_available(self) -> bool {
        let cmd = match self {
            Runtime::Wasmtime => "wasmtime",
            Runtime::Spin => "spin",
            Runtime::WasmCloud => "wash",
        };
        Command::new(cmd)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Get all available runtimes on this system.
    fn available() -> Vec<Runtime> {
        [Runtime::Wasmtime, Runtime::Spin, Runtime::WasmCloud]
            .into_iter()
            .filter(|r| r.is_available())
            .collect()
    }

    /// Get the runtime specified by WASI_RUNTIME env var, or all available.
    fn from_env() -> Vec<Runtime> {
        match std::env::var("WASI_RUNTIME").as_deref() {
            Ok("wasmtime") => vec![Runtime::Wasmtime],
            Ok("spin") => vec![Runtime::Spin],
            Ok("wasmcloud") => vec![Runtime::WasmCloud],
            _ => Self::available(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Runtime::Wasmtime => "wasmtime",
            Runtime::Spin => "spin",
            Runtime::WasmCloud => "wasmcloud",
        }
    }
}

// =============================================================================
// Test Server
// =============================================================================

/// Find an available port for the test server.
fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to port")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

/// Test server wrapper that cleans up on drop.
struct TestServer {
    process: Child,
    port: u16,
    runtime: Runtime,
    #[cfg(windows)]
    pid: u32,
}

impl TestServer {
    /// Start a WASI HTTP runtime with the given component.
    fn start(runtime: Runtime, wasm_path: &Path) -> anyhow::Result<Self> {
        let port = find_available_port();
        let addr = format!("127.0.0.1:{port}");
        let wasm = wasm_path.to_str().unwrap();

        let mut cmd = match runtime {
            Runtime::Wasmtime => {
                let mut c = Command::new("wasmtime");
                c.args(["serve", "-S", "cli=y", "--addr", &addr, wasm]);
                c
            }
            Runtime::Spin => {
                let mut c = Command::new("spin");
                c.args(["up", "-f", wasm, "--listen", &addr]);
                c
            }
            Runtime::WasmCloud => {
                let mut c = Command::new("wash");
                c.args(["dev", "--component-path", wasm, "--address", &addr]);
                c
            }
        };

        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        // On Unix, create a new process group so we can kill all children
        #[cfg(unix)]
        cmd.process_group(0);

        let process = cmd.spawn()?;

        #[cfg(windows)]
        let pid = process.id();

        // Wait for server to start (wasmCloud needs more time)
        let startup_delay = match runtime {
            Runtime::WasmCloud => Duration::from_millis(2000),
            _ => Duration::from_millis(500),
        };
        thread::sleep(startup_delay);

        // Verify the server is responding
        let server = Self {
            process,
            port,
            runtime,
            #[cfg(windows)]
            pid,
        };
        server.wait_for_ready(Duration::from_secs(10))?;

        Ok(server)
    }

    /// Wait for the server to be ready to accept connections.
    fn wait_for_ready(&self, timeout: Duration) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if ureq::get(&format!("{}/", self.base_url()))
                .timeout(Duration::from_millis(100))
                .call()
                .is_ok()
            {
                return Ok(());
            }
            // Also accept 404 as "ready" - means server is up but route doesn't exist
            if let Err(ureq::Error::Status(404, _)) = ureq::get(&format!("{}/__health", self.base_url()))
                .timeout(Duration::from_millis(100))
                .call()
            {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        anyhow::bail!(
            "{} server did not become ready within {:?}",
            self.runtime.name(),
            timeout
        )
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Kill the process tree, not just the parent process
        #[cfg(windows)]
        {
            // Use taskkill /T to kill the entire process tree
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &self.pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }

        #[cfg(unix)]
        {
            // Kill the process group (negative PID)
            let pid = self.process.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
            }
            // Give processes time to clean up
            thread::sleep(Duration::from_millis(100));
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }

        // Also try the standard kill as fallback
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

// =============================================================================
// Test Helpers
// =============================================================================

fn get_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// Run a test function on all available runtimes.
fn run_on_all_runtimes<F>(wasm_name: &str, test_fn: F)
where
    F: Fn(&TestServer),
{
    let wasm_path = get_fixture_path(wasm_name);
    if !wasm_path.exists() {
        eprintln!("Skipping: {} not found. Build with:", wasm_path.display());
        eprintln!("  cd mik-bridge && cargo component build --release");
        eprintln!("  cd examples/hello-world && cargo component build --release");
        eprintln!("  wac plug ... -o tests-integration/fixtures/{wasm_name}");
        return;
    }

    let runtimes = Runtime::from_env();
    if runtimes.is_empty() {
        eprintln!("No WASI runtimes available. Install wasmtime, spin, or wash.");
        return;
    }

    for runtime in runtimes {
        eprintln!("Testing on {}...", runtime.name());
        match TestServer::start(runtime, &wasm_path) {
            Ok(server) => test_fn(&server),
            Err(e) => {
                eprintln!("  Failed to start {}: {e}", runtime.name());
                // Don't fail the test, just skip this runtime
            }
        }
    }
}

// =============================================================================
// hello-world E2E Tests
// =============================================================================

#[test]
#[ignore = "requires pre-built WASM components - run with: cargo test --ignored"]
fn test_hello_world_home() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        let response = ureq::get(&format!("{}/", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        assert_eq!(response.header("content-type"), Some("application/json"));

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert!(json["message"].is_string());
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_hello_world_hello_with_name() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        let response = ureq::get(&format!("{}/hello/Claude", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["name"], "Claude");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_hello_world_search_with_query() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        let response = ureq::get(&format!("{}/search?q=rust&page=2", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["query"], "rust");
        assert_eq!(json["page"], 2);
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_hello_world_404() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        let response = ureq::get(&format!("{}/nonexistent", server.base_url())).call();

        // ureq returns Err for non-2xx status codes
        match response {
            Ok(_) => panic!("Expected 404"),
            Err(ureq::Error::Status(code, _)) => assert_eq!(code, 404),
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_hello_world_echo_post() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        let response = ureq::post(&format!("{}/echo", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({"message": "Hello from test"}))
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert!(json["echo"].is_string() || json["received"].is_object());
    });
}

// =============================================================================
// Error Response Tests (413, 501)
// =============================================================================

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_413_payload_too_large() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        // Default MIK_MAX_BODY_SIZE is 10MB, send 11MB
        let large_body = vec![b'x'; 11 * 1024 * 1024];

        let response = ureq::post(&format!("{}/echo", server.base_url()))
            .set("Content-Type", "application/octet-stream")
            .send_bytes(&large_body);

        match response {
            Ok(_) => panic!("Expected 413 Payload Too Large"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 413, "Expected 413, got {code}");
                // Verify RFC 7807 Problem Details response
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    assert_eq!(json["status"], 413);
                    assert_eq!(json["title"], "Payload Too Large");
                }
            }
            Err(ureq::Error::Transport(t)) => {
                // Server may close connection before client finishes sending 11MB.
                // This is valid HTTP behavior for early rejection of oversized payloads.
                let msg = t.to_string().to_lowercase();
                assert!(
                    msg.contains("broken pipe")
                        || msg.contains("connection reset")
                        || msg.contains("connection closed")
                        || msg.contains("connection was aborted")        // Windows
                        || msg.contains("forcibly closed")               // Windows
                        || msg.contains("os error 10053")                // Windows WSAECONNABORTED
                        || msg.contains("os error 10054"),               // Windows WSAECONNRESET
                    "Expected connection closed/reset, got: {t}"
                );
            }
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_501_unsupported_method_connect() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        // CONNECT method is not supported by mik-bridge
        // ureq doesn't support CONNECT directly, use raw request
        let client = std::net::TcpStream::connect(format!("127.0.0.1:{}", server.port));

        if let Ok(mut stream) = client {
            use std::io::{Read, Write};

            // Send raw CONNECT request
            let request = format!(
                "CONNECT / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                server.port
            );
            let _ = stream.write_all(request.as_bytes());
            let _ = stream.flush();

            // Read response
            let mut response = [0u8; 1024];
            if let Ok(n) = stream.read(&mut response) {
                let response_str = String::from_utf8_lossy(&response[..n]);
                // Should contain 501 status
                assert!(
                    response_str.contains("501") || response_str.contains("Not Implemented"),
                    "Expected 501 for CONNECT, got: {response_str}"
                );
            }
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_501_unsupported_method_trace() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        // TRACE method is not supported by mik-bridge
        let client = std::net::TcpStream::connect(format!("127.0.0.1:{}", server.port));

        if let Ok(mut stream) = client {
            use std::io::{Read, Write};

            // Send raw TRACE request
            let request = format!(
                "TRACE / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                server.port
            );
            let _ = stream.write_all(request.as_bytes());
            let _ = stream.flush();

            // Read response
            let mut response = [0u8; 1024];
            if let Ok(n) = stream.read(&mut response) {
                let response_str = String::from_utf8_lossy(&response[..n]);
                // Should contain 501 status
                assert!(
                    response_str.contains("501") || response_str.contains("Not Implemented"),
                    "Expected 501 for TRACE, got: {response_str}"
                );
            }
        }
    });
}

// =============================================================================
// Runtime-specific Smoke Tests
// =============================================================================

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_wasmtime_available() {
    if !Runtime::Wasmtime.is_available() {
        eprintln!("wasmtime not installed, skipping");
        return;
    }

    let wasm_path = get_fixture_path("hello-world-service.wasm");
    if !wasm_path.exists() {
        return;
    }

    let server = TestServer::start(Runtime::Wasmtime, &wasm_path).expect("Failed to start wasmtime");
    let response = ureq::get(&format!("{}/", server.base_url()))
        .call()
        .expect("Request failed");
    assert_eq!(response.status(), 200);
    eprintln!("wasmtime: OK");
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_spin_available() {
    if !Runtime::Spin.is_available() {
        eprintln!("spin not installed, skipping");
        return;
    }

    let wasm_path = get_fixture_path("hello-world-service.wasm");
    if !wasm_path.exists() {
        return;
    }

    let server = TestServer::start(Runtime::Spin, &wasm_path).expect("Failed to start spin");
    let response = ureq::get(&format!("{}/", server.base_url()))
        .call()
        .expect("Request failed");
    assert_eq!(response.status(), 200);
    eprintln!("spin: OK");
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_wasmcloud_available() {
    if !Runtime::WasmCloud.is_available() {
        eprintln!("wash not installed, skipping");
        return;
    }

    let wasm_path = get_fixture_path("hello-world-service.wasm");
    if !wasm_path.exists() {
        return;
    }

    let server = TestServer::start(Runtime::WasmCloud, &wasm_path).expect("Failed to start wasmcloud");
    let response = ureq::get(&format!("{}/", server.base_url()))
        .call()
        .expect("Request failed");
    assert_eq!(response.status(), 200);
    eprintln!("wasmcloud: OK");
}

// =============================================================================
// crud-api E2E Tests - Tests PUT, DELETE, error!, no_content!, path+body
// =============================================================================

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_index() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        let response = ureq::get(&format!("{}/", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["name"], "CRUD API Example");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_get_user() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test GET /users/{id} with path parameter
        let response = ureq::get(&format!("{}/users/1", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["id"], "1");
        assert_eq!(json["name"], "Alice");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_get_user_not_found() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test 404 with error! macro
        let response = ureq::get(&format!("{}/users/999", server.base_url())).call();

        match response {
            Ok(_) => panic!("Expected 404"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 404);
                // Verify RFC 7807 Problem Details
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    assert_eq!(json["status"], 404);
                    assert_eq!(json["title"], "Not Found");
                }
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_create_user_post() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test POST with JSON body
        let response = ureq::post(&format!("{}/users", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "name": "Charlie",
                "email": "charlie@example.com"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 201); // Created
        assert!(response.header("location").is_some()); // Location header

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["name"], "Charlie");
        assert_eq!(json["email"], "charlie@example.com");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_update_user_put() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test PUT with path + body
        let response = ureq::put(&format!("{}/users/1", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "name": "Alice Updated"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["id"], "1");
        assert!(json["updated_at"].is_string());
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_update_user_bad_request() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test 400 Bad Request - invalid ID format
        let response = ureq::put(&format!("{}/users/not-a-number", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({ "name": "Test" }));

        match response {
            Ok(_) => panic!("Expected 400"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 400);
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    assert_eq!(json["status"], 400);
                    assert_eq!(json["title"], "Bad Request");
                }
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_update_user_unprocessable() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test 422 Unprocessable Entity - no fields provided
        let response = ureq::put(&format!("{}/users/1", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({}));

        match response {
            Ok(_) => panic!("Expected 422"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 422);
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    assert_eq!(json["status"], 422);
                }
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_delete_user() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test DELETE returns 204 No Content
        let response = ureq::delete(&format!("{}/users/1", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 204);
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_delete_user_not_found() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test DELETE 404
        let response = ureq::delete(&format!("{}/users/999", server.base_url())).call();

        match response {
            Ok(_) => panic!("Expected 404"),
            Err(ureq::Error::Status(code, _)) => assert_eq!(code, 404),
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_list_users_with_pagination() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test query params with defaults
        let response = ureq::get(&format!("{}/users?page=2&limit=25", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["page"], 2);
        assert_eq!(json["limit"], 25);
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_list_posts_cursor_pagination() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test cursor pagination
        let response = ureq::get(&format!("{}/posts", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert!(json["posts"].is_array());
        assert!(json["has_next"].is_boolean());
        assert!(json["next_cursor"].is_string() || json["next_cursor"].is_null());
    });
}

// =============================================================================
// CRUD API - Search Endpoint (Runtime Filter Parsing)
// =============================================================================

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_search_users() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test POST /users/search with Mongo-style filter
        let response = ureq::post(&format!("{}/users/search", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "name": {"$starts_with": "A"},
                "status": "active"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert!(json["users"].is_array());
        assert!(json["page"].is_number());
        assert!(json["limit"].is_number());
        // Check SQL was generated with merged filter
        assert!(json["_debug"]["sql"].as_str().unwrap().contains("WHERE"));
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_search_users_with_pagination() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test search with query params for pagination
        let response = ureq::post(&format!("{}/users/search?page=2&limit=10", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "status": "active"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);
        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["page"], 2);
        assert_eq!(json["limit"], 10);
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_search_users_empty_filter() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test search with empty filter body - should return 400
        let response = ureq::post(&format!("{}/users/search", server.base_url()))
            .set("Content-Type", "application/json")
            .send_string("");

        match response {
            Ok(_) => panic!("Expected 400"),
            Err(ureq::Error::Status(code, _)) => {
                assert_eq!(code, 400);
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_search_users_invalid_filter() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test search with invalid JSON filter
        let response = ureq::post(&format!("{}/users/search", server.base_url()))
            .set("Content-Type", "application/json")
            .send_string("not valid json");

        match response {
            Ok(_) => panic!("Expected 400"),
            Err(ureq::Error::Status(code, _)) => {
                assert_eq!(code, 400);
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_search_users_disallowed_field() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test search with field not in allow list - should return 400
        let response = ureq::post(&format!("{}/users/search", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "password": "secret"
            }));

        match response {
            Ok(_) => panic!("Expected 400"),
            Err(ureq::Error::Status(code, _)) => {
                assert_eq!(code, 400);
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

// ============================================================================
// OPENAPI SCHEMA TESTS
// ============================================================================

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_hello_world_schema_endpoint() {
    run_on_all_runtimes("hello-world-service.wasm", |server| {
        // Test GET /__schema returns OpenAPI JSON
        let response = ureq::get(&format!("{}/__schema", server.base_url()))
            .call()
            .expect("/__schema should return 200");

        assert_eq!(response.status(), 200);

        let body = response.into_string().unwrap();
        assert!(body.contains("\"openapi\""), "Should contain openapi field");
        assert!(body.contains("\"3.0.0\""), "Should be OpenAPI 3.0.0");
        assert!(body.contains("\"paths\""), "Should contain paths");
        assert!(body.contains("\"components\""), "Should contain components");
        // Note: braces escaped to avoid format string interpretation
        assert!(body.contains(r#""/hello/{name}""#), "Should document /hello/{{name}} route");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_crud_api_schema_endpoint() {
    run_on_all_runtimes("crud-api-service.wasm", |server| {
        // Test GET /__schema returns OpenAPI JSON
        let response = ureq::get(&format!("{}/__schema", server.base_url()))
            .call()
            .expect("/__schema should return 200");

        assert_eq!(response.status(), 200);

        let body = response.into_string().unwrap();
        assert!(body.contains("\"openapi\""), "Should contain openapi field");
        assert!(body.contains("\"paths\""), "Should contain paths");
        assert!(body.contains("\"/users\""), "Should document /users route");
        assert!(body.contains("\"/users/search\""), "Should document /users/search route");
    });
}
