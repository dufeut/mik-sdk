//! Benchmarks for mik-sql query building operations.
//!
//! Run with: cargo bench -p mik-sql

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mik_sql::{Cursor, Filter, FilterValidator, Operator, SortDir, Value, postgres, sqlite};
use mik_sql::{is_valid_sql_expression, is_valid_sql_identifier};

// =============================================================================
// SQL Validation Benchmarks
// =============================================================================

fn bench_sql_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_validation");

    // Identifier validation
    let identifiers = [
        ("short", "id"),
        ("medium", "user_email_address"),
        ("long", "very_long_column_name_with_many_parts_here"),
        ("invalid", "DROP TABLE users--"),
    ];

    for (name, ident) in identifiers {
        group.bench_with_input(BenchmarkId::new("identifier", name), ident, |b, s| {
            b.iter(|| is_valid_sql_identifier(black_box(s)))
        });
    }

    // Expression validation
    let expressions = [
        ("simple", "price * quantity"),
        ("function", "COALESCE(name, 'unknown')"),
        ("complex", "(price * quantity) - discount + tax"),
        ("malicious", "1; DROP TABLE users--"),
    ];

    for (name, expr) in expressions {
        group.bench_with_input(BenchmarkId::new("expression", name), expr, |b, s| {
            b.iter(|| is_valid_sql_expression(black_box(s)))
        });
    }

    group.finish();
}

// =============================================================================
// Query Builder Benchmarks
// =============================================================================

fn bench_query_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_builder");

    // Simple SELECT
    group.bench_function("select_simple", |b| {
        b.iter(|| {
            postgres(black_box("users"))
                .fields(&["id", "name", "email"])
                .build()
        })
    });

    // SELECT with WHERE
    group.bench_function("select_with_where", |b| {
        b.iter(|| {
            postgres(black_box("users"))
                .fields(&["id", "name", "email"])
                .filter("active", Operator::Eq, Value::Bool(true))
                .filter("role", Operator::Eq, Value::String("admin".to_string()))
                .build()
        })
    });

    // SELECT with multiple clauses
    group.bench_function("select_full", |b| {
        b.iter(|| {
            postgres(black_box("users"))
                .fields(&["id", "name", "email", "created_at"])
                .filter("active", Operator::Eq, Value::Bool(true))
                .sort("created_at", SortDir::Desc)
                .sort("id", SortDir::Asc)
                .limit(50)
                .build()
        })
    });

    group.finish();
}

// =============================================================================
// Dialect Comparison Benchmarks
// =============================================================================

fn bench_dialects(c: &mut Criterion) {
    let mut group = c.benchmark_group("dialects");

    // Compare Postgres vs SQLite for same query
    group.bench_function("postgres", |b| {
        b.iter(|| {
            postgres(black_box("users"))
                .fields(&["id", "name"])
                .filter("active", Operator::Eq, Value::Bool(true))
                .limit(10)
                .build()
        })
    });

    group.bench_function("sqlite", |b| {
        b.iter(|| {
            sqlite(black_box("users"))
                .fields(&["id", "name"])
                .filter("active", Operator::Eq, Value::Bool(true))
                .limit(10)
                .build()
        })
    });

    group.finish();
}

// =============================================================================
// Cursor Pagination Benchmarks
// =============================================================================

fn bench_cursor_pagination(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor");

    // Cursor creation
    group.bench_function("create", |b| {
        b.iter(|| {
            Cursor::new()
                .string("created_at", black_box("2024-01-15T10:30:00Z"))
                .int("id", black_box(12345))
        })
    });

    // Cursor encoding
    let cursor = Cursor::new()
        .string("created_at", "2024-01-15T10:30:00Z")
        .int("id", 12345);

    group.bench_function("encode", |b| b.iter(|| cursor.encode()));

    // Cursor decoding
    let encoded = cursor.encode();
    group.bench_function("decode", |b| b.iter(|| Cursor::decode(black_box(&encoded))));

    // Query with cursor
    group.bench_function("query_with_cursor", |b| {
        let cursor = Cursor::new()
            .string("created_at", "2024-01-15T10:30:00Z")
            .int("id", 12345);

        b.iter(|| {
            postgres(black_box("posts"))
                .fields(&["id", "title", "created_at"])
                .filter("published", Operator::Eq, Value::Bool(true))
                .sort("created_at", SortDir::Desc)
                .sort("id", SortDir::Desc)
                .after_cursor(cursor.clone())
                .limit(20)
                .build()
        })
    });

    group.finish();
}

// =============================================================================
// Filter Validation Benchmarks
// =============================================================================

fn bench_filter_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_validation");

    // Validator creation
    group.bench_function("validator_new", |b| b.iter(FilterValidator::new));

    group.bench_function("validator_with_fields", |b| {
        b.iter(|| {
            FilterValidator::new().allow_fields(black_box(&[
                "name",
                "email",
                "status",
                "created_at",
                "updated_at",
            ]))
        })
    });

    // Simple filter validation
    let validator = FilterValidator::new().allow_fields(&["name", "email", "status", "created_at"]);

    let simple_filter = Filter {
        field: "name".to_string(),
        op: Operator::Eq,
        value: Value::String("Alice".to_string()),
    };
    group.bench_function("validate_simple", |b| {
        b.iter(|| validator.validate(black_box(&simple_filter)))
    });

    // Invalid field (should fail fast)
    let invalid_filter = Filter {
        field: "password".to_string(),
        op: Operator::Eq,
        value: Value::String("secret".to_string()),
    };
    group.bench_function("validate_invalid_field", |b| {
        b.iter(|| validator.validate(black_box(&invalid_filter)))
    });

    // Filter with IN operator (array)
    let in_filter = Filter {
        field: "status".to_string(),
        op: Operator::In,
        value: Value::Array(vec![
            Value::String("active".to_string()),
            Value::String("pending".to_string()),
            Value::String("review".to_string()),
        ]),
    };
    group.bench_function("validate_in_operator", |b| {
        b.iter(|| validator.validate(black_box(&in_filter)))
    });

    // Filter with deeply nested array values
    let nested_array_filter = Filter {
        field: "tags".to_string(),
        op: Operator::In,
        value: Value::Array(vec![
            Value::String("rust".to_string()),
            Value::String("wasm".to_string()),
            Value::String("web".to_string()),
            Value::String("api".to_string()),
            Value::String("http".to_string()),
        ]),
    };
    group.bench_function("validate_array_5_items", |b| {
        b.iter(|| validator.validate(black_box(&nested_array_filter)))
    });

    // Regex operator (denied by default)
    let regex_filter = Filter {
        field: "email".to_string(),
        op: Operator::Regex,
        value: Value::String(".*@example\\.com".to_string()),
    };
    group.bench_function("validate_denied_operator", |b| {
        b.iter(|| validator.validate(black_box(&regex_filter)))
    });

    group.finish();
}

// =============================================================================
// String Escaping Benchmarks
// =============================================================================

fn bench_string_escaping(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_escaping");

    // Build queries with strings that need escaping
    let strings = [
        ("no_quotes", "hello world"),
        ("single_quote", "it's a test"),
        ("many_quotes", "he said 'hello' and she said 'goodbye'"),
        ("long_clean", &"abcdefghij".repeat(10)),
    ];

    for (name, s) in strings {
        group.bench_with_input(BenchmarkId::new("filter_string", name), s, |b, s| {
            b.iter(|| {
                postgres("users")
                    .fields(&["id"])
                    .filter(
                        "name",
                        Operator::Eq,
                        Value::String(black_box(s.to_string())),
                    )
                    .build()
            })
        });
    }

    group.finish();
}

// =============================================================================
// Complex Query Benchmarks (Real-world patterns)
// =============================================================================

fn bench_complex_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_queries");

    // Typical list API with pagination
    group.bench_function("list_with_pagination", |b| {
        b.iter(|| {
            postgres(black_box("posts"))
                .fields(&["id", "title", "slug", "created_at", "author_id", "status"])
                .filter(
                    "status",
                    Operator::Eq,
                    Value::String("published".to_string()),
                )
                .filter("active", Operator::Eq, Value::Bool(true))
                .sort("created_at", SortDir::Desc)
                .sort("id", SortDir::Desc)
                .limit_offset(20, 40)
                .build()
        })
    });

    // Search query with multiple conditions
    group.bench_function("search_with_filters", |b| {
        b.iter(|| {
            postgres(black_box("products"))
                .fields(&["id", "name", "price", "category", "in_stock"])
                .filter(
                    "category",
                    Operator::In,
                    Value::Array(vec![
                        Value::String("electronics".to_string()),
                        Value::String("computers".to_string()),
                    ]),
                )
                .filter("price", Operator::Gte, Value::Float(100.0))
                .filter("price", Operator::Lte, Value::Float(1000.0))
                .filter("in_stock", Operator::Eq, Value::Bool(true))
                .sort("price", SortDir::Asc)
                .limit(50)
                .build()
        })
    });

    // Aggregation-style query
    group.bench_function("grouped_query", |b| {
        b.iter(|| {
            postgres(black_box("orders"))
                .fields(&["customer_id", "status"])
                .computed("total", "SUM(amount)")
                .computed("count", "COUNT(*)")
                .filter(
                    "created_at",
                    Operator::Gte,
                    Value::String("2024-01-01".to_string()),
                )
                .group_by(&["customer_id", "status"])
                .sort("total", SortDir::Desc)
                .limit(100)
                .build()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sql_validation,
    bench_query_builder,
    bench_dialects,
    bench_cursor_pagination,
    bench_filter_validation,
    bench_string_escaping,
    bench_complex_queries,
);

criterion_main!(benches);
