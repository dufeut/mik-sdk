//! Auth API Example - Authentication patterns, error handling, and logging.
//!
//! Demonstrates real-world patterns:
//! - API key authentication via headers
//! - Protected routes with guard!/ensure! macros
//! - Structured error handling
//! - Logging patterns
//! - Environment variable access
//!
//! Note: This is a demo. In production, use proper JWT libraries
//! and secure token validation.

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
    pub auth_required: Vec<String>,
}

#[derive(Type)]
pub struct LoginInput {
    #[field(min = 3, max = 50)]
    pub username: String,
    #[field(min = 8)]
    pub password: String,
}

#[derive(Type)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: i64,
}

#[derive(Type)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub role: String,
}

#[derive(Type)]
pub struct ProtectedData {
    pub message: String,
    pub user_id: String,
    pub accessed_at: String,
}

// ============================================================================
// ROUTES
// ============================================================================

routes! {
    // Public endpoints
    GET "/" => index -> IndexResponse,
    POST "/login" => login(body: LoginInput) -> LoginResponse,
    GET "/health" => health,

    // Protected endpoints (require authentication)
    GET "/me" => get_profile -> UserProfile,
    GET "/protected" => protected_resource -> ProtectedData,
    POST "/logout" => logout,
}

// ============================================================================
// PUBLIC HANDLERS
// ============================================================================

fn index(_req: &Request) -> Response {
    ok!({
        "name": "Auth API Example",
        "version": "0.1.0",
        "auth_required": ["/me", "/protected", "/logout"]
    })
}

fn health(_req: &Request) -> Response {
    // Simple health check - useful for load balancers
    let timestamp = time::now_iso();
    ok!({ "status": "healthy", "timestamp": timestamp })
}

/// Login endpoint - validates credentials and returns a token.
fn login(body: LoginInput, _req: &Request) -> Response {
    // Log login attempt (structured logging)
    log!(info, "login attempt", username: &body.username);

    // Demo: Accept any username with password "password123"
    // In production: validate against database, use proper password hashing
    if body.password != "password123" {
        log!(warn, "login failed", username: &body.username, reason: "invalid_password");
        return error! {
            status: status::UNAUTHORIZED,
            title: "Unauthorized",
            detail: "Invalid username or password"
        };
    }

    // Generate a demo token (in production: use proper JWT library)
    // Token format: base64(username:timestamp:random)
    let token = generate_demo_token(&body.username);
    let expires_in: i64 = 3600; // 1 hour

    log!(info, "login success", username: &body.username);

    ok!({
        "token": token,
        "expires_in": expires_in
    })
}

// ============================================================================
// PROTECTED HANDLERS
// ============================================================================

/// Get current user's profile - requires authentication.
fn get_profile(req: &Request) -> Response {
    // Extract and validate token
    let user = match authenticate(req) {
        Ok(u) => u,
        Err(response) => return response,
    };

    ok!({
        "id": user.id,
        "username": user.username,
        "role": user.role
    })
}

/// Access protected resource - demonstrates guard! and ensure! macros.
fn protected_resource(req: &Request) -> Response {
    // Method 1: Using authenticate helper (recommended)
    let user = match authenticate(req) {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Method 2: Using guard! for additional checks
    // guard! returns early with error if condition is false
    guard!(
        user.role == "admin" || user.role == "user",
        403,
        "Insufficient permissions"
    );

    // Log access to protected resource
    log!(info, "protected access", user_id: &user.id, resource: "protected_data");

    let accessed_at = time::now_iso();
    ok!({
        "message": "You have accessed the protected resource!",
        "user_id": user.id,
        "accessed_at": accessed_at
    })
}

/// Logout - invalidate token (demo only, tokens are stateless here).
fn logout(req: &Request) -> Response {
    let user = match authenticate(req) {
        Ok(u) => u,
        Err(response) => return response,
    };

    log!(info, "logout", user_id: &user.id);

    // In production: add token to blacklist or use short-lived tokens with refresh
    no_content!()
}

// ============================================================================
// AUTHENTICATION HELPERS
// ============================================================================

/// Represents an authenticated user.
struct AuthUser {
    id: String,
    username: String,
    role: String,
}

/// Authenticate request using Bearer token.
///
/// Returns `Ok(AuthUser)` if valid, `Err(Response)` with error otherwise.
fn authenticate(req: &Request) -> Result<AuthUser, Response> {
    // Get Authorization header
    let auth_header = req.header("authorization").ok_or_else(|| {
        error! {
            status: status::UNAUTHORIZED,
            title: "Unauthorized",
            detail: "Missing Authorization header"
        }
    })?;

    // Check Bearer scheme
    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        error! {
            status: status::UNAUTHORIZED,
            title: "Unauthorized",
            detail: "Invalid Authorization scheme, expected 'Bearer <token>'"
        }
    })?;

    // Validate token (demo: decode and check format)
    let user = validate_demo_token(token).ok_or_else(|| {
        log!(warn, "invalid token", token_prefix: &token.chars().take(10).collect::<String>());
        error! {
            status: status::UNAUTHORIZED,
            title: "Unauthorized",
            detail: "Invalid or expired token"
        }
    })?;

    Ok(user)
}

/// Generate a demo token (NOT FOR PRODUCTION).
///
/// In production, use a proper JWT library with:
/// - RS256 or ES256 signing
/// - Proper expiration handling
/// - Secure secret management
fn generate_demo_token(username: &str) -> String {
    // Simple demo token: username.timestamp.random
    let timestamp = time::now();
    let random_part = random::hex(8);
    let payload = format!("{username}.{timestamp}.{random_part}");

    // Base64-like encoding (simplified for demo)
    // In production: use proper base64 + HMAC signature
    hex_encode(payload.as_bytes())
}

/// Validate a demo token and extract user info.
fn validate_demo_token(token: &str) -> Option<AuthUser> {
    // Decode the token
    let decoded = hex_decode(token)?;
    let payload = String::from_utf8(decoded).ok()?;

    // Parse: username.timestamp.random
    let parts: Vec<&str> = payload.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let username = parts[0];
    let timestamp: u64 = parts[1].parse().ok()?;

    // Check expiration (1 hour = 3600 seconds)
    let now = time::now();
    if now - timestamp > 3600 {
        return None;
    }

    // Return user (in production: look up from database)
    Some(AuthUser {
        id: format!("user_{}", random::hex(4)),
        username: username.to_string(),
        role: if username == "admin" { "admin" } else { "user" }.to_string(),
    })
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Simple hex encoding (demo purposes).
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Simple hex decoding (demo purposes).
#[allow(unknown_lints, clippy::manual_is_multiple_of)]
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }

    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
