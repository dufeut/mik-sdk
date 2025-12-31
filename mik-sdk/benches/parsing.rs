#![allow(clippy::indexing_slicing)] // Benchmark code uses indexing for setup
#![allow(clippy::unwrap_used)] // Benchmark code uses unwrap for setup
//! Benchmarks for mik-sdk parsing operations.
//!
//! Run with: cargo bench -p mik-sdk

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mik_sdk::{Method, Request, json};
use std::collections::HashMap;
use std::hint::black_box;

// =============================================================================
// Request Creation Benchmarks
// =============================================================================

fn bench_request_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_creation");

    // Minimal request
    group.bench_function("minimal", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box("/".to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    // With query string
    group.bench_function("with_query", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box("/api/users?page=1&limit=50&sort=name".to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    // With headers
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), "Bearer token123".to_string()),
        ("accept".to_string(), "application/json".to_string()),
        ("user-agent".to_string(), "MikSDK/1.0".to_string()),
        ("x-request-id".to_string(), "abc-123-def".to_string()),
    ];
    group.bench_function("with_5_headers", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box("/".to_string()),
                headers.clone(),
                None,
                HashMap::new(),
            )
        });
    });

    // With body
    let body = r#"{"name": "test", "email": "test@example.com"}"#.as_bytes().to_vec();
    group.bench_function("with_json_body", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                black_box("/api/users".to_string()),
                vec![("content-type".to_string(), "application/json".to_string())],
                Some(body.clone()),
                HashMap::new(),
            )
        });
    });

    // Full request (headers + query + body)
    let full_body = br#"{"name": "test"}"#.to_vec();
    group.bench_function("full_request", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                black_box("/api/users?include=posts&fields=id,name".to_string()),
                vec![
                    ("content-type".to_string(), "application/json".to_string()),
                    ("authorization".to_string(), "Bearer token".to_string()),
                ],
                Some(full_body.clone()),
                HashMap::new(),
            )
        });
    });

    group.finish();
}

// =============================================================================
// Request Access Benchmarks
// =============================================================================

fn bench_request_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_access");

    // Create a realistic request once
    let req = Request::new(
        Method::Post,
        "/api/users/123?include=posts&fields=id,name,email&sort=-created_at".to_string(),
        vec![
            ("content-type".to_string(), "application/json".to_string()),
            (
                "authorization".to_string(),
                "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".to_string(),
            ),
            ("accept".to_string(), "application/json".to_string()),
            (
                "x-request-id".to_string(),
                "550e8400-e29b-41d4-a716-446655440000".to_string(),
            ),
            ("user-agent".to_string(), "Mozilla/5.0".to_string()),
        ],
        Some(
            r#"{"name": "Alice", "email": "alice@example.com"}"#
                .as_bytes()
                .to_vec(),
        ),
        {
            let mut params = HashMap::new();
            params.insert("id".to_string(), "123".to_string());
            params
        },
    );

    // Query access
    group.bench_function("query_hit", |b| b.iter(|| req.query(black_box("include"))));

    group.bench_function("query_miss", |b| {
        b.iter(|| req.query(black_box("nonexistent")));
    });

    // Header access
    group.bench_function("header_hit_lowercase", |b| {
        b.iter(|| req.header(black_box("content-type")));
    });

    group.bench_function("header_hit_uppercase", |b| {
        b.iter(|| req.header(black_box("CONTENT-TYPE")));
    });

    group.bench_function("header_miss", |b| {
        b.iter(|| req.header(black_box("x-nonexistent")));
    });

    // Path param access
    group.bench_function("param_hit", |b| b.iter(|| req.param(black_box("id"))));

    // Body access
    group.bench_function("body", |b| b.iter(|| req.body()));

    group.bench_function("text", |b| b.iter(|| req.text()));

    // Method access
    group.bench_function("method", |b| b.iter(|| req.method()));

    group.finish();
}

// =============================================================================
// Query Parsing Benchmarks
// =============================================================================

fn bench_query_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_parsing");

    let test_cases = [
        ("1_param", "?a=1"),
        ("5_params", "?a=1&b=2&c=3&d=4&e=5"),
        ("10_params", "?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8&i=9&j=10"),
        ("encoded", "?name=hello%20world&q=foo%26bar"),
        ("array", "?tags=a&tags=b&tags=c&tags=d&tags=e"),
    ];

    for (name, query) in test_cases {
        let path = format!("/api{query}");
        group.bench_with_input(BenchmarkId::new("parse", name), &path, |b, p| {
            b.iter(|| {
                Request::new(
                    Method::Get,
                    black_box(p.clone()),
                    vec![],
                    None,
                    HashMap::new(),
                )
            });
        });
    }

    group.finish();
}

// =============================================================================
// JSON Parsing Benchmarks
// =============================================================================

fn bench_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing");

    // Simple object
    let simple = br#"{"name":"Alice","age":30}"#;
    group.bench_function("simple_object", |b| {
        b.iter(|| json::try_parse(black_box(simple)));
    });

    // Nested object
    let nested = br#"{"user":{"name":"Alice","profile":{"bio":"Hello","avatar":"url"}}}"#;
    group.bench_function("nested_object", |b| {
        b.iter(|| json::try_parse(black_box(nested)));
    });

    // Array of primitives
    let array_primitives = br"[1,2,3,4,5,6,7,8,9,10]";
    group.bench_function("array_10_ints", |b| {
        b.iter(|| json::try_parse(black_box(array_primitives)));
    });

    // Array of objects (common API response pattern)
    let array_objects = br#"[{"id":1,"name":"a"},{"id":2,"name":"b"},{"id":3,"name":"c"},{"id":4,"name":"d"},{"id":5,"name":"e"}]"#;
    group.bench_function("array_5_objects", |b| {
        b.iter(|| json::try_parse(black_box(array_objects)));
    });

    // Realistic API response
    let api_response = br#"{"data":{"users":[{"id":"123","name":"Alice","email":"alice@example.com","active":true},{"id":"456","name":"Bob","email":"bob@example.com","active":false}]},"meta":{"total":2,"page":1}}"#;
    group.bench_function("api_response", |b| {
        b.iter(|| json::try_parse(black_box(api_response)));
    });

    // Large string value
    let large_string = format!(r#"{{"content":"{}"}}"#, "x".repeat(1000));
    let large_bytes = large_string.as_bytes();
    group.bench_function("large_string_1kb", |b| {
        b.iter(|| json::try_parse(black_box(large_bytes)));
    });

    group.finish();
}

// =============================================================================
// JSON Building Benchmarks
// =============================================================================

fn bench_json_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_building");

    // Simple object
    group.bench_function("simple_object", |b| {
        b.iter(|| {
            json::obj()
                .set("name", json::str(black_box("Alice")))
                .set("age", json::int(black_box(30)))
        });
    });

    // Nested object
    group.bench_function("nested_object", |b| {
        b.iter(|| {
            json::obj().set(
                "user",
                json::obj().set("name", json::str("Alice")).set(
                    "profile",
                    json::obj()
                        .set("bio", json::str("Hello"))
                        .set("avatar", json::str("url")),
                ),
            )
        });
    });

    // Array building
    group.bench_function("array_10_items", |b| {
        b.iter(|| {
            json::arr()
                .push(json::int(1))
                .push(json::int(2))
                .push(json::int(3))
                .push(json::int(4))
                .push(json::int(5))
                .push(json::int(6))
                .push(json::int(7))
                .push(json::int(8))
                .push(json::int(9))
                .push(json::int(10))
        });
    });

    // API response pattern
    group.bench_function("api_response", |b| {
        b.iter(|| {
            json::obj()
                .set(
                    "data",
                    json::obj().set(
                        "users",
                        json::arr()
                            .push(
                                json::obj()
                                    .set("id", json::str("123"))
                                    .set("name", json::str("Alice"))
                                    .set("active", json::bool(true)),
                            )
                            .push(
                                json::obj()
                                    .set("id", json::str("456"))
                                    .set("name", json::str("Bob"))
                                    .set("active", json::bool(false)),
                            ),
                    ),
                )
                .set(
                    "meta",
                    json::obj()
                        .set("total", json::int(2))
                        .set("page", json::int(1)),
                )
        });
    });

    group.finish();
}

// =============================================================================
// JSON Access Benchmarks
// =============================================================================

fn bench_json_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_access");

    // Parse once, access many times
    let data = br#"{"user":{"name":"Alice","profile":{"bio":"Hello"}},"items":[1,2,3,4,5]}"#;
    let parsed = json::try_parse(data).unwrap();

    // Direct field access
    group.bench_function("get_field", |b| b.iter(|| parsed.get(black_box("user"))));

    // Chained field access (clones intermediate values)
    group.bench_function("get_nested", |b| {
        b.iter(|| parsed.get("user").get("profile").get("bio"));
    });

    // Path-based access (zero intermediate clones)
    group.bench_function("path_str_nested", |b| {
        b.iter(|| parsed.path_str(black_box(&["user", "profile", "bio"])));
    });

    // Compare: chained get().str() vs path_str()
    group.bench_function("get_chain_to_str", |b| {
        b.iter(|| parsed.get("user").get("name").str());
    });

    group.bench_function("path_str_2_levels", |b| {
        b.iter(|| parsed.path_str(black_box(&["user", "name"])));
    });

    // Array access
    group.bench_function("get_array_element", |b| {
        b.iter(|| parsed.get("items").at(black_box(2)));
    });

    // Value extraction
    let user = parsed.get("user");
    group.bench_function("str_or", |b| {
        b.iter(|| user.get("name").str_or(black_box("default")));
    });

    group.bench_function("int_or", |b| {
        let items = parsed.get("items");
        b.iter(|| items.at(0).int_or(black_box(0)));
    });

    // Array length
    group.bench_function("len", |b| b.iter(|| parsed.get("items").len()));

    group.finish();
}

// =============================================================================
// JSON Serialization Benchmarks
// =============================================================================

fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Simple object
    let simple = json::obj()
        .set("name", json::str("Alice"))
        .set("age", json::int(30));
    group.bench_function("simple_object", |b| b.iter(|| simple.to_string()));

    // Complex object
    let complex = json::obj()
        .set(
            "data",
            json::obj().set(
                "users",
                json::arr().push(
                    json::obj()
                        .set("id", json::str("123"))
                        .set("name", json::str("Alice"))
                        .set("active", json::bool(true)),
                ),
            ),
        )
        .set("meta", json::obj().set("total", json::int(1)));
    group.bench_function("complex_object", |b| b.iter(|| complex.to_string()));

    // To bytes
    group.bench_function("to_bytes", |b| b.iter(|| simple.to_bytes()));

    group.finish();
}

// =============================================================================
// Form Parsing Benchmarks
// =============================================================================

fn bench_form_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("form_parsing");

    // Simple form
    let simple_form = b"name=Alice&email=alice%40example.com";
    group.bench_function("simple_2_fields", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                "/submit".to_string(),
                vec![(
                    "content-type".to_string(),
                    "application/x-www-form-urlencoded".to_string(),
                )],
                Some(black_box(simple_form.to_vec())),
                HashMap::new(),
            )
        });
    });

    // Realistic form (login)
    let login_form =
        b"username=alice%40example.com&password=secret123&remember=on&csrf=abc123def456";
    group.bench_function("login_form_4_fields", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                "/login".to_string(),
                vec![(
                    "content-type".to_string(),
                    "application/x-www-form-urlencoded".to_string(),
                )],
                Some(black_box(login_form.to_vec())),
                HashMap::new(),
            )
        });
    });

    // Form access after parsing
    let req = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        Some(b"name=Alice&email=alice%40example.com&age=30&city=NYC".to_vec()),
        HashMap::new(),
    );

    group.bench_function("form_field_access", |b| {
        b.iter(|| req.form(black_box("email")));
    });

    group.bench_function("is_form_check", |b| b.iter(|| req.is_form()));

    group.finish();
}

// =============================================================================
// URL Decoding Benchmarks
// =============================================================================

fn bench_url_decoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("url_decoding");

    // No encoding needed (fast path)
    let no_encoding = "/api/users/123";
    group.bench_function("no_encoding", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box(no_encoding.to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    // Common URL encoding (spaces as %20)
    let with_spaces = "/api/search?q=hello%20world%20test";
    group.bench_function("with_spaces", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box(with_spaces.to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    // Heavy encoding
    let heavy_encoding =
        "/api/search?q=%E4%B8%AD%E6%96%87%E6%B5%8B%E8%AF%95&filter=%5B%7B%22a%22%3A1%7D%5D";
    group.bench_function("heavy_encoding", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box(heavy_encoding.to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    // Plus as space (form encoding style)
    let plus_spaces = "/api/search?q=hello+world+from+form";
    group.bench_function("plus_as_space", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box(plus_spaces.to_string()),
                vec![],
                None,
                HashMap::new(),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_request_creation,
    bench_request_access,
    bench_query_parsing,
    bench_json_parsing,
    bench_json_building,
    bench_json_access,
    bench_json_serialization,
    bench_form_parsing,
    bench_url_decoding,
);

criterion_main!(benches);
