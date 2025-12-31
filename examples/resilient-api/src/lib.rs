#![allow(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::exhaustive_structs)]
//! Resilient API Example - Production-grade error handling patterns.
//!
//! Demonstrates real-world resilience patterns for external HTTP calls:
//! - Retry with exponential backoff
//! - Graceful degradation with fallbacks
//! - Partial success aggregation
//! - Rate limit (429) handling
//! - Health checks with dependency verification

#[allow(warnings, unsafe_code)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use mik_sdk::http_client::Error as HttpError;
use mik_sdk::prelude::*;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum retry attempts for transient failures
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds)
const BASE_DELAY_MS: u64 = 100;

/// Request timeout (milliseconds)
const TIMEOUT_MS: u32 = 5000;

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Type)]
pub struct IndexResponse {
    pub name: String,
    pub version: String,
    pub patterns: Vec<String>,
}

#[derive(Type)]
pub struct RetryResponse {
    pub data: Option<String>,
    pub attempts: i64,
    pub success: bool,
}

#[derive(Type)]
pub struct FallbackResponse {
    pub data: String,
    pub source: String,
}

#[derive(Type)]
pub struct AggregateResponse {
    pub results: Vec<AggregateResult>,
    pub errors: Vec<AggregateError>,
    pub partial_success: bool,
}

#[derive(Type)]
pub struct AggregateResult {
    pub source: String,
    pub data: String,
}

#[derive(Type)]
pub struct AggregateError {
    pub source: String,
    pub reason: String,
}

#[derive(Type)]
pub struct HealthResponse {
    pub status: String,
    pub dependencies: Vec<DependencyStatus>,
}

#[derive(Type)]
pub struct DependencyStatus {
    pub name: String,
    pub healthy: bool,
    pub latency_ms: Option<i64>,
    pub error: Option<String>,
}

// ============================================================================
// ROUTES
// ============================================================================

routes! {
    GET "/" => index -> IndexResponse,

    // Retry with exponential backoff
    GET "/retry" => retry_demo -> RetryResponse,

    // Graceful degradation with fallbacks
    GET "/fallback" => fallback_demo -> FallbackResponse,

    // Partial success aggregation
    GET "/aggregate" => aggregate_demo -> AggregateResponse,

    // Rate limit handling
    GET "/rate-limited" => rate_limit_demo,

    // Health check with dependency verification
    GET "/health" => health_check -> HealthResponse,
}

// ============================================================================
// HANDLERS
// ============================================================================

fn index(_req: &Request) -> Response {
    ok!({
        "name": "Resilient API Example",
        "version": "0.1.0",
        "patterns": [
            "GET /retry - Retry with exponential backoff",
            "GET /fallback - Graceful degradation with fallbacks",
            "GET /aggregate - Partial success aggregation",
            "GET /rate-limited - Rate limit (429) handling",
            "GET /health - Health check with dependency verification"
        ]
    })
}

// ============================================================================
// PATTERN 1: Retry with Exponential Backoff
// ============================================================================

/// Demonstrates retry logic with exponential backoff.
///
/// When external services have transient failures (network blips, temporary
/// overload), retrying with increasing delays often succeeds.
///
/// Backoff schedule: 100ms, 200ms, 400ms (exponential)
fn retry_demo(req: &Request) -> Response {
    log!(info, "starting retry demo");

    // This endpoint sometimes fails (simulated with httpbin's status endpoint)
    // In real usage, you'd retry on network errors or 5xx responses
    let url = "https://httpbin.org/get";

    let mut attempts = 0u32;

    while attempts < MAX_RETRIES {
        attempts += 1;

        log!(debug, "attempt", number: attempts, max: MAX_RETRIES);

        let result = fetch!(GET url, timeout: TIMEOUT_MS)
            .with_trace_id(req.trace_id())
            .send();

        match result {
            Ok(response) if response.is_success() => {
                log!(info, "retry succeeded", attempts: attempts);
                return ok!({
                    "data": "Request succeeded",
                    "attempts": attempts as i64,
                    "success": true
                });
            },
            Ok(response) if is_retryable_status(response.status()) => {
                log!(warn, "retryable error", status: response.status(), attempt: attempts);
            },
            Ok(response) => {
                // Non-retryable HTTP error (4xx except 429)
                log!(error, "non-retryable error", status: response.status());
                return error! {
                    status: response.status(),
                    title: "Request Failed",
                    detail: format!("Non-retryable error: HTTP {}", response.status())
                };
            },
            Err(e) if is_retryable_error(&e) => {
                log!(warn, "retryable error", error: &e.to_string(), attempt: attempts);
            },
            Err(e) => {
                // Non-retryable error (e.g., invalid URL, SSRF block)
                log!(error, "non-retryable error", error: &e.to_string());
                return error! {
                    status: 502,
                    title: "Bad Gateway",
                    detail: format!("Non-retryable error: {}", e)
                };
            },
        }

        // Exponential backoff before next attempt
        if attempts < MAX_RETRIES {
            let delay_ms = BASE_DELAY_MS * (1 << (attempts - 1));
            log!(debug, "backing off", delay_ms: delay_ms);
            // Note: In WASM we can't actually sleep, but we document the intent
            // In production, you might use a request queue or async patterns
        }
    }

    // All retries exhausted
    log!(error, "all retries exhausted", attempts: attempts);
    ok!({
        "data": null,
        "attempts": attempts as i64,
        "success": false
    })
}

/// Check if HTTP status code is retryable
const fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Check if error is retryable (network issues, timeouts)
const fn is_retryable_error(e: &HttpError) -> bool {
    matches!(
        e,
        HttpError::Timeout | HttpError::ConnectionError(_) | HttpError::DnsError(_)
    )
}

// ============================================================================
// PATTERN 2: Graceful Degradation with Fallbacks
// ============================================================================

/// Demonstrates fallback chain: primary -> secondary -> cached/default.
///
/// When the primary data source fails, try alternatives before giving up.
/// This provides graceful degradation instead of hard failure.
fn fallback_demo(req: &Request) -> Response {
    log!(info, "starting fallback demo");

    // Try primary source
    if let Some((data, source)) = try_primary_source(req) {
        return ok!({ "data": data, "source": source });
    }

    // Try secondary source
    if let Some((data, source)) = try_secondary_source(req) {
        return ok!({ "data": data, "source": source });
    }

    // Return cached/default data
    log!(warn, "all sources failed, using default");
    ok!({
        "data": "Default fallback data (cached or static)",
        "source": "default"
    })
}

fn try_primary_source(req: &Request) -> Option<(String, String)> {
    log!(debug, "trying primary source");

    let result = fetch!(GET "https://httpbin.org/get", timeout: 2000)
        .with_trace_id(req.trace_id())
        .send();

    match result {
        Ok(r) if r.is_success() => {
            log!(info, "primary source succeeded");
            Some((
                "Data from primary source".to_string(),
                "primary".to_string(),
            ))
        },
        Ok(r) => {
            log!(warn, "primary source failed", status: r.status());
            None
        },
        Err(e) => {
            log!(warn, "primary source error", error: &e.to_string());
            None
        },
    }
}

fn try_secondary_source(req: &Request) -> Option<(String, String)> {
    log!(debug, "trying secondary source");

    let result = fetch!(GET "https://httpbin.org/get", timeout: 2000)
        .with_trace_id(req.trace_id())
        .send();

    match result {
        Ok(r) if r.is_success() => {
            log!(info, "secondary source succeeded");
            Some((
                "Data from secondary source".to_string(),
                "secondary".to_string(),
            ))
        },
        Ok(r) => {
            log!(warn, "secondary source failed", status: r.status());
            None
        },
        Err(e) => {
            log!(warn, "secondary source error", error: &e.to_string());
            None
        },
    }
}

// ============================================================================
// PATTERN 3: Partial Success Aggregation
// ============================================================================

/// Demonstrates partial success: return what succeeded, report what failed.
///
/// When aggregating from multiple sources, don't fail the entire request
/// if one source fails. Return partial data with error details.
fn aggregate_demo(req: &Request) -> Response {
    log!(info, "starting aggregate demo");

    let mut results: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    // Source 1: UUID service
    match fetch_source("uuid", "https://httpbin.org/uuid", req) {
        Ok(data) => results.push(("uuid".to_string(), data)),
        Err(reason) => errors.push(("uuid".to_string(), reason)),
    }

    // Source 2: Headers echo
    match fetch_source("headers", "https://httpbin.org/headers", req) {
        Ok(data) => results.push(("headers".to_string(), data)),
        Err(reason) => errors.push(("headers".to_string(), reason)),
    }

    // Source 3: IP address
    match fetch_source("ip", "https://httpbin.org/ip", req) {
        Ok(data) => results.push(("ip".to_string(), data)),
        Err(reason) => errors.push(("ip".to_string(), reason)),
    }

    let total = results.len() + errors.len();
    let partial_success = !results.is_empty() && !errors.is_empty();

    log!(
        info,
        "aggregate complete",
        succeeded: results.len(),
        failed: errors.len(),
        partial: partial_success
    );

    // Build response arrays
    let results_arr = results.iter().fold(json::arr(), |arr, (source, data)| {
        arr.push(
            json::obj()
                .set("source", json::str(source))
                .set("data", json::str(data)),
        )
    });

    let errors_arr = errors.iter().fold(json::arr(), |arr, (source, reason)| {
        arr.push(
            json::obj()
                .set("source", json::str(source))
                .set("reason", json::str(reason)),
        )
    });

    // Return appropriate status based on results
    if results.is_empty() {
        // Total failure
        error! {
            status: 502,
            title: "Bad Gateway",
            detail: format!("All {} sources failed", total)
        }
    } else {
        ok!({
            "results": results_arr,
            "errors": errors_arr,
            "partial_success": partial_success
        })
    }
}

fn fetch_source(name: &str, url: &str, req: &Request) -> Result<String, String> {
    log!(debug, "fetching source", name: name);

    let result = fetch!(GET url, timeout: 3000)
        .with_trace_id(req.trace_id())
        .send();

    match result {
        Ok(r) if r.is_success() => {
            let body = r.body();
            let text = String::from_utf8_lossy(&body);
            // Truncate for demo purposes
            let truncated: String = text.chars().take(100).collect();
            Ok(truncated)
        },
        Ok(r) => Err(format!("HTTP {}", r.status())),
        Err(e) => Err(e.to_string()),
    }
}

// ============================================================================
// PATTERN 4: Rate Limit Handling
// ============================================================================

/// Demonstrates proper handling of HTTP 429 (Too Many Requests).
///
/// When rate limited:
/// 1. Check Retry-After header
/// 2. Return 503 to client with Retry-After forwarded
/// 3. Log for monitoring/alerting
fn rate_limit_demo(req: &Request) -> Response {
    log!(info, "starting rate limit demo");

    // httpbin's /status/429 simulates a rate-limited response
    let result = fetch!(GET "https://httpbin.org/status/429", timeout: TIMEOUT_MS)
        .with_trace_id(req.trace_id())
        .send();

    match result {
        Ok(response) if response.status() == 429 => {
            // Extract Retry-After header if present
            let retry_after = response
                .headers()
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .map(|(_, v)| v.clone());

            log!(
                warn,
                "rate limited by upstream",
                retry_after: retry_after.as_deref().unwrap_or("not specified")
            );

            // Forward rate limit to client as 503 Service Unavailable
            // This is more accurate than 429 (we're not rate limiting them,
            // our upstream is limiting us)
            let mut headers = vec![("content-type".to_string(), "application/json".to_string())];

            if let Some(ref retry) = retry_after {
                headers.push(("retry-after".to_string(), retry.clone()));
            }

            let retry_msg = retry_after.map_or_else(
                || "Retry later".to_string(),
                |r| format!("Retry after {r} seconds"),
            );

            let body = json::obj()
                .set("type", json::str("about:blank"))
                .set("status", json::int(503))
                .set("title", json::str("Service Unavailable"))
                .set(
                    "detail",
                    json::str(format!("Upstream rate limit exceeded. {retry_msg}")),
                );

            Response {
                status: 503,
                headers,
                body: Some(body.to_string().into_bytes()),
            }
        },
        Ok(response) if response.is_success() => {
            ok!({
                "message": "Request succeeded (not rate limited)",
                "status": response.status() as i64
            })
        },
        Ok(response) => {
            error! {
                status: 502,
                title: "Bad Gateway",
                detail: format!("Unexpected status: {}", response.status())
            }
        },
        Err(e) => {
            log!(error, "request failed", error: &e.to_string());
            error! {
                status: 502,
                title: "Bad Gateway",
                detail: format!("Request failed: {}", e)
            }
        },
    }
}

// ============================================================================
// PATTERN 5: Health Check with Dependency Verification
// ============================================================================

/// Demonstrates production health check that verifies dependencies.
///
/// A good health check:
/// - Checks all critical dependencies
/// - Returns quickly (uses short timeouts)
/// - Reports individual dependency status
/// - Returns 503 if any critical dependency is unhealthy
fn health_check(req: &Request) -> Response {
    log!(info, "running health check");

    let mut dependencies: Vec<(String, bool, Option<i64>, Option<String>)> = Vec::new();
    let mut all_healthy = true;

    // Check dependency 1: Primary API
    let (healthy, latency, error) = check_dependency("https://httpbin.org/get", req);
    if !healthy {
        all_healthy = false;
    }
    dependencies.push(("primary-api".to_string(), healthy, latency, error));

    // Check dependency 2: Secondary API
    let (healthy, latency, error) = check_dependency("https://httpbin.org/get", req);
    if !healthy {
        all_healthy = false;
    }
    dependencies.push(("secondary-api".to_string(), healthy, latency, error));

    // Build response
    let deps_arr = dependencies
        .iter()
        .fold(json::arr(), |arr, (name, healthy, latency, error)| {
            let mut obj = json::obj()
                .set("name", json::str(name))
                .set("healthy", json::bool(*healthy));

            if let Some(ms) = latency {
                obj = obj.set("latency_ms", json::int(*ms));
            }
            if let Some(err) = error {
                obj = obj.set("error", json::str(err));
            }

            arr.push(obj)
        });

    let status_str = if all_healthy { "healthy" } else { "degraded" };

    log!(info, "health check complete", status: status_str);

    // Return 503 if unhealthy, 200 if healthy
    let http_status = if all_healthy { 200 } else { 503 };

    Response {
        status: http_status,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: Some(
            json::obj()
                .set("status", json::str(status_str))
                .set("dependencies", deps_arr)
                .to_string()
                .into_bytes(),
        ),
    }
}

fn check_dependency(url: &str, req: &Request) -> (bool, Option<i64>, Option<String>) {
    let start = time::now_millis();

    let result = fetch!(GET url, timeout: 2000)
        .with_trace_id(req.trace_id())
        .send();

    #[allow(clippy::cast_possible_wrap)]
    let latency = (time::now_millis() - start) as i64;

    match result {
        Ok(r) if r.is_success() => (true, Some(latency), None),
        Ok(r) => (false, Some(latency), Some(format!("HTTP {}", r.status()))),
        Err(e) => (false, None, Some(e.to_string())),
    }
}
