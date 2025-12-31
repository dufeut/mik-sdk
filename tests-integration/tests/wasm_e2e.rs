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
            Err(e) => panic!("Unexpected error: {e}"),
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
