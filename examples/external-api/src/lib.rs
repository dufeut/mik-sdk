//! External API Example - Fetching data from external HTTP APIs.
//!
//! Demonstrates real-world patterns for external HTTP calls:
//! - Simple GET requests with `.send()`
//! - POST requests with JSON body
//! - Request headers and timeouts
//! - Trace ID propagation for distributed tracing
//! - Error handling for HTTP failures
//! - SSRF protection with `deny_private_ips()`

#[allow(warnings, unsafe_code)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use mik_sdk::prelude::*;

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Type)]
pub struct IndexResponse {
    pub name: String,
    pub version: String,
    pub endpoints: Vec<String>,
}

#[derive(Query)]
pub struct ProxyQuery {
    pub url: String,
}

#[derive(Type)]
pub struct ProxyResponse {
    pub status: i64,
    pub body: Option<String>,
    pub headers: Vec<String>,
}

#[derive(Type)]
pub struct GithubUser {
    pub login: String,
    pub id: i64,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub public_repos: i64,
}

#[derive(Path)]
pub struct UsernamePath {
    pub username: String,
}

#[derive(Type)]
pub struct WebhookPayload {
    pub event: String,
    pub data: String,
}

#[derive(Type)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// ROUTES
// ============================================================================

routes! {
    GET "/" => index -> IndexResponse,

    // Fetch GitHub user info (demonstrates GET with path params)
    GET "/github/{username}" => github_user(path: UsernamePath) -> GithubUser,

    // Proxy any URL (demonstrates SSRF protection)
    GET "/proxy" => proxy(query: ProxyQuery) -> ProxyResponse,

    // Send webhook (demonstrates POST with JSON body)
    POST "/webhook" => send_webhook(body: WebhookPayload) -> WebhookResponse,

    // Aggregation example (demonstrates multiple sequential calls)
    GET "/aggregate" => aggregate,
}

// ============================================================================
// HANDLERS
// ============================================================================

fn index(_req: &Request) -> Response {
    ok!({
        "name": "External API Example",
        "version": "0.1.0",
        "endpoints": [
            "GET /github/{username} - Fetch GitHub user info",
            "GET /proxy?url=<url> - Proxy external URL (SSRF protected)",
            "POST /webhook - Send webhook with JSON body",
            "GET /aggregate - Aggregate multiple API calls"
        ]
    })
}

/// Fetch GitHub user information using the public API.
///
/// Demonstrates:
/// - Simple GET request to external API with `.send()`
/// - JSON response parsing with path_* methods
/// - Error handling for HTTP failures
fn github_user(path: UsernamePath, req: &Request) -> Response {
    let url = format!("https://api.github.com/users/{}", path.username);

    log!(info, "fetching github user", username: &path.username);

    // Make the request - just call .send()!
    let result = fetch!(GET &url,
        headers: {
            "User-Agent": "mik-sdk-example/0.1.0",
            "Accept": "application/vnd.github.v3+json"
        },
        timeout: 5000
    )
    .with_trace_id(req.trace_id())
    .send();

    // Handle the response
    let response = match result {
        Ok(r) => r,
        Err(e) => {
            log!(error, "github api failed", error: &e.to_string());
            return error! {
                status: 502,
                title: "Bad Gateway",
                detail: "Failed to fetch from GitHub API"
            };
        },
    };

    // Check HTTP status
    if !response.is_success() {
        if response.status() == 404 {
            return error! {
                status: 404,
                title: "Not Found",
                detail: "GitHub user not found"
            };
        }
        return error! {
            status: 502,
            title: "Bad Gateway",
            detail: "GitHub API returned an error"
        };
    }

    // Parse the JSON response using lazy path extraction
    let body = response.body();
    let parsed = match json::try_parse(&body) {
        Some(p) => p,
        None => {
            return error! {
                status: 502,
                title: "Bad Gateway",
                detail: "Invalid JSON from GitHub API"
            };
        },
    };

    // Extract fields using path_* methods (fast, no tree building)
    let login = parsed.path_str_or(&["login"], "unknown");
    let id = parsed.path_int_or(&["id"], 0);
    let name = parsed.path_str(&["name"]);
    let bio = parsed.path_str(&["bio"]);
    let public_repos = parsed.path_int_or(&["public_repos"], 0);

    ok!({
        "login": login,
        "id": id,
        "name": name,
        "bio": bio,
        "public_repos": public_repos
    })
}

/// Proxy external URL with SSRF protection.
///
/// Demonstrates:
/// - `deny_private_ips()` for SSRF protection
/// - Passing through response status and body
fn proxy(query: ProxyQuery, req: &Request) -> Response {
    log!(info, "proxying url", url: &query.url);

    // CRITICAL: Use deny_private_ips() when the URL comes from user input!
    // This prevents Server-Side Request Forgery (SSRF) attacks.
    let result = fetch!(GET &query.url,
        timeout: 10000
    )
    .deny_private_ips() // Block requests to localhost, 10.x, 192.168.x, etc.
    .with_trace_id(req.trace_id())
    .send();

    let response = match result {
        Ok(r) => r,
        Err(e) => {
            let error_msg = e.to_string();
            // Check if it's an SSRF block
            if error_msg.contains("SSRF") || error_msg.contains("private") {
                log!(warn, "ssrf blocked", url: &query.url);
                return error! {
                    status: 403,
                    title: "Forbidden",
                    detail: "Requests to private/internal addresses are not allowed"
                };
            }
            log!(error, "proxy failed", url: &query.url, error: &error_msg);
            return error! {
                status: 502,
                title: "Bad Gateway",
                detail: "Failed to fetch URL"
            };
        },
    };

    // Return proxy response
    let status = response.status();
    let response_headers = response.headers();

    // Collect some response headers for debugging
    let headers: Vec<String> = response_headers
        .iter()
        .take(5)
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();

    let body = response.body();
    let body_str = String::from_utf8_lossy(&body);

    // Build headers array
    let headers_arr = headers
        .iter()
        .fold(json::arr(), |arr, h| arr.push(json::str(h)));

    let truncated_body: String = body_str.chars().take(1000).collect();
    ok!({
        "status": status as i64,
        "body": truncated_body,
        "headers": headers_arr
    })
}

/// Send a webhook to an external service.
///
/// Demonstrates:
/// - POST request with JSON body
/// - Custom headers
fn send_webhook(body: WebhookPayload, req: &Request) -> Response {
    // In a real app, this URL would come from configuration
    let webhook_url = "https://httpbin.org/post";

    log!(info, "sending webhook", event: &body.event);

    let result = fetch!(POST webhook_url,
        headers: {
            "Content-Type": "application/json",
            "X-Webhook-Event": &body.event
        },
        json: {
            "event": body.event,
            "data": body.data,
            "timestamp": time::now_iso()
        },
        timeout: 5000
    )
    .with_trace_id(req.trace_id())
    .send();

    match result {
        Ok(response) if response.is_success() => {
            log!(info, "webhook sent", event: &body.event, status: response.status());
            ok!({
                "success": true,
                "message": "Webhook delivered successfully"
            })
        },
        Ok(response) => {
            log!(warn, "webhook failed", event: &body.event, status: response.status());
            let message = format!("Webhook returned status {}", response.status());
            ok!({
                "success": false,
                "message": message
            })
        },
        Err(e) => {
            log!(error, "webhook error", event: &body.event, error: &e.to_string());
            error! {
                status: 502,
                title: "Bad Gateway",
                detail: "Failed to deliver webhook"
            }
        },
    }
}

/// Aggregate data from multiple external sources.
///
/// Demonstrates:
/// - Making multiple HTTP calls
/// - Combining results into a single response
fn aggregate(req: &Request) -> Response {
    log!(info, "aggregating data");

    // Note: In WASM, we can't do truly parallel requests.
    // These are sequential, but demonstrate the pattern.

    // Call 1: Get a UUID from httpbin
    let uuid_result = fetch!(GET "https://httpbin.org/uuid",
        timeout: 3000
    )
    .with_trace_id(req.trace_id())
    .send();

    let uuid = match uuid_result {
        Ok(r) if r.is_success() => {
            let body = r.body();
            json::try_parse(&body)
                .and_then(|p| p.path_str(&["uuid"]))
                .unwrap_or_else(|| "unknown".to_string())
        },
        _ => "error".to_string(),
    };

    // Call 2: Get headers echo from httpbin
    let headers_result = fetch!(GET "https://httpbin.org/headers",
        headers: {
            "X-Custom-Header": "mik-sdk-test"
        },
        timeout: 3000
    )
    .with_trace_id(req.trace_id())
    .send();

    let host = match headers_result {
        Ok(r) if r.is_success() => {
            let body = r.body();
            json::try_parse(&body)
                .and_then(|p| p.path_str(&["headers", "Host"]))
                .unwrap_or_else(|| "unknown".to_string())
        },
        _ => "error".to_string(),
    };

    // Call 3: Get IP address
    let ip_result = fetch!(GET "https://httpbin.org/ip",
        timeout: 3000
    )
    .with_trace_id(req.trace_id())
    .send();

    let origin_ip = match ip_result {
        Ok(r) if r.is_success() => {
            let body = r.body();
            json::try_parse(&body)
                .and_then(|p| p.path_str(&["origin"]))
                .unwrap_or_else(|| "unknown".to_string())
        },
        _ => "error".to_string(),
    };

    let timestamp = time::now_iso();
    ok!({
        "uuid": uuid,
        "host": host,
        "origin_ip": origin_ip,
        "timestamp": timestamp
    })
}
