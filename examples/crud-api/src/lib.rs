#![allow(clippy::cast_possible_wrap)] // usize to i64 is fine for this example

//! CRUD API Example - Demonstrates REST patterns with typed inputs.
//!
//! This example shows how to build a typical CRUD API with:
//! - GET /users - list users (offset pagination)
//! - GET /users/{id} - get user by ID
//! - POST /users - create user
//! - PUT /users/{id} - update user
//! - DELETE /users/{id} - delete user
//! - GET /posts - list posts (cursor pagination)
//!
//! Demonstrates typed inputs with derive macros:
//! - `#[derive(Type)]`  - JSON body/response types
//! - `#[derive(Path)]`  - URL path parameters
//! - `#[derive(Query)]` - URL query parameters
//!
//! Also demonstrates SQL query generation with the CRUD macros.
//!
//! Note: WASM handlers are stateless. In production, you'd use
//! external storage via host capabilities or outbound HTTP.

#[allow(warnings)]
mod bindings;

use bindings::exports::mik::core::handler::{self, Guest, Response};
use bindings::wasi::clocks::wall_clock;
use mik_sdk::prelude::*;
use mik_sql::{Cursor, PageInfo, sql_create, sql_delete, sql_read, sql_update};

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

// --- Path Parameters ---

#[derive(Path)]
pub struct UserPath {
    pub id: String,
}

// --- Query Parameters ---

#[derive(Query)]
pub struct ListUsersQuery {
    #[field(default = 1)]
    pub page: u32,
    #[field(default = 50, max = 100)]
    pub limit: u32,
}

#[derive(Query)]
pub struct ListPostsQuery {
    pub after: Option<String>,
    #[field(default = 20, max = 100)]
    pub limit: u32,
}

// --- Request Bodies ---

#[derive(Type)]
pub struct CreateUserInput {
    #[field(min = 1, max = 100)]
    pub name: String,
    #[field(format = "email")]
    pub email: String,
}

#[derive(Type)]
pub struct UpdateUserInput {
    pub name: Option<String>,
    pub email: Option<String>,
}

// --- Response Types ---

#[derive(Type)]
pub struct IndexResponse {
    pub name: String,
    pub version: String,
}

#[derive(Type)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Type)]
pub struct UserListResponse {
    pub users: Vec<User>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
}

#[derive(Type)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Type)]
pub struct PostListResponse {
    pub posts: Vec<Post>,
    pub has_next: bool,
    pub has_prev: bool,
    pub next_cursor: Option<String>,
}

// ============================================================================
// ROUTES
// ============================================================================

routes! {
    GET "/" => index -> IndexResponse,

    // Users - list and create
    GET  "/users" => list_users(query: ListUsersQuery) -> UserListResponse,
    POST "/users" => create_user(body: CreateUserInput) -> User,

    // Users - by ID
    GET    "/users/{id}" => get_user(path: UserPath) -> User,
    PUT    "/users/{id}" => update_user(path: UserPath, body: UpdateUserInput) -> User,
    DELETE "/users/{id}" => delete_user(path: UserPath),

    // Posts - cursor pagination
    GET "/posts" => list_posts(query: ListPostsQuery) -> PostListResponse,
}

// ============================================================================
// HANDLERS
// ============================================================================

fn index(_req: &Request) -> Response {
    ok!({
        "name": "CRUD API Example",
        "version": "0.1.0",
        "endpoints": {
            "list_users": "GET /users?page=1&limit=50",
            "get_user": "GET /users/{id}",
            "create_user": "POST /users",
            "update_user": "PUT /users/{id}",
            "delete_user": "DELETE /users/{id}",
            "list_posts": "GET /posts?after={cursor}"
        }
    })
}

/// List users with offset-based pagination.
fn list_users(query: ListUsersQuery, _req: &Request) -> Response {
    // Generate SQL query for listing users (offset pagination)
    let (sql, params) = sql_read!(users {
        select: [id, name, email],
        filter: { active: true },
        order: name,
        page: query.page,
        limit: query.limit,
    });

    // In production: execute query against database
    // For demo, return mock data with generated SQL
    let param_count = params.len();
    ok!({
        "users": [
            { "id": "1", "name": "Alice", "email": "alice@example.com" },
            { "id": "2", "name": "Bob", "email": "bob@example.com" }
        ],
        "page": query.page,
        "limit": query.limit,
        "total": 2,
        "_debug": {
            "sql": sql,
            "param_count": param_count
        }
    })
}

/// Get a single user by ID.
fn get_user(path: UserPath, _req: &Request) -> Response {
    let id = &path.id;

    // Parse ID to integer for SQL query
    let Ok(user_id) = id.parse::<i64>() else {
        return error! {
            status: status::BAD_REQUEST,
            title: "Bad Request",
            detail: "User ID must be a number"
        };
    };

    // Generate SQL query
    let (sql, _params) = sql_read!(users {
        select: [id, name, email, created_at],
        filter: { id: int(user_id) },
        limit: 1,
    });

    // In production: execute query against database
    match id.as_str() {
        "1" => ok!({
            "id": "1",
            "name": "Alice",
            "email": "alice@example.com",
            "created_at": "2024-01-15T10:30:00Z",
            "_debug": { "sql": sql }
        }),
        "2" => ok!({
            "id": "2",
            "name": "Bob",
            "email": "bob@example.com",
            "created_at": "2024-02-20T14:45:00Z",
            "_debug": { "sql": sql }
        }),
        _ => {
            let detail = format!("User '{}' not found", id);
            error! {
                status: status::NOT_FOUND,
                title: "Not Found",
                detail: &detail
            }
        },
    }
}

/// Create a new user.
fn create_user(body: CreateUserInput, _req: &Request) -> Response {
    // body is already parsed and validated by the derive macro!
    // name and email are guaranteed to be present

    // Generate SQL INSERT query
    let (sql, _params) = sql_create!(users {
        name: str(&body.name),
        email: str(&body.email),
        active: true,
        returning: [id, created_at],
    });

    // In production: execute query, get returned id and created_at
    let new_id = "3"; // Mock generated ID
    let location = format!("/users/{}", new_id);
    let created_at = get_iso_time();

    // Return 201 Created with the new resource
    Response {
        status: status::CREATED,
        headers: vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("location".to_string(), location),
        ],
        body: Some(
            json::obj()
                .set("id", json::str(new_id))
                .set("name", json::str(&body.name))
                .set("email", json::str(&body.email))
                .set("created_at", json::str(&created_at))
                .set("_debug", json::obj().set("sql", json::str(&sql)))
                .to_bytes(),
        ),
    }
}

/// Update an existing user.
fn update_user(path: UserPath, body: UpdateUserInput, _req: &Request) -> Response {
    let id = &path.id;

    // Parse and validate ID
    let Ok(user_id) = id.parse::<i64>() else {
        return error! {
            status: status::BAD_REQUEST,
            title: "Bad Request",
            detail: "User ID must be a number"
        };
    };

    // Check if user exists (in production: database lookup)
    if id != "1" && id != "2" {
        let detail = format!("User '{}' not found", id);
        return error! {
            status: status::NOT_FOUND,
            title: "Not Found",
            detail: &detail
        };
    }

    // At least one field must be provided
    if body.name.is_none() && body.email.is_none() {
        return error! {
            status: status::UNPROCESSABLE_ENTITY,
            title: "Validation Error",
            detail: "At least one field (name or email) is required"
        };
    }

    let name = body.name.as_deref().unwrap_or("");
    let email = body.email.as_deref().unwrap_or("");

    // Generate SQL UPDATE query
    let (sql, _params) = sql_update!(users {
        set: {
            name: str(name),
            email: str(email),
        },
        filter: { id: int(user_id) },
        returning: [id, name, email, updated_at],
    });

    // In production: execute query
    let final_name = if name.is_empty() { "Unchanged" } else { name };
    let final_email = if email.is_empty() {
        "unchanged@example.com"
    } else {
        email
    };

    let updated_at = get_iso_time();
    ok!({
        "id": id,
        "name": final_name,
        "email": final_email,
        "updated_at": updated_at,
        "_debug": { "sql": sql }
    })
}

/// Delete a user.
fn delete_user(path: UserPath, _req: &Request) -> Response {
    let id = &path.id;

    // Parse and validate ID
    let Ok(user_id) = id.parse::<i64>() else {
        return error! {
            status: status::BAD_REQUEST,
            title: "Bad Request",
            detail: "User ID must be a number"
        };
    };

    // Check if user exists (in production: database lookup)
    if id != "1" && id != "2" {
        let detail = format!("User '{}' not found", id);
        return error! {
            status: status::NOT_FOUND,
            title: "Not Found",
            detail: &detail
        };
    }

    // Generate SQL DELETE query
    let (_sql, _params) = sql_delete!(users {
        filter: { id: int(user_id) },
    });

    // In production: execute query
    // Return 204 No Content (successful deletion, no body)
    no_content!()
}

/// List posts with cursor-based pagination.
fn list_posts(query: ListPostsQuery, _req: &Request) -> Response {
    // sql_read! supports cursor pagination directly via after/before
    let (sql, params) = sql_read!(posts {
        select: [id, title, created_at],
        filter: { published: true },
        order: [-created_at, -id],  // DESC for stable cursor ordering
        after: query.after.as_deref(),
        limit: query.limit,
    });

    // Mock data for demo
    let mock_posts = [
        ("101", "Getting Started with WASM", "2025-01-15T10:00:00Z"),
        ("100", "Building REST APIs", "2025-01-14T09:00:00Z"),
        ("99", "Cursor Pagination Guide", "2025-01-13T08:00:00Z"),
    ];

    // Create cursor for the last item (for "next page" link)
    let last_post = mock_posts.last();
    let next_cursor = last_post.map(|(id, _, created_at)| {
        Cursor::new()
            .string("created_at", *created_at)
            .int("id", id.parse::<i64>().unwrap_or(0))
            .encode()
    });

    // Create page info
    let page_info = PageInfo::new(mock_posts.len(), query.limit as usize)
        .with_next_cursor(next_cursor.clone())
        .with_has_prev(query.after.is_some());

    // Build response
    let posts_json: Vec<_> = mock_posts
        .iter()
        .map(|(id, title, created_at)| {
            json::obj()
                .set("id", json::str(id))
                .set("title", json::str(title))
                .set("created_at", json::str(created_at))
        })
        .collect();

    let mut posts_array = json::arr();
    for post in posts_json {
        posts_array = posts_array.push(post);
    }

    Response {
        status: status::OK,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: Some(
            json::obj()
                .set("posts", posts_array)
                .set("has_next", json::bool(page_info.has_next))
                .set("has_prev", json::bool(page_info.has_prev))
                .set(
                    "next_cursor",
                    next_cursor.as_ref().map_or_else(json::null, json::str),
                )
                .set(
                    "_debug",
                    json::obj()
                        .set("sql", json::str(&sql))
                        .set("param_count", json::int(params.len() as i64)),
                )
                .to_bytes(),
        ),
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Get ISO time from WASI clock.
fn get_iso_time() -> String {
    let dt = wall_clock::now();
    time::to_iso(dt.seconds, dt.nanoseconds)
}
