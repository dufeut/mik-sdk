#![allow(clippy::too_many_lines)]
//! Benchmarks for Request operations: query parsing, header lookup, path parameters.
//!
//! Run with: cargo bench -p mik-sdk -- request

use criterion::{Criterion, criterion_group, criterion_main};
use mik_sdk::{Method, Request};
use std::collections::HashMap;
use std::hint::black_box;

// =============================================================================
// Query String Parsing Benchmarks
// =============================================================================

fn bench_query_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_parsing");

    // No query string (baseline)
    group.bench_function("no_query", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/users".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("page");
            black_box(result.is_some())
        });
    });

    // 1 parameter
    group.bench_function("1_param", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/users?page=1".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("page");
            black_box(result.is_some())
        });
    });

    // 3 parameters (typical pagination)
    group.bench_function("3_params", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/users?page=1&limit=50&sort=name".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("page");
            black_box(result.is_some())
        });
    });

    // 5 parameters (common filter query)
    group.bench_function("5_params", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box(
                    "/api/users?page=1&limit=50&sort=name&filter=active&include=posts".to_string(),
                ),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("page");
            black_box(result.is_some())
        });
    });

    // 10 parameters (complex query)
    group.bench_function("10_params", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box(
                    "/api/users?p1=v1&p2=v2&p3=v3&p4=v4&p5=v5&p6=v6&p7=v7&p8=v8&p9=v9&p10=v10"
                        .to_string(),
                ),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("p5");
            black_box(result.is_some())
        });
    });

    // URL-encoded parameters
    group.bench_function("url_encoded", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/search?q=hello%20world&filter=%5B%7B%22a%22%3A1%7D%5D".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("q");
            black_box(result.is_some())
        });
    });

    // Array parameters (same key multiple times)
    group.bench_function("array_5_values", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/users?ids=1&ids=2&ids=3&ids=4&ids=5".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query_all("ids");
            black_box(result.len())
        });
    });

    // Unicode parameters
    group.bench_function("unicode", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/search?q=%E4%B8%AD%E6%96%87&city=%E6%9D%B1%E4%BA%AC".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("q");
            black_box(result.is_some())
        });
    });

    // Plus-as-space encoding
    group.bench_function("plus_as_space", |b| {
        b.iter(|| {
            let req = Request::new(
                Method::Get,
                black_box("/api/search?q=hello+world+from+form".to_string()),
                vec![],
                None,
                HashMap::new(),
            );
            let result = req.query("q");
            black_box(result.is_some())
        });
    });

    group.finish();
}

// =============================================================================
// Query Access Benchmarks (after parsing)
// =============================================================================

fn bench_query_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_access");

    // Pre-create request with query string
    let req = Request::new(
        Method::Get,
        "/api/users?page=1&limit=50&sort=name&filter=active&include=posts&fields=id,name,email"
            .to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    // First access triggers lazy parsing
    let _ = req.query("page");

    // Subsequent accesses use cache
    group.bench_function("hit_first_param", |b| {
        b.iter(|| req.query(black_box("page")));
    });

    group.bench_function("hit_middle_param", |b| {
        b.iter(|| req.query(black_box("filter")));
    });

    group.bench_function("hit_last_param", |b| {
        b.iter(|| req.query(black_box("fields")));
    });

    group.bench_function("miss", |b| b.iter(|| req.query(black_box("nonexistent"))));

    // Array access
    let req_array = Request::new(
        Method::Get,
        "/api/users?tag=rust&tag=wasm&tag=http&tag=json&tag=api".to_string(),
        vec![],
        None,
        HashMap::new(),
    );
    let _ = req_array.query_all("tag");

    group.bench_function("query_all_5_values", |b| {
        b.iter(|| req_array.query_all(black_box("tag")));
    });

    group.finish();
}

// =============================================================================
// Header Lookup Benchmarks
// =============================================================================

fn bench_header_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_lookup");

    // Request with typical headers
    let req = Request::new(
        Method::Get,
        "/api/users".to_string(),
        vec![
            ("content-type".to_string(), "application/json".to_string()),
            (
                "authorization".to_string(),
                "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0".to_string(),
            ),
            (
                "accept".to_string(),
                "application/json, text/plain, */*".to_string(),
            ),
            (
                "user-agent".to_string(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string(),
            ),
            (
                "x-request-id".to_string(),
                "550e8400-e29b-41d4-a716-446655440000".to_string(),
            ),
            ("x-trace-id".to_string(), "abc123def456".to_string()),
            ("accept-language".to_string(), "en-US,en;q=0.9".to_string()),
            (
                "accept-encoding".to_string(),
                "gzip, deflate, br".to_string(),
            ),
            ("connection".to_string(), "keep-alive".to_string()),
            ("cache-control".to_string(), "no-cache".to_string()),
        ],
        None,
        HashMap::new(),
    );

    // Lowercase lookup (fast path - no allocation)
    group.bench_function("lowercase_hit", |b| {
        b.iter(|| req.header(black_box("content-type")));
    });

    group.bench_function("lowercase_miss", |b| {
        b.iter(|| req.header(black_box("x-nonexistent")));
    });

    // Mixed case lookup (slow path - needs lowercase allocation)
    group.bench_function("mixed_case_hit", |b| {
        b.iter(|| req.header(black_box("Content-Type")));
    });

    group.bench_function("uppercase_hit", |b| {
        b.iter(|| req.header(black_box("CONTENT-TYPE")));
    });

    // Header at different positions
    group.bench_function("first_header", |b| {
        b.iter(|| req.header(black_box("content-type")));
    });

    group.bench_function("middle_header", |b| {
        b.iter(|| req.header(black_box("x-request-id")));
    });

    group.bench_function("last_header", |b| {
        b.iter(|| req.header(black_box("cache-control")));
    });

    // Trace ID (common operation)
    group.bench_function("trace_id", |b| b.iter(|| req.trace_id()));

    group.finish();
}

// =============================================================================
// Header All Benchmarks (multiple values)
// =============================================================================

fn bench_header_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_all");

    // Request with multiple Set-Cookie headers
    let req = Request::new(
        Method::Get,
        "/".to_string(),
        vec![
            (
                "set-cookie".to_string(),
                "session=abc123; Path=/".to_string(),
            ),
            (
                "set-cookie".to_string(),
                "csrf=xyz789; Path=/; HttpOnly".to_string(),
            ),
            (
                "set-cookie".to_string(),
                "theme=dark; Path=/; Max-Age=31536000".to_string(),
            ),
            ("set-cookie".to_string(), "locale=en-US; Path=/".to_string()),
            (
                "set-cookie".to_string(),
                "tracking=opt-out; Path=/; Secure".to_string(),
            ),
            ("content-type".to_string(), "text/html".to_string()),
        ],
        None,
        HashMap::new(),
    );

    group.bench_function("5_cookies", |b| {
        b.iter(|| req.header_all(black_box("set-cookie")));
    });

    group.bench_function("single_value_header", |b| {
        b.iter(|| req.header_all(black_box("content-type")));
    });

    group.bench_function("nonexistent", |b| {
        b.iter(|| req.header_all(black_box("x-missing")));
    });

    group.finish();
}

// =============================================================================
// Path Parameter Extraction Benchmarks
// =============================================================================

fn bench_path_params(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_params");

    // Single path parameter
    let req_single = Request::new(
        Method::Get,
        "/api/users/550e8400-e29b-41d4-a716-446655440000".to_string(),
        vec![],
        None,
        {
            let mut params = HashMap::new();
            params.insert(
                "id".to_string(),
                "550e8400-e29b-41d4-a716-446655440000".to_string(),
            );
            params
        },
    );

    group.bench_function("single_param_hit", |b| {
        b.iter(|| req_single.param(black_box("id")));
    });

    group.bench_function("single_param_miss", |b| {
        b.iter(|| req_single.param(black_box("missing")));
    });

    // Multiple path parameters
    let req_multi = Request::new(
        Method::Get,
        "/api/orgs/org-123/users/user-456/posts/post-789".to_string(),
        vec![],
        None,
        {
            let mut params = HashMap::new();
            params.insert("org_id".to_string(), "org-123".to_string());
            params.insert("user_id".to_string(), "user-456".to_string());
            params.insert("post_id".to_string(), "post-789".to_string());
            params
        },
    );

    group.bench_function("multi_param_first", |b| {
        b.iter(|| req_multi.param(black_box("org_id")));
    });

    group.bench_function("multi_param_middle", |b| {
        b.iter(|| req_multi.param(black_box("user_id")));
    });

    group.bench_function("multi_param_last", |b| {
        b.iter(|| req_multi.param(black_box("post_id")));
    });

    // No path parameters
    let req_none = Request::new(
        Method::Get,
        "/api/health".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    group.bench_function("no_params_miss", |b| {
        b.iter(|| req_none.param(black_box("id")));
    });

    group.finish();
}

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

    // With 5 headers
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
                Method::Post,
                black_box("/api/users".to_string()),
                headers.clone(),
                None,
                HashMap::new(),
            )
        });
    });

    // With 10 headers
    let headers_10 = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), "Bearer token123".to_string()),
        ("accept".to_string(), "application/json".to_string()),
        ("user-agent".to_string(), "MikSDK/1.0".to_string()),
        ("x-request-id".to_string(), "abc-123-def".to_string()),
        ("x-trace-id".to_string(), "trace-456".to_string()),
        ("accept-language".to_string(), "en-US".to_string()),
        ("accept-encoding".to_string(), "gzip, deflate".to_string()),
        ("connection".to_string(), "keep-alive".to_string()),
        ("cache-control".to_string(), "no-cache".to_string()),
    ];
    group.bench_function("with_10_headers", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                black_box("/api/users".to_string()),
                headers_10.clone(),
                None,
                HashMap::new(),
            )
        });
    });

    // With body
    let body = r#"{"name": "Alice", "email": "alice@example.com", "age": 30}"#
        .as_bytes()
        .to_vec();
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

    // With path params
    let params = {
        let mut p = HashMap::new();
        p.insert("org_id".to_string(), "org-123".to_string());
        p.insert("user_id".to_string(), "user-456".to_string());
        p
    };
    group.bench_function("with_path_params", |b| {
        b.iter(|| {
            Request::new(
                Method::Get,
                black_box("/api/orgs/org-123/users/user-456".to_string()),
                vec![],
                None,
                params.clone(),
            )
        });
    });

    // Full realistic request
    let full_headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), "Bearer token".to_string()),
        ("x-request-id".to_string(), "req-123".to_string()),
    ];
    let full_body = br#"{"data": "value"}"#.to_vec();
    let full_params = {
        let mut p = HashMap::new();
        p.insert("id".to_string(), "123".to_string());
        p
    };
    group.bench_function("full_request", |b| {
        b.iter(|| {
            Request::new(
                Method::Post,
                black_box("/api/users/123?include=posts&fields=id,name".to_string()),
                full_headers.clone(),
                Some(full_body.clone()),
                full_params.clone(),
            )
        });
    });

    group.finish();
}

// =============================================================================
// Content Type Check Benchmarks
// =============================================================================

fn bench_content_type_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_type_checks");

    let req_json = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![(
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        )],
        None,
        HashMap::new(),
    );

    let req_form = Request::new(
        Method::Post,
        "/submit".to_string(),
        vec![(
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )],
        None,
        HashMap::new(),
    );

    let req_html = Request::new(
        Method::Get,
        "/page".to_string(),
        vec![("content-type".to_string(), "text/html".to_string())],
        None,
        HashMap::new(),
    );

    group.bench_function("is_json", |b| b.iter(|| req_json.is_json()));

    group.bench_function("is_form", |b| b.iter(|| req_form.is_form()));

    group.bench_function("is_html", |b| b.iter(|| req_html.is_html()));

    // Accept header checks
    let req_accept = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![(
            "accept".to_string(),
            "text/html, application/json, */*".to_string(),
        )],
        None,
        HashMap::new(),
    );

    group.bench_function("accepts_json", |b| {
        b.iter(|| req_accept.accepts(black_box("json")));
    });

    group.bench_function("accepts_html", |b| {
        b.iter(|| req_accept.accepts(black_box("html")));
    });

    group.bench_function("accepts_xml", |b| {
        b.iter(|| req_accept.accepts(black_box("xml")));
    });

    group.finish();
}

// =============================================================================
// Body Access Benchmarks
// =============================================================================

fn bench_body_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("body_access");

    // Small body
    let small_body = br#"{"id": 123}"#.to_vec();
    let req_small = Request::new(
        Method::Post,
        "/api/data".to_string(),
        vec![],
        Some(small_body),
        HashMap::new(),
    );

    group.bench_function("body_small", |b| b.iter(|| req_small.body()));

    group.bench_function("text_small", |b| b.iter(|| req_small.text()));

    group.bench_function("has_body", |b| b.iter(|| req_small.has_body()));

    // Large body (10KB)
    let large_body = "x".repeat(10 * 1024).into_bytes();
    let req_large = Request::new(
        Method::Post,
        "/api/upload".to_string(),
        vec![],
        Some(large_body),
        HashMap::new(),
    );

    group.bench_function("body_10kb", |b| b.iter(|| req_large.body()));

    group.bench_function("text_10kb", |b| b.iter(|| req_large.text()));

    // No body
    let req_none = Request::new(
        Method::Get,
        "/api/data".to_string(),
        vec![],
        None,
        HashMap::new(),
    );

    group.bench_function("body_none", |b| b.iter(|| req_none.body()));

    group.bench_function("text_none", |b| b.iter(|| req_none.text()));

    group.finish();
}

// =============================================================================
// Main
// =============================================================================

criterion_group!(
    benches,
    bench_query_parsing,
    bench_query_access,
    bench_header_lookup,
    bench_header_all,
    bench_path_params,
    bench_request_creation,
    bench_content_type_checks,
    bench_body_access,
);

criterion_main!(benches);
