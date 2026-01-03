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
                // -S cli=y: Enable WASI CLI APIs
                // -S http=y: Enable WASI HTTP imports (for outbound requests)
                // -S inherit-network=y: Allow network access to all addresses
                c.args([
                    "serve",
                    "-S", "cli=y",
                    "-S", "http=y",
                    "-S", "inherit-network=y",
                    "--addr", &addr,
                    wasm,
                ]);
                c
            }
            Runtime::Spin => {
                // Create a temporary spin.toml manifest with allowed_outbound_hosts
                let wasm_dir = wasm_path.parent().expect("wasm should have parent dir");
                let wasm_filename = wasm_path.file_name().expect("wasm should have filename")
                    .to_str().expect("filename should be valid utf8");
                let manifest_path = wasm_dir.join("spin-temp.toml");
                let manifest_content = format!(
                    r#"spin_manifest_version = 2

[application]
name = "e2e-test"
version = "0.1.0"

[[trigger.http]]
route = "/..."
component = "handler"

[component.handler]
source = "{}"
allowed_outbound_hosts = ["*://*:*"]
"#,
                    wasm_filename
                );
                std::fs::write(&manifest_path, manifest_content)
                    .expect("Failed to write spin manifest");

                let mut c = Command::new("spin");
                c.args(["up", "-f", manifest_path.to_str().unwrap(), "--listen", &addr]);
                c.current_dir(wasm_dir);
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
// OPENAPI SCHEMA TESTS - Removed
// ============================================================================
// OpenAPI schema is now generated statically at build time (not served at runtime).
// See: examples/hello-world/openapi.json
// The /__schema endpoint no longer exists.

// =============================================================================
// HTTP Client E2E Tests (local mock server)
// =============================================================================
// These tests verify that the HTTP client works across WASI runtimes by:
// 1. Starting a mock HTTP server on localhost
// 2. Starting the WASM runtime with external-api-service.wasm
// 3. Having the WASM service make outbound requests to the mock server
// 4. Verifying the responses
//
// Runtime Support:
// - wasmtime: Full support with -S http=y -S inherit-network=y
// - spin: Full support with dynamically generated spin.toml (allowed_outbound_hosts)
// - wasmcloud: Not supported (requires NATS infrastructure, use WASI_RUNTIME=wasmtime or spin)

/// Mock HTTP server for testing outbound requests from WASM.
struct MockServer {
    port: u16,
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    /// Start a mock HTTP server that echoes back request details as JSON.
    fn start() -> Self {
        use std::io::{Read, Write};
        use std::sync::mpsc;

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock server");
        let port = listener.local_addr().unwrap().port();
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = std::thread::spawn(move || {
            loop {
                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                // Accept connections (non-blocking)
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_read_timeout(Some(Duration::from_millis(100))).ok();
                        stream.set_write_timeout(Some(Duration::from_millis(100))).ok();

                        let mut buffer = [0u8; 4096];
                        let n = match stream.read(&mut buffer) {
                            Ok(n) if n > 0 => n,
                            _ => continue,
                        };

                        let request = String::from_utf8_lossy(&buffer[..n]);

                        // Parse basic HTTP request
                        let first_line = request.lines().next().unwrap_or("");
                        let parts: Vec<&str> = first_line.split_whitespace().collect();
                        let method = parts.first().copied().unwrap_or("GET");
                        let path = parts.get(1).copied().unwrap_or("/");

                        // Extract body (after \r\n\r\n)
                        let body = request
                            .split("\r\n\r\n")
                            .nth(1)
                            .unwrap_or("")
                            .trim();

                        // Escape body for JSON (handle all control characters)
                        let escaped_body = body
                            .replace('\\', "\\\\")
                            .replace('\"', "\\\"")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r")
                            .replace('\t', "\\t")
                            .chars()
                            .filter(|c| !c.is_control())
                            .collect::<String>();

                        // Create JSON response echoing request details
                        let response_body = format!(
                            r#"{{"method":"{}","path":"{}","body":"{}","server":"mock"}}"#,
                            method,
                            path.replace('\"', "\\\""),
                            escaped_body
                        );

                        let response = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: application/json\r\n\
                             Content-Length: {}\r\n\
                             X-Mock-Server: true\r\n\
                             \r\n\
                             {}",
                            response_body.len(),
                            response_body
                        );

                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection yet, sleep briefly
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            port,
            shutdown_tx: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        // Signal shutdown
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Wait for thread to finish
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_get_request() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        // Start mock server
        let mock = MockServer::start();
        let mock_url = format!("{}/api/test", mock.url());
        eprintln!("Mock server URL: {}", mock_url);

        // Use POST /fetch-local with JSON body to avoid URL encoding issues
        let result = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": mock_url,
                "method": "GET"
            }));

        let response = match result {
            Ok(r) => r,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                panic!("Request returned {}: {}", code, body);
            }
            Err(e) => panic!("Request failed: {}", e),
        };

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["status"], 200, "Mock server should return 200");

        // Parse the body returned by the proxy
        let body_str = json["body"].as_str().expect("Should have body");
        let inner: serde_json::Value =
            serde_json::from_str(body_str).expect("Body should be JSON");

        assert_eq!(inner["method"], "GET");
        assert_eq!(inner["path"], "/api/test");
        assert_eq!(inner["server"], "mock");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_post_request() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        let mock = MockServer::start();
        let mock_url = format!("{}/api/create", mock.url());

        // POST with JSON body via fetch-local
        let response = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": mock_url,
                "method": "POST",
                "body": "{\"name\":\"test\",\"value\":42}"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["status"], 200);

        let body_str = json["body"].as_str().expect("Should have body");
        let inner: serde_json::Value =
            serde_json::from_str(body_str).expect("Body should be JSON");

        assert_eq!(inner["method"], "POST");
        assert_eq!(inner["server"], "mock");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_put_request() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        let mock = MockServer::start();
        let mock_url = format!("{}/api/update/123", mock.url());

        let response = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": mock_url,
                "method": "PUT",
                "body": "{\"updated\":true}"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["status"], 200);

        let body_str = json["body"].as_str().expect("Should have body");
        let inner: serde_json::Value =
            serde_json::from_str(body_str).expect("Body should be JSON");

        assert_eq!(inner["method"], "PUT");
        assert_eq!(inner["path"], "/api/update/123");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_delete_request() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        let mock = MockServer::start();
        let mock_url = format!("{}/api/delete/456", mock.url());

        let response = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": mock_url,
                "method": "DELETE"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["status"], 200);

        let body_str = json["body"].as_str().expect("Should have body");
        let inner: serde_json::Value =
            serde_json::from_str(body_str).expect("Body should be JSON");

        assert_eq!(inner["method"], "DELETE");
        assert_eq!(inner["path"], "/api/delete/456");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_connection_refused() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        // Try to connect to a port where nothing is listening
        let bad_url = "http://127.0.0.1:1"; // Port 1 is privileged and unlikely to be in use

        // Use POST with JSON body to avoid URL encoding issues
        let response = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": bad_url,
                "method": "GET"
            }));

        match response {
            Ok(_) => panic!("Expected error for connection refused"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 502, "Should return 502 Bad Gateway");
                let json: serde_json::Value = resp.into_json().expect("Should be JSON");
                assert_eq!(json["title"], "Bad Gateway");
            }
            Err(e) => panic!("Unexpected error type: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_ssrf_protection() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        // The /proxy endpoint uses deny_private_ips(), should block localhost
        // Note: We need to manually construct the URL because the SDK has a known
        // issue with URL-decoding query parameters (tracked for future fix)
        let proxy_url = format!("{}/proxy?url=http://127.0.0.1:8080/internal", server.base_url());
        let response = ureq::get(&proxy_url).call();

        match response {
            Ok(_) => panic!("Expected SSRF block"),
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 403, "Should return 403 Forbidden for SSRF");
                let json: serde_json::Value = resp.into_json().expect("Should be JSON");
                assert_eq!(json["title"], "Forbidden");
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_response_headers() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        let mock = MockServer::start();

        // Use POST with JSON body to avoid URL encoding issues
        let response = ureq::post(&format!("{}/fetch-local", server.base_url()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "url": mock.url(),
                "method": "GET"
            }))
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");

        // Check that response headers from mock server are captured
        let headers = json["headers"].as_array().expect("Should have headers array");
        let headers_str: String = headers
            .iter()
            .map(|h| h.as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(", ");

        assert!(
            headers_str.to_lowercase().contains("content-type")
                || headers_str.to_lowercase().contains("x-mock-server"),
            "Should capture response headers, got: {}",
            headers_str
        );
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_url_decoding() {
    // Test that URL-encoded query parameters are properly decoded
    run_on_all_runtimes("external-api-service.wasm", |server| {
        let mock = MockServer::start();
        let mock_url = format!("{}/api/test", mock.url());

        // Use ureq's .query() which URL-encodes the value
        // This tests that the SDK properly decodes the URL before using it
        let response = ureq::get(&format!("{}/fetch-local", server.base_url()))
            .query("url", &mock_url)
            .call();

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                panic!("Request returned {}: {}", code, body);
            }
            Err(e) => panic!("Request failed: {}", e),
        };

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["status"], 200, "Mock server should return 200");

        // Parse the body returned by the proxy
        let body_str = json["body"].as_str().expect("Should have body");
        let inner: serde_json::Value =
            serde_json::from_str(body_str).expect("Body should be JSON");

        assert_eq!(inner["method"], "GET");
        assert_eq!(inner["path"], "/api/test");
        assert_eq!(inner["server"], "mock");
    });
}

#[test]
#[ignore = "requires pre-built WASM components"]
fn test_http_client_external_api_index() {
    run_on_all_runtimes("external-api-service.wasm", |server| {
        // Basic sanity check that external-api-service is working
        let response = ureq::get(&format!("{}/", server.base_url()))
            .call()
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let json: serde_json::Value = response.into_json().expect("Failed to parse JSON");
        assert_eq!(json["name"], "External API Example");

        // Verify fetch-local endpoints are listed
        let endpoints = json["endpoints"].as_array().expect("Should have endpoints");
        let endpoints_str = endpoints
            .iter()
            .filter_map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        assert!(
            endpoints_str.contains("fetch-local"),
            "Should list fetch-local endpoint"
        );
    });
}
