//! Benchmarks for JSON parsing and building operations.
//!
//! Run with: cargo bench -p mik-sdk -- json

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use mik_sdk::json;

// =============================================================================
// Test Data Generation
// =============================================================================

/// Small JSON: Simple user object (~50 bytes)
fn small_json() -> &'static [u8] {
    br#"{"name":"Alice","age":30,"active":true}"#
}

/// Medium JSON: Nested object with array (~500 bytes)
fn medium_json() -> &'static [u8] {
    br#"{"user":{"id":"550e8400-e29b-41d4-a716-446655440000","name":"Alice Johnson","email":"alice@example.com","profile":{"bio":"Software developer","avatar":"https://example.com/avatar.jpg","location":"San Francisco"}},"posts":[{"id":1,"title":"Hello World","published":true},{"id":2,"title":"Second Post","published":false}],"meta":{"created_at":"2025-01-15T10:30:00Z","version":1}}"#
}

/// Large JSON: Array of 100 objects (~5KB)
fn large_json() -> Vec<u8> {
    let mut items = String::from("[");
    for i in 0..100 {
        if i > 0 {
            items.push(',');
        }
        items.push_str(&format!(
            r#"{{"id":{},"name":"User {}","email":"user{}@example.com","active":{}}}"#,
            i,
            i,
            i,
            i % 2 == 0
        ));
    }
    items.push(']');
    items.into_bytes()
}

/// Very large JSON: Deeply nested structure (~50KB)
fn very_large_json() -> Vec<u8> {
    let mut json = String::from(r#"{"data":{"users":["#);
    for i in 0..500 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            r#"{{"id":"{}","name":"User {} with a longer name for more realistic size","email":"user{}@example.com","metadata":{{"created":"2025-01-15","tags":["tag1","tag2","tag3"]}}}}"#,
            i, i, i
        ));
    }
    json.push_str(r#"]},"pagination":{"page":1,"limit":500,"total":10000}}"#);
    json.into_bytes()
}

// =============================================================================
// JSON Parsing Benchmarks
// =============================================================================

fn bench_json_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parse");

    // Small JSON (~50 bytes)
    let small = small_json();
    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_with_input(BenchmarkId::new("size", "small_50B"), &small, |b, data| {
        b.iter(|| json::try_parse(black_box(*data)))
    });

    // Medium JSON (~500 bytes)
    let medium = medium_json();
    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("size", "medium_500B"),
        &medium,
        |b, data| b.iter(|| json::try_parse(black_box(*data))),
    );

    // Large JSON (~5KB)
    let large = large_json();
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_with_input(BenchmarkId::new("size", "large_5KB"), &large, |b, data| {
        b.iter(|| json::try_parse(black_box(data.as_slice())))
    });

    // Very large JSON (~50KB)
    let very_large = very_large_json();
    group.throughput(Throughput::Bytes(very_large.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("size", "very_large_50KB"),
        &very_large,
        |b, data| b.iter(|| json::try_parse(black_box(data.as_slice()))),
    );

    group.finish();
}

// =============================================================================
// JSON Object Builder Benchmarks
// =============================================================================

fn bench_json_obj_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_obj_builder");

    // 3 fields (minimal API response)
    group.bench_function("3_fields", |b| {
        b.iter(|| {
            json::obj()
                .set("id", json::str(black_box("123")))
                .set("name", json::str(black_box("Alice")))
                .set("active", json::bool(black_box(true)))
        })
    });

    // 5 fields (typical entity)
    group.bench_function("5_fields", |b| {
        b.iter(|| {
            json::obj()
                .set("id", json::str(black_box("550e8400-e29b-41d4")))
                .set("name", json::str(black_box("Alice Johnson")))
                .set("email", json::str(black_box("alice@example.com")))
                .set("age", json::int(black_box(30)))
                .set("active", json::bool(black_box(true)))
        })
    });

    // 10 fields (detailed entity)
    group.bench_function("10_fields", |b| {
        b.iter(|| {
            json::obj()
                .set("id", json::str("550e8400-e29b-41d4"))
                .set("firstName", json::str("Alice"))
                .set("lastName", json::str("Johnson"))
                .set("email", json::str("alice@example.com"))
                .set("age", json::int(30))
                .set("active", json::bool(true))
                .set("role", json::str("admin"))
                .set("department", json::str("Engineering"))
                .set("salary", json::float(150000.0))
                .set("verified", json::bool(true))
        })
    });

    // 20 fields (stress test)
    group.bench_function("20_fields", |b| {
        b.iter(|| {
            json::obj()
                .set("field01", json::str("value"))
                .set("field02", json::str("value"))
                .set("field03", json::str("value"))
                .set("field04", json::str("value"))
                .set("field05", json::str("value"))
                .set("field06", json::str("value"))
                .set("field07", json::str("value"))
                .set("field08", json::str("value"))
                .set("field09", json::str("value"))
                .set("field10", json::str("value"))
                .set("field11", json::int(1))
                .set("field12", json::int(2))
                .set("field13", json::int(3))
                .set("field14", json::int(4))
                .set("field15", json::int(5))
                .set("field16", json::bool(true))
                .set("field17", json::bool(false))
                .set("field18", json::float(1.5))
                .set("field19", json::float(2.5))
                .set("field20", json::null())
        })
    });

    // Nested object (2 levels)
    group.bench_function("nested_2_levels", |b| {
        b.iter(|| {
            json::obj().set(
                "user",
                json::obj()
                    .set("id", json::str("123"))
                    .set("name", json::str("Alice"))
                    .set(
                        "profile",
                        json::obj()
                            .set("bio", json::str("Developer"))
                            .set("avatar", json::str("url")),
                    ),
            )
        })
    });

    // Nested object (4 levels - deep nesting)
    group.bench_function("nested_4_levels", |b| {
        b.iter(|| {
            json::obj().set(
                "level1",
                json::obj().set(
                    "level2",
                    json::obj().set(
                        "level3",
                        json::obj().set("level4", json::obj().set("value", json::str("deep"))),
                    ),
                ),
            )
        })
    });

    group.finish();
}

// =============================================================================
// JSON Array Builder Benchmarks
// =============================================================================

fn bench_json_arr_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_arr_builder");

    // 5 items
    group.bench_function("5_items", |b| {
        b.iter(|| {
            json::arr()
                .push(json::int(black_box(1)))
                .push(json::int(black_box(2)))
                .push(json::int(black_box(3)))
                .push(json::int(black_box(4)))
                .push(json::int(black_box(5)))
        })
    });

    // 10 items
    group.bench_function("10_items", |b| {
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
        })
    });

    // 20 items
    group.bench_function("20_items", |b| {
        b.iter(|| {
            let mut arr = json::arr();
            for i in 0..20 {
                arr = arr.push(json::int(i));
            }
            arr
        })
    });

    // 50 items (realistic list)
    group.bench_function("50_items", |b| {
        b.iter(|| {
            let mut arr = json::arr();
            for i in 0..50 {
                arr = arr.push(json::int(i));
            }
            arr
        })
    });

    // Array of strings
    group.bench_function("10_strings", |b| {
        b.iter(|| {
            json::arr()
                .push(json::str("item-001"))
                .push(json::str("item-002"))
                .push(json::str("item-003"))
                .push(json::str("item-004"))
                .push(json::str("item-005"))
                .push(json::str("item-006"))
                .push(json::str("item-007"))
                .push(json::str("item-008"))
                .push(json::str("item-009"))
                .push(json::str("item-010"))
        })
    });

    // Array of objects (common API pattern)
    group.bench_function("5_objects", |b| {
        b.iter(|| {
            json::arr()
                .push(
                    json::obj()
                        .set("id", json::int(1))
                        .set("name", json::str("Item 1")),
                )
                .push(
                    json::obj()
                        .set("id", json::int(2))
                        .set("name", json::str("Item 2")),
                )
                .push(
                    json::obj()
                        .set("id", json::int(3))
                        .set("name", json::str("Item 3")),
                )
                .push(
                    json::obj()
                        .set("id", json::int(4))
                        .set("name", json::str("Item 4")),
                )
                .push(
                    json::obj()
                        .set("id", json::int(5))
                        .set("name", json::str("Item 5")),
                )
        })
    });

    group.finish();
}

// =============================================================================
// Path Extraction Benchmarks
// =============================================================================

fn bench_json_path_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_path_extraction");

    // Prepare parsed JSON for extraction benchmarks
    let nested_json = br#"{"user":{"name":"Alice","profile":{"bio":"Developer","settings":{"theme":"dark","notifications":true}}},"items":[1,2,3,4,5],"meta":{"version":1,"timestamp":1705312200}}"#;
    let parsed = json::try_parse(nested_json).unwrap();

    // path_str: 1 level deep
    group.bench_function("path_str_1_level", |b| {
        b.iter(|| parsed.path_str(black_box(&["user"])))
    });

    // path_str: 2 levels deep
    group.bench_function("path_str_2_levels", |b| {
        b.iter(|| parsed.path_str(black_box(&["user", "name"])))
    });

    // path_str: 3 levels deep
    group.bench_function("path_str_3_levels", |b| {
        b.iter(|| parsed.path_str(black_box(&["user", "profile", "bio"])))
    });

    // path_str: 4 levels deep
    group.bench_function("path_str_4_levels", |b| {
        b.iter(|| parsed.path_str(black_box(&["user", "profile", "settings", "theme"])))
    });

    // path_int
    group.bench_function("path_int_2_levels", |b| {
        b.iter(|| parsed.path_int(black_box(&["meta", "version"])))
    });

    // path_bool
    group.bench_function("path_bool_4_levels", |b| {
        b.iter(|| parsed.path_bool(black_box(&["user", "profile", "settings", "notifications"])))
    });

    // path_float
    group.bench_function("path_float_2_levels", |b| {
        b.iter(|| parsed.path_float(black_box(&["meta", "timestamp"])))
    });

    // path_exists
    group.bench_function("path_exists_hit", |b| {
        b.iter(|| parsed.path_exists(black_box(&["user", "name"])))
    });

    group.bench_function("path_exists_miss", |b| {
        b.iter(|| parsed.path_exists(black_box(&["user", "nonexistent"])))
    });

    // path_is_null
    group.bench_function("path_is_null", |b| {
        b.iter(|| parsed.path_is_null(black_box(&["user", "name"])))
    });

    // Compare: get() chain vs path_str() for same extraction
    group.bench_function("get_chain_3_levels", |b| {
        b.iter(|| parsed.get("user").get("profile").get("bio").str())
    });

    // path_str_or (with default)
    group.bench_function("path_str_or_hit", |b| {
        b.iter(|| parsed.path_str_or(black_box(&["user", "name"]), black_box("default")))
    });

    group.bench_function("path_str_or_miss", |b| {
        b.iter(|| parsed.path_str_or(black_box(&["user", "missing"]), black_box("default")))
    });

    // path_int_or (with default)
    group.bench_function("path_int_or", |b| {
        b.iter(|| parsed.path_int_or(black_box(&["meta", "version"]), black_box(0)))
    });

    group.finish();
}

// =============================================================================
// JSON Serialization Benchmarks
// =============================================================================

fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Small object
    let small = json::obj()
        .set("id", json::str("123"))
        .set("name", json::str("Alice"))
        .set("active", json::bool(true));
    group.bench_function("to_string_small", |b| b.iter(|| small.to_string()));
    group.bench_function("to_bytes_small", |b| b.iter(|| small.to_bytes()));

    // Medium object with nesting
    let medium = json::obj()
        .set(
            "user",
            json::obj()
                .set("id", json::str("123"))
                .set("name", json::str("Alice"))
                .set("email", json::str("alice@example.com")),
        )
        .set(
            "meta",
            json::obj()
                .set("created", json::str("2025-01-15"))
                .set("version", json::int(1)),
        );
    group.bench_function("to_string_medium", |b| b.iter(|| medium.to_string()));

    // Large array
    let mut large = json::arr();
    for i in 0..100 {
        large = large.push(
            json::obj()
                .set("id", json::int(i))
                .set("value", json::str("item")),
        );
    }
    group.bench_function("to_string_large_100_items", |b| {
        b.iter(|| large.to_string())
    });

    group.finish();
}

// =============================================================================
// Main
// =============================================================================

criterion_group!(
    benches,
    bench_json_parse,
    bench_json_obj_builder,
    bench_json_arr_builder,
    bench_json_path_extraction,
    bench_json_serialization,
);

criterion_main!(benches);
