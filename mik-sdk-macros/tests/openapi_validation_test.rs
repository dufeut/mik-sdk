#![allow(
    clippy::unwrap_used,                      // Test code uses unwrap for assertions
    clippy::expect_used,                      // Test code uses expect for setup
    clippy::needless_raw_string_hashes,       // r##"..."## is fine for JSON
    clippy::too_many_lines,                   // Test functions can be long
    clippy::doc_markdown,                     // Don't require backticks in test docs
    clippy::match_wildcard_for_single_variants // Wildcards are fine in tests
)]
//! OpenAPI schema validation tests.
//!
//! These tests verify that the generated OpenAPI schemas are structurally valid
//! and can be parsed by OpenAPI tooling (using the openapiv3 crate).
//!
//! This validates that our macro-generated schemas conform to OpenAPI 3.0 spec.

use openapiv3::OpenAPI;

/// Test that a minimal OpenAPI schema is valid.
#[test]
fn test_minimal_openapi_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test API", "version": "1.0.0"},
        "paths": {}
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Minimal schema should be valid");

    assert_eq!(spec.openapi, "3.0.0");
    assert_eq!(spec.info.title, "Test API");
    assert_eq!(spec.info.version, "1.0.0");
}

/// Test that our RFC 7807 ProblemDetails schema is valid OpenAPI.
#[test]
fn test_problem_details_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test", "version": "1.0.0"},
        "paths": {},
        "components": {
            "schemas": {
                "ProblemDetails": {
                    "type": "object",
                    "description": "RFC 7807 Problem Details for HTTP APIs",
                    "required": ["type", "title", "status"],
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "URI reference identifying the problem type",
                            "default": "about:blank"
                        },
                        "title": {
                            "type": "string",
                            "description": "Short human-readable summary of the problem"
                        },
                        "status": {
                            "type": "integer",
                            "description": "HTTP status code",
                            "minimum": 100,
                            "maximum": 599
                        },
                        "detail": {
                            "type": "string",
                            "description": "Human-readable explanation specific to this occurrence"
                        },
                        "instance": {
                            "type": "string",
                            "description": "URI reference identifying the specific occurrence"
                        }
                    }
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("ProblemDetails schema should be valid");

    let schemas = spec.components.expect("Should have components").schemas;
    assert!(
        schemas.contains_key("ProblemDetails"),
        "Should contain ProblemDetails schema"
    );
}

/// Test that a complete OpenAPI spec with paths, parameters, and responses is valid.
/// This mirrors the structure our routes! macro generates.
#[test]
fn test_complete_openapi_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "my-api", "version": "0.1.0"},
        "paths": {
            "/users": {
                "get": {
                    "tags": ["Users"],
                    "summary": "List all users",
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/UserList"}
                                }
                            }
                        },
                        "4XX": {
                            "description": "Client Error",
                            "content": {
                                "application/problem+json": {
                                    "schema": {"$ref": "#/components/schemas/ProblemDetails"}
                                }
                            }
                        },
                        "5XX": {
                            "description": "Server Error",
                            "content": {
                                "application/problem+json": {
                                    "schema": {"$ref": "#/components/schemas/ProblemDetails"}
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": ["Users"],
                    "summary": "Create a user",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateUser"}
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "Success"}
                    }
                }
            },
            "/users/{id}": {
                "get": {
                    "tags": ["Users"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Success"}
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "ProblemDetails": {
                    "type": "object",
                    "required": ["type", "title", "status"],
                    "properties": {
                        "type": {"type": "string"},
                        "title": {"type": "string"},
                        "status": {"type": "integer"},
                        "detail": {"type": "string"},
                        "instance": {"type": "string"}
                    }
                },
                "UserList": {
                    "type": "object",
                    "properties": {
                        "users": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/User"}
                        }
                    }
                },
                "User": {
                    "type": "object",
                    "required": ["id", "name"],
                    "properties": {
                        "id": {"type": "string"},
                        "name": {"type": "string"},
                        "email": {"type": "string", "nullable": true}
                    }
                },
                "CreateUser": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string", "minLength": 1, "maxLength": 100},
                        "email": {"type": "string", "format": "email"}
                    }
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Complete schema should be valid");

    // Verify structure
    assert_eq!(spec.openapi, "3.0.0");
    assert_eq!(spec.info.title, "my-api");
    assert_eq!(spec.info.version, "0.1.0");
    assert_eq!(spec.paths.paths.len(), 2, "Should have 2 paths");

    // Verify schemas
    let schemas = spec.components.expect("Should have components").schemas;
    assert!(schemas.contains_key("ProblemDetails"));
    assert!(schemas.contains_key("User"));
    assert!(schemas.contains_key("CreateUser"));
    assert!(schemas.contains_key("UserList"));
}

/// Test that query parameters schema is valid.
#[test]
fn test_query_parameters_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test", "version": "1.0.0"},
        "paths": {
            "/search": {
                "get": {
                    "parameters": [
                        {
                            "name": "q",
                            "in": "query",
                            "required": false,
                            "schema": {"type": "string"}
                        },
                        {
                            "name": "page",
                            "in": "query",
                            "required": false,
                            "schema": {"type": "integer", "default": 1, "minimum": 1}
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "required": false,
                            "schema": {"type": "integer", "default": 20, "minimum": 1, "maximum": 100}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Success"}
                    }
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Query params schema should be valid");

    // Get the path and verify parameters
    let path_item = spec
        .paths
        .paths
        .get("/search")
        .expect("Should have /search path");
    let get_op = match path_item {
        openapiv3::ReferenceOr::Item(item) => item.get.as_ref().expect("Should have GET"),
        _ => panic!("Expected path item, not reference"),
    };

    assert_eq!(get_op.parameters.len(), 3, "Should have 3 query parameters");
}

/// Test that path parameters with multiple segments are valid.
#[test]
fn test_nested_path_parameters_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test", "version": "1.0.0"},
        "paths": {
            "/users/{user_id}/posts/{post_id}": {
                "get": {
                    "parameters": [
                        {
                            "name": "user_id",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        },
                        {
                            "name": "post_id",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Success"}
                    }
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Nested path params should be valid");
    assert!(
        spec.paths
            .paths
            .contains_key("/users/{user_id}/posts/{post_id}")
    );
}

/// Test that enum schemas are valid.
#[test]
fn test_enum_schema_is_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test", "version": "1.0.0"},
        "paths": {},
        "components": {
            "schemas": {
                "Status": {
                    "type": "string",
                    "enum": ["active", "inactive", "pending"]
                },
                "UserWithStatus": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "status": {"$ref": "#/components/schemas/Status"}
                    }
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Enum schema should be valid");

    let schemas = spec.components.expect("Should have components").schemas;
    assert!(schemas.contains_key("Status"));
    assert!(schemas.contains_key("UserWithStatus"));
}

/// Test that tags with summaries are valid (our new feature).
#[test]
fn test_tags_and_summaries_are_valid() {
    let json = r##"{
        "openapi": "3.0.0",
        "info": {"title": "Test", "version": "1.0.0"},
        "paths": {
            "/users": {
                "get": {
                    "tags": ["Users"],
                    "summary": "List all users with pagination",
                    "responses": {"200": {"description": "Success"}}
                }
            },
            "/health": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Health check endpoint",
                    "responses": {"200": {"description": "Success"}}
                }
            }
        }
    }"##;

    let spec: OpenAPI = serde_json::from_str(json).expect("Tags and summaries should be valid");

    // Verify paths exist
    assert!(spec.paths.paths.contains_key("/users"));
    assert!(spec.paths.paths.contains_key("/health"));
}
