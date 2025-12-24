//! Tests for the sql_read!/sql_create!/sql_update!/sql_delete! macros.

use mik_sql::Value;
use mik_sql_macros::{sql_create, sql_delete, sql_read, sql_update};

#[test]
fn test_sql_basic_select() {
    let (sql, params) = sql_read!(users {
        select: [id, name, email],
    });

    assert_eq!(sql, "SELECT id, name, email FROM users");
    assert!(params.is_empty());
}

#[test]
fn test_sql_select_all() {
    let (sql, params) = sql_read!(users {});

    assert_eq!(sql, "SELECT * FROM users");
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_filter() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            active: true,
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE active = $1");
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_with_operator() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            age: { $gte: 18 },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE age >= $1");
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_with_multiple_filters() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            active: true,
            age: { $gte: 18 },
            status: "pending",
        },
    });

    assert!(sql.contains("active = $1"));
    assert!(sql.contains("age >= $2"));
    assert!(sql.contains("status = $3"));
    assert_eq!(params.len(), 3);
}

#[test]
fn test_sql_with_null() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            deleted_at: null,
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE deleted_at IS NULL");
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_in_operator() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            status: { $in: ["active", "pending"] },
        },
    });

    assert!(sql.contains("status = ANY($1)"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_with_order() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: name,
    });

    assert_eq!(sql, "SELECT id, name FROM users ORDER BY name ASC");
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_order_desc() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: -created_at,
    });

    assert_eq!(sql, "SELECT id, name FROM users ORDER BY created_at DESC");
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_multiple_orders() {
    let (sql, params) = sql_read!(users {
        select: [id, name, created_at],
        order: [name, -created_at],
    });

    assert_eq!(
        sql,
        "SELECT id, name, created_at FROM users ORDER BY name ASC, created_at DESC"
    );
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_pagination() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        page: 2,
        limit: 50,
    });

    assert!(sql.contains("LIMIT 50"));
    assert!(sql.contains("OFFSET 50"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_limit_only() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        limit: 100,
    });

    assert!(sql.contains("LIMIT 100"));
    assert!(sql.contains("OFFSET 0"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_dynamic_pagination() {
    // Dynamic pagination from variables
    let page: u32 = 3;
    let limit: u32 = 25;

    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: { active: true },
        page: page,
        limit: limit,
    });

    // page 3, limit 25 → offset = (3-1) * 25 = 50
    assert!(sql.contains("LIMIT 25"));
    assert!(sql.contains("OFFSET 50"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_with_dynamic_limit_offset() {
    // Direct limit/offset from variables
    let limit: u32 = 10;
    let offset: u32 = 100;

    #[rustfmt::skip]
    let (sql, params) = sql_read!(users {
        select: [id, name],
        limit: limit,
        offset: offset,
    });

    assert!(sql.contains("LIMIT 10"));
    assert!(sql.contains("OFFSET 100"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_with_type_hint() {
    let org_id: i64 = 123;
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            org_id: int(org_id),
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE org_id = $1");
    assert_eq!(params.len(), 1);
    match &params[0] {
        Value::Int(v) => assert_eq!(*v, 123),
        _ => panic!("Expected Int value"),
    }
}

#[test]
fn test_sql_full_query() {
    let (sql, params) = sql_read!(users {
        select: [id, name, email, created_at],
        filter: {
            active: true,
            age: { $gte: 18 },
            deleted_at: null,
        },
        order: [name, -created_at],
        page: 1,
        limit: 50,
    });

    assert!(sql.starts_with("SELECT id, name, email, created_at FROM users WHERE"));
    assert!(sql.contains("active = $1"));
    assert!(sql.contains("age >= $2"));
    assert!(sql.contains("deleted_at IS NULL"));
    assert!(sql.contains("ORDER BY name ASC, created_at DESC"));
    assert!(sql.contains("LIMIT 50 OFFSET 0"));
    assert_eq!(params.len(), 2); // active and age (null doesn't add param)
}

#[test]
fn test_sql_like_operator() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            name: { $like: "%test%" },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE name LIKE $1");
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_string_type_hint() {
    let search = "john";
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            name: str(search),
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE name = $1");
    assert_eq!(params.len(), 1);
    match &params[0] {
        Value::String(v) => assert_eq!(v, "john"),
        _ => panic!("Expected String value"),
    }
}

#[test]
fn test_sql_starts_with() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            name: { $startsWith: "A" },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE name LIKE $1 || '%'");
    assert_eq!(params.len(), 1);
    match &params[0] {
        Value::String(v) => assert_eq!(v, "A"),
        _ => panic!("Expected String value"),
    }
}

#[test]
fn test_sql_ends_with() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            email: { $endsWith: "@example.com" },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE email LIKE '%' || $1");
    assert_eq!(params.len(), 1);
    match &params[0] {
        Value::String(v) => assert_eq!(v, "@example.com"),
        _ => panic!("Expected String value"),
    }
}

#[test]
fn test_sql_contains() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            description: { $contains: "test" },
        },
    });

    assert_eq!(
        sql,
        "SELECT id, name FROM users WHERE description LIKE '%' || $1 || '%'"
    );
    assert_eq!(params.len(), 1);
    match &params[0] {
        Value::String(v) => assert_eq!(v, "test"),
        _ => panic!("Expected String value"),
    }
}

#[test]
fn test_sql_between() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            age: { $between: [18, 65] },
        },
    });

    assert_eq!(
        sql,
        "SELECT id, name FROM users WHERE age BETWEEN $1 AND $2"
    );
    assert_eq!(params.len(), 2);
    match &params[0] {
        Value::Int(v) => assert_eq!(*v, 18),
        _ => panic!("Expected Int value"),
    }
    match &params[1] {
        Value::Int(v) => assert_eq!(*v, 65),
        _ => panic!("Expected Int value"),
    }
}

#[test]
fn test_sql_starts_with_snake_case() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            name: { $starts_with: "B" },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE name LIKE $1 || '%'");
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_ends_with_snake_case() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            name: { $ends_with: "son" },
        },
    });

    assert_eq!(sql, "SELECT id, name FROM users WHERE name LIKE '%' || $1");
    assert_eq!(params.len(), 1);
}

// ═══════════════════════════════════════════════════════════════
// LOGICAL OPERATORS TESTS ($and, $or, $not)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sql_or_simple() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $or: [
                { role: "admin" },
                { role: "moderator" },
            ]
        },
    });

    assert!(sql.contains("(role = $1 OR role = $2)"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_and_explicit() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $and: [
                { active: true },
                { age: { $gte: 18 } },
            ]
        },
    });

    assert!(sql.contains("(active = $1 AND age >= $2)"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_not() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $not: [
                { status: "banned" },
            ]
        },
    });

    assert!(sql.contains("NOT (status = $1)"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_nested_and_or() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $and: [
                { active: true },
                { $or: [
                    { role: "admin" },
                    { role: "moderator" },
                ]},
            ]
        },
    });

    assert!(sql.contains("(active = $1 AND (role = $2 OR role = $3))"));
    assert_eq!(params.len(), 3);
}

#[test]
fn test_sql_or_with_operators() {
    let (sql, params) = sql_read!(users {
        select: [id, name, email],
        filter: {
            $or: [
                { age: { $lt: 18 } },
                { age: { $gte: 65 } },
            ]
        },
    });

    assert!(sql.contains("(age < $1 OR age >= $2)"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_complex_nested() {
    // (a = 1 AND b = 2) OR (c = 3 AND d = 4)
    let (sql, params) = sql_read!(data {
        select: [id],
        filter: {
            $or: [
                { $and: [
                    { a: 1 },
                    { b: 2 },
                ]},
                { $and: [
                    { c: 3 },
                    { d: 4 },
                ]},
            ]
        },
    });

    assert!(sql.contains("((a = $1 AND b = $2) OR (c = $3 AND d = $4))"));
    assert_eq!(params.len(), 4);
}

#[test]
fn test_sql_or_with_in_operator() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $or: [
                { status: { $in: ["active", "pending"] } },
                { role: "admin" },
            ]
        },
    });

    assert!(sql.contains("(status = ANY($1) OR role = $2)"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_logical_with_pagination() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: {
            $or: [
                { role: "admin" },
                { verified: true },
            ]
        },
        order: [name],
        page: 1,
        limit: 20,
    });

    assert!(sql.contains("(role = $1 OR verified = $2)"));
    assert!(sql.contains("ORDER BY name ASC"));
    assert!(sql.contains("LIMIT 20 OFFSET 0"));
    assert_eq!(params.len(), 2);
}

// ═══════════════════════════════════════════════════════════════
// AGGREGATION TESTS (COUNT, SUM, AVG, MIN, MAX, GROUP BY, HAVING)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sql_count_star() {
    let (sql, params) = sql_read!(users {
        aggregate: { count: * },
    });

    assert_eq!(sql, "SELECT COUNT(*) AS count FROM users");
    assert!(params.is_empty());
}

#[test]
fn test_sql_count_with_filter() {
    let (sql, params) = sql_read!(users {
        aggregate: { count: * },
        filter: { active: true },
    });

    assert!(sql.contains("COUNT(*) AS count"));
    assert!(sql.contains("WHERE active = $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_sum() {
    let (sql, params) = sql_read!(orders {
        aggregate: { sum: total },
    });

    assert_eq!(sql, "SELECT SUM(total) FROM orders");
    assert!(params.is_empty());
}

#[test]
fn test_sql_avg() {
    let (sql, params) = sql_read!(products {
        aggregate: { avg: price },
    });

    assert_eq!(sql, "SELECT AVG(price) FROM products");
    assert!(params.is_empty());
}

#[test]
fn test_sql_min_max() {
    let (sql, params) = sql_read!(products {
        aggregate: {
            min: price,
            max: price,
        },
    });

    assert!(sql.contains("MIN(price)"));
    assert!(sql.contains("MAX(price)"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_count_distinct() {
    let (sql, params) = sql_read!(orders {
        aggregate: { count_distinct: customer_id },
    });

    assert!(sql.contains("COUNT(DISTINCT customer_id)"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_group_by_simple() {
    let (sql, params) = sql_read!(orders {
        select: [status],
        aggregate: { count: * },
        group_by: [status],
    });

    assert!(sql.contains("SELECT status, COUNT(*) AS count"));
    assert!(sql.contains("GROUP BY status"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_group_by_multiple() {
    let (sql, params) = sql_read!(orders {
        select: [status, category],
        aggregate: { count: *, sum: total },
        group_by: [status, category],
    });

    assert!(sql.contains("SELECT status, category"));
    assert!(sql.contains("COUNT(*) AS count"));
    assert!(sql.contains("SUM(total)"));
    assert!(sql.contains("GROUP BY status, category"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_group_by_with_having() {
    let (sql, params) = sql_read!(orders {
        select: [status],
        aggregate: { count: * },
        group_by: [status],
        having: { count: { $gt: 10 } },
    });

    assert!(sql.contains("GROUP BY status"));
    assert!(sql.contains("HAVING count > $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_full_aggregation() {
    let (sql, params) = sql_read!(sales {
        select: [region, product_category],
        aggregate: { sum: amount, avg: amount, count: * },
        filter: { year: 2024 },
        group_by: [region, product_category],
        having: { count: { $gte: 10 } },
        order: [-sum],
        limit: 10,
    });

    assert!(sql.contains("SELECT region, product_category"));
    assert!(sql.contains("SUM(amount)"));
    assert!(sql.contains("AVG(amount)"));
    assert!(sql.contains("COUNT(*) AS count"));
    assert!(sql.contains("WHERE year = $1"));
    assert!(sql.contains("GROUP BY region, product_category"));
    assert!(sql.contains("HAVING count >= $2"));
    assert!(sql.contains("ORDER BY sum DESC"));
    assert!(sql.contains("LIMIT 10"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_agg_shorthand() {
    // Test the 'agg' alias for 'aggregate'
    let (sql, params) = sql_read!(users {
        agg: { count: * },
    });

    assert_eq!(sql, "SELECT COUNT(*) AS count FROM users");
    assert!(params.is_empty());
}

#[test]
fn test_sql_group_by_camel_case() {
    // Test the 'groupBy' alias for 'group_by'
    let (sql, params) = sql_read!(orders {
        select: [status],
        agg: { count: * },
        groupBy: [status],
    });

    assert!(sql.contains("GROUP BY status"));
    assert!(params.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// IDS! MACRO TESTS (Batched loading helper)
// ═══════════════════════════════════════════════════════════════

use mik_sql::ids;

#[derive(Clone)]
#[allow(dead_code)]
struct User {
    id: i64,
    name: String,
}

#[derive(Clone)]
#[allow(dead_code)]
struct Order {
    order_id: String,
    user_id: i64,
    total: f64,
}

#[test]
fn test_ids_default_field() {
    let users = [
        User {
            id: 1,
            name: "Alice".into(),
        },
        User {
            id: 2,
            name: "Bob".into(),
        },
        User {
            id: 3,
            name: "Charlie".into(),
        },
    ];

    let user_ids: Vec<i64> = ids!(users);

    assert_eq!(user_ids, [1, 2, 3]);
}

#[test]
fn test_ids_custom_field() {
    let orders = [
        Order {
            order_id: "A001".into(),
            user_id: 1,
            total: 100.0,
        },
        Order {
            order_id: "A002".into(),
            user_id: 2,
            total: 200.0,
        },
        Order {
            order_id: "A003".into(),
            user_id: 1,
            total: 150.0,
        },
    ];

    let order_ids: Vec<String> = ids!(orders, order_id);

    assert_eq!(order_ids, ["A001", "A002", "A003"]);
}

#[test]
fn test_ids_user_id_field() {
    let orders = [
        Order {
            order_id: "A001".into(),
            user_id: 1,
            total: 100.0,
        },
        Order {
            order_id: "A002".into(),
            user_id: 2,
            total: 200.0,
        },
        Order {
            order_id: "A003".into(),
            user_id: 1,
            total: 150.0,
        },
    ];

    let user_ids: Vec<i64> = ids!(orders, user_id);

    assert_eq!(user_ids, [1, 2, 1]);
}

#[test]
fn test_ids_empty_list() {
    let users: Vec<User> = vec![];

    let user_ids: Vec<i64> = ids!(users);

    assert!(user_ids.is_empty());
}

#[test]
fn test_ids_single_item() {
    let users = [User {
        id: 42,
        name: "Solo".into(),
    }];

    let user_ids: Vec<i64> = ids!(users);

    assert_eq!(user_ids, [42]);
}

// ═══════════════════════════════════════════════════════════════
// COMPUTE TESTS (computed fields in SELECT)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sql_compute_arithmetic() {
    let (sql, params) = sql_read!(order_lines {
        select: [id, quantity, price],
        compute: {
            line_total: quantity * price,
        },
    });

    assert!(sql.contains("id, quantity, price"));
    assert!(sql.contains("(quantity * price) AS line_total"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_addition() {
    let (sql, params) = sql_read!(accounts {
        select: [id],
        compute: {
            balance: credits - debits,
        },
    });

    assert!(sql.contains("(credits - debits) AS balance"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_with_parentheses() {
    let (sql, params) = sql_read!(orders {
        select: [id, quantity, price, tax_rate],
        compute: {
            total_with_tax: (quantity * price) * (1 + tax_rate),
        },
    });

    assert!(sql.contains("((quantity * price) * (1 + tax_rate)) AS total_with_tax"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_concat() {
    let (sql, params) = sql_read!(users {
        select: [id],
        compute: {
            full_name: concat(first_name, " ", last_name),
        },
    });

    assert!(sql.contains("(first_name || ' ' || last_name) AS full_name"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_coalesce() {
    let (sql, params) = sql_read!(users {
        select: [id],
        compute: {
            display_name: coalesce(nickname, first_name),
        },
    });

    assert!(sql.contains("(COALESCE(nickname, first_name)) AS display_name"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_upper_lower() {
    let (sql, params) = sql_read!(users {
        select: [id],
        compute: {
            upper_name: upper(name),
            lower_email: lower(email),
        },
    });

    assert!(sql.contains("(UPPER(name)) AS upper_name"));
    assert!(sql.contains("(LOWER(email)) AS lower_email"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_round_abs() {
    let (sql, params) = sql_read!(products {
        select: [id],
        compute: {
            rounded_price: round(price, 2),
            abs_discount: abs(discount),
        },
    });

    assert!(sql.contains("(ROUND(price, 2)) AS rounded_price"));
    assert!(sql.contains("(ABS(discount)) AS abs_discount"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_length() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        compute: {
            name_length: length(name),
        },
    });

    assert!(sql.contains("(LENGTH(name)) AS name_length"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_display_name() {
    // Common Odoo pattern: code || ' - ' || name
    let (sql, params) = sql_read!(products {
        select: [id, code, name],
        compute: {
            display_name: concat(code, " - ", name),
        },
    });

    assert!(sql.contains("(code || ' - ' || name) AS display_name"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_with_filter() {
    let (sql, params) = sql_read!(order_lines {
        select: [id],
        compute: {
            line_total: quantity * price,
        },
        filter: {
            order_id: 123,
        },
    });

    assert!(sql.contains("(quantity * price) AS line_total"));
    assert!(sql.contains("WHERE order_id = $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_compute_multiple_fields() {
    let (sql, params) = sql_read!(order_lines {
        select: [id],
        compute: {
            subtotal: quantity * unit_price,
            tax_amount: (quantity * unit_price) * tax_rate,
            total: (quantity * unit_price) * (1 + tax_rate),
        },
    });

    assert!(sql.contains("(quantity * unit_price) AS subtotal"));
    assert!(sql.contains("((quantity * unit_price) * tax_rate) AS tax_amount"));
    assert!(sql.contains("((quantity * unit_price) * (1 + tax_rate)) AS total"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_compute_division() {
    let (sql, params) = sql_read!(metrics {
        select: [id],
        compute: {
            rate: success_count / total_count,
            percentage: (success_count * 100) / total_count,
        },
    });

    assert!(sql.contains("(success_count / total_count) AS rate"));
    assert!(sql.contains("((success_count * 100) / total_count) AS percentage"));
    assert!(params.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// DYNAMIC SORT TESTS (runtime sort from user input)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sql_dynamic_sort_valid() {
    let user_sort = "name,-created_at";

    let result = sql_read!(users {
        select: [id, name, created_at],
        order: user_sort,
        allow_sort: [name, created_at, email],
    });

    assert!(result.is_ok());
    let (sql, _params) = result.unwrap();
    assert!(sql.contains("ORDER BY name ASC, created_at DESC"));
}

#[test]
fn test_sql_dynamic_sort_invalid_field() {
    let user_sort = "password"; // Not in allow_sort

    let result = sql_read!(users {
        select: [id, name],
        order: user_sort,
        allow_sort: [name, email],
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("password"));
}

#[test]
fn test_sql_dynamic_sort_with_static_filter() {
    let user_sort = "-created_at";

    let result = sql_read!(users {
        select: [id, name],
        filter: { active: true },
        order: user_sort,
        allow_sort: [name, created_at],
    });

    assert!(result.is_ok());
    let (sql, params) = result.unwrap();
    assert!(sql.contains("WHERE active = $1"));
    assert!(sql.contains("ORDER BY created_at DESC"));
    assert_eq!(params.len(), 1);
}

// ═══════════════════════════════════════════════════════════════
// MERGE FILTER TESTS (runtime filters from user input)
// ═══════════════════════════════════════════════════════════════

use mik_sql::{Filter, Operator};

#[test]
fn test_sql_merge_filters_valid() {
    let user_filters = vec![Filter {
        field: "name".to_string(),
        op: Operator::Eq,
        value: Value::String("Alice".to_string()),
    }];

    let result = sql_read!(users {
        select: [id, name],
        filter: { active: true },
        merge: user_filters,
        allow: [name, email, status],
    });

    assert!(result.is_ok());
    let (sql, params) = result.unwrap();
    assert!(sql.contains("active = $1"));
    assert!(sql.contains("name = $2"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_merge_filters_invalid_field() {
    let user_filters = vec![Filter {
        field: "password".to_string(), // Not allowed
        op: Operator::Eq,
        value: Value::String("secret".to_string()),
    }];

    let result = sql_read!(users {
        select: [id, name],
        merge: user_filters,
        allow: [name, email],
    });

    assert!(result.is_err());
}

#[test]
fn test_sql_merge_filters_denied_operator() {
    let user_filters = vec![Filter {
        field: "name".to_string(),
        op: Operator::Regex, // Denied
        value: Value::String(".*".to_string()),
    }];

    let result = sql_read!(users {
        select: [id, name],
        merge: user_filters,
        allow: [name, email],
        deny_ops: [$regex, $ilike],
    });

    assert!(result.is_err());
}

#[test]
fn test_sql_merge_filters_with_dynamic_sort() {
    let user_filters = vec![Filter {
        field: "status".to_string(),
        op: Operator::Eq,
        value: Value::String("active".to_string()),
    }];
    let user_sort = "name";

    let result = sql_read!(users {
        select: [id, name, status],
        filter: { deleted: false },
        merge: user_filters,
        allow: [name, status],
        order: user_sort,
        allow_sort: [name, created_at],
    });

    assert!(result.is_ok());
    let (sql, params) = result.unwrap();
    assert!(sql.contains("deleted = $1"));
    assert!(sql.contains("status = $2"));
    assert!(sql.contains("ORDER BY name ASC"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_merge_empty_filters() {
    let user_filters: Vec<Filter> = vec![];

    let result = sql_read!(users {
        select: [id, name],
        filter: { active: true },
        merge: user_filters,
        allow: [name, email],
    });

    assert!(result.is_ok());
    let (sql, params) = result.unwrap();
    assert!(sql.contains("active = $1"));
    assert_eq!(params.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// sql_insert! MACRO TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sql_insert_basic() {
    use mik_sql_macros::sql_create;

    let (sql, params) = sql_create!(users {
        name: "Alice",
        email: "alice@example.com",
    });

    assert_eq!(sql, "INSERT INTO users (name, email) VALUES ($1, $2)");
    assert_eq!(params.len(), 2);
    assert_eq!(params[0], Value::String("Alice".into()));
    assert_eq!(params[1], Value::String("alice@example.com".into()));
}

#[test]
fn test_sql_insert_with_type_hints() {
    use mik_sql_macros::sql_create;

    let name = "Bob";
    let age = 25;

    let (sql, params) = sql_create!(users {
        name: str(name),
        age: int(age),
        active: true,
    });

    assert_eq!(
        sql,
        "INSERT INTO users (name, age, active) VALUES ($1, $2, $3)"
    );
    assert_eq!(params.len(), 3);
    assert_eq!(params[0], Value::String("Bob".into()));
    assert_eq!(params[1], Value::Int(25));
    assert_eq!(params[2], Value::Bool(true));
}

#[test]
fn test_sql_insert_with_returning() {
    use mik_sql_macros::sql_create;

    let (sql, params) = sql_create!(users {
        name: "Carol",
        email: "carol@example.com",
        returning: [id, created_at],
    });

    assert_eq!(
        sql,
        "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, created_at"
    );
    assert_eq!(params.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// sql_update! MACRO TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sql_update_basic() {
    use mik_sql_macros::sql_update;

    let user_id = 42;

    let (sql, params) = sql_update!(users {
        set: {
            name: "Alice Updated",
            updated_at: "2025-01-01",
        },
        filter: {
            id: int(user_id),
        },
    });

    assert_eq!(
        sql,
        "UPDATE users SET name = $1, updated_at = $2 WHERE id = $3"
    );
    assert_eq!(params.len(), 3);
    assert_eq!(params[0], Value::String("Alice Updated".into()));
    assert_eq!(params[1], Value::String("2025-01-01".into()));
    assert_eq!(params[2], Value::Int(42));
}

#[test]
fn test_sql_update_with_compound_filter() {
    use mik_sql_macros::sql_update;

    let (sql, params) = sql_update!(users {
        set: {
            notified: true,
        },
        filter: {
            $or: [
                { role: "admin" },
                { role: "moderator" },
            ],
        },
    });

    assert_eq!(
        sql,
        "UPDATE users SET notified = $1 WHERE (role = $2 OR role = $3)"
    );
    assert_eq!(params.len(), 3);
    assert_eq!(params[0], Value::Bool(true));
    assert_eq!(params[1], Value::String("admin".into()));
    assert_eq!(params[2], Value::String("moderator".into()));
}

#[test]
fn test_sql_update_with_returning() {
    use mik_sql_macros::sql_update;

    let (sql, params) = sql_update!(users {
        set: {
            status: "active",
        },
        filter: {
            id: 1,
        },
        returning: [id, status, updated_at],
    });

    assert_eq!(
        sql,
        "UPDATE users SET status = $1 WHERE id = $2 RETURNING id, status, updated_at"
    );
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_update_with_operators() {
    use mik_sql_macros::sql_update;

    let (sql, params) = sql_update!(users {
        set: {
            active: false,
        },
        filter: {
            last_login: { $lt: "2024-01-01" },
            status: { $ne: "admin" },
        },
    });

    assert!(sql.contains("UPDATE users SET active = $1 WHERE"));
    assert!(sql.contains("last_login < $2"));
    assert!(sql.contains("status != $3"));
    assert_eq!(params.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// sql_delete! MACRO TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sql_delete_basic() {
    use mik_sql_macros::sql_delete;

    let user_id = 42;

    let (sql, params) = sql_delete!(users {
        filter: {
            id: int(user_id),
        },
    });

    assert_eq!(sql, "DELETE FROM users WHERE id = $1");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], Value::Int(42));
}

#[test]
fn test_sql_delete_with_compound_filter() {
    use mik_sql_macros::sql_delete;

    let (sql, params) = sql_delete!(sessions {
        filter: {
            $or: [
                { expired: true },
                { created_at: { $lt: "2024-01-01" } },
            ],
        },
    });

    assert_eq!(
        sql,
        "DELETE FROM sessions WHERE (expired = $1 OR created_at < $2)"
    );
    assert_eq!(params.len(), 2);
    assert_eq!(params[0], Value::Bool(true));
    assert_eq!(params[1], Value::String("2024-01-01".into()));
}

#[test]
fn test_sql_delete_with_returning() {
    use mik_sql_macros::sql_delete;

    let (sql, params) = sql_delete!(sessions {
        filter: {
            user_id: 123,
        },
        returning: [id, token],
    });

    assert_eq!(
        sql,
        "DELETE FROM sessions WHERE user_id = $1 RETURNING id, token"
    );
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_delete_with_multiple_conditions() {
    use mik_sql_macros::sql_delete;

    let (sql, params) = sql_delete!(logs {
        filter: {
            level: "debug",
            created_at: { $lt: "2024-06-01" },
        },
    });

    assert!(sql.contains("DELETE FROM logs WHERE"));
    assert!(sql.contains("level = $1"));
    assert!(sql.contains("created_at < $2"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_delete_with_not() {
    use mik_sql_macros::sql_delete;

    let (sql, params) = sql_delete!(users {
        filter: {
            $not: [
                { status: "active" },
            ],
        },
    });

    assert_eq!(sql, "DELETE FROM users WHERE NOT (status = $1)");
    assert_eq!(params.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// DIALECT PARAMETER TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sql_read_sqlite_dialect() {
    // SQLite uses ?1, ?2, ... parameter syntax
    let (sql, params) = sql_read!(sqlite, users {
        select: [id, name],
        filter: {
            active: true,
            age: { $gte: 18 },
        },
        limit: 10,
    });

    assert!(sql.contains("?1"));
    assert!(sql.contains("?2"));
    assert!(!sql.contains("$1")); // Should NOT have Postgres syntax
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_create_sqlite_dialect() {
    // SQLite uses ?1, ?2, ... parameter syntax
    let (sql, params) = sql_create!(
        sqlite,
        users {
            name: "Alice",
            email: "alice@example.com",
        }
    );

    assert!(sql.contains("?1"));
    assert!(sql.contains("?2"));
    assert!(!sql.contains("$1")); // Should NOT have Postgres syntax
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_update_sqlite_dialect() {
    let user_id = 42;

    let (sql, params) = sql_update!(sqlite, users {
        set: {
            name: "Bob Updated",
        },
        filter: {
            id: int(user_id),
        },
    });

    assert!(sql.contains("?1"));
    assert!(sql.contains("?2"));
    assert!(!sql.contains("$1")); // Should NOT have Postgres syntax
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_delete_sqlite_dialect() {
    let user_id = 42;

    let (sql, params) = sql_delete!(sqlite, users {
        filter: {
            id: int(user_id),
        },
    });

    assert!(sql.contains("?1"));
    assert!(!sql.contains("$1")); // Should NOT have Postgres syntax
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_read_postgres_is_default() {
    // Without dialect, should default to Postgres ($1, $2, ...)
    let (sql, _params) = sql_read!(users {
        filter: { active: true },
    });

    assert!(sql.contains("$1"));
    assert!(!sql.contains("?1")); // Should NOT have SQLite syntax
}

#[test]
fn test_sql_read_explicit_postgres() {
    // Explicit postgres dialect
    let (sql, _params) = sql_read!(postgres, users {
        filter: { active: true },
    });

    assert!(sql.contains("$1"));
    assert!(!sql.contains("?1"));
}

#[test]
fn test_sql_read_pg_alias() {
    // "pg" as alias for postgres
    let (sql, _params) = sql_read!(pg, users {
        filter: { active: true },
    });

    assert!(sql.contains("$1"));
    assert!(!sql.contains("?1"));
}

// ═══════════════════════════════════════════════════════════════
// CURSOR PAGINATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sql_read_with_after_cursor() {
    use mik_sql::Cursor;

    let cursor = Cursor::new().int("id", 100);

    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: id,
        after: cursor.clone(),
        limit: 20,
    });

    assert!(sql.contains("id > $1"));
    assert!(sql.contains("ORDER BY id ASC"));
    assert!(sql.contains("LIMIT 20"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_read_with_before_cursor() {
    use mik_sql::Cursor;

    let cursor = Cursor::new().int("id", 100);

    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: id,
        before: cursor.clone(),
        limit: 20,
    });

    assert!(sql.contains("id < $1"));
    assert!(sql.contains("ORDER BY id ASC"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_read_cursor_with_desc_sort() {
    use mik_sql::Cursor;

    let cursor = Cursor::new().int("id", 100);

    let (sql, params) = sql_read!(posts {
        select: [id, title],
        order: -id,
        after: cursor.clone(),
        limit: 10,
    });

    // DESC + after = < operator
    assert!(sql.contains("id < $1"));
    assert!(sql.contains("ORDER BY id DESC"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_read_cursor_with_filter() {
    use mik_sql::Cursor;

    let cursor = Cursor::new().int("id", 50);

    let (sql, params) = sql_read!(users {
        select: [id, name],
        filter: { active: true },
        order: id,
        after: cursor.clone(),
        limit: 20,
    });

    assert!(sql.contains("active = $1"));
    assert!(sql.contains("id > $2"));
    assert!(sql.contains("ORDER BY id ASC"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_read_cursor_multi_field() {
    use mik_sql::Cursor;

    let cursor = Cursor::new()
        .string("created_at", "2025-01-01T00:00:00Z")
        .int("id", 100);

    let (sql, params) = sql_read!(posts {
        select: [id, title, created_at],
        order: [-created_at, -id],
        after: cursor.clone(),
        limit: 20,
    });

    // Multi-field uses tuple comparison
    assert!(sql.contains("(created_at, id) <"));
    assert!(sql.contains("ORDER BY created_at DESC, id DESC"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_sql_read_cursor_none_ignored() {
    let cursor: Option<&str> = None;

    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: id,
        after: cursor,
        limit: 20,
    });

    // No cursor condition should be added
    assert!(!sql.contains("id >"));
    assert!(!sql.contains("id <"));
    assert!(sql.contains("ORDER BY id ASC"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_read_cursor_from_string() {
    use mik_sql::Cursor;

    // Create and encode a cursor
    let cursor = Cursor::new().int("id", 42);
    let encoded = cursor.encode();

    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: id,
        after: encoded.as_str(),
        limit: 20,
    });

    assert!(sql.contains("id > $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_sql_read_cursor_invalid_string_ignored() {
    let (sql, params) = sql_read!(users {
        select: [id, name],
        order: id,
        after: "invalid-cursor!!!",
        limit: 20,
    });

    // Invalid cursor should be silently ignored
    assert!(!sql.contains("id >"));
    assert!(params.is_empty());
}

#[test]
fn test_sql_read_cursor_sqlite() {
    use mik_sql::Cursor;

    let cursor = Cursor::new().int("id", 100);

    let (sql, params) = sql_read!(
        sqlite,
        users {
            select: [id, name],
            order: id,
            after: cursor.clone(),
            limit: 20,
        }
    );

    assert!(sql.contains("id > ?1"));
    assert!(sql.contains("ORDER BY id ASC"));
    assert_eq!(params.len(), 1);
}
