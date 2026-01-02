# mik-sql

[![Crates.io](https://img.shields.io/crates/v/mik-sql.svg)](https://crates.io/crates/mik-sql)
[![Documentation](https://docs.rs/mik-sql/badge.svg)](https://docs.rs/mik-sql)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)

SQL query builder with Mongo-style filter operators and cursor pagination.

> **v0.1.x** - Published and usable, but evolving. The API may change between minor versions.

## Features

- **Compile-time SQL generation** - Macro-based query building
- **Mongo-style operators** - `$eq`, `$in`, `$between`, `$like`, etc.
- **Cursor pagination** - Built-in keyset pagination support
- **Dialect support** - Postgres (`$1`) and SQLite (`?1`)
- **Standalone** - Use with or without mik-sdk

## Quick Start

```rust
use mik_sql::prelude::*;

// SELECT with filters
let (sql, params) = sql_read!(users {
    select: [id, name, email],
    filter: { active: true, role: "admin" },
    order: name,
    limit: 10,
});
// → "SELECT id, name, email FROM users WHERE active = $1 AND role = $2 ORDER BY name LIMIT 10"

// INSERT with returning
let (sql, params) = sql_create!(users {
    name: str(name),
    email: str(email),
    returning: [id],
});
// → "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id"

// UPDATE with filter
let (sql, params) = sql_update!(users {
    set: { name: str(new_name) },
    filter: { id: int(user_id) },
});
// → "UPDATE users SET name = $1 WHERE id = $2"

// DELETE with filter
let (sql, params) = sql_delete!(users {
    filter: { id: int(user_id) },
});
// → "DELETE FROM users WHERE id = $1"
```

## Filter Operators

| Operator       | SQL                        | Example                                 |
| -------------- | -------------------------- | --------------------------------------- |
| `$eq`          | `=`                        | `status: { $eq: "active" }`             |
| `$ne`          | `!=`                       | `status: { $ne: "deleted" }`            |
| `$gt`          | `>`                        | `age: { $gt: 18 }`                      |
| `$gte`         | `>=`                       | `age: { $gte: 21 }`                     |
| `$lt`          | `<`                        | `price: { $lt: 100 }`                   |
| `$lte`         | `<=`                       | `price: { $lte: 50 }`                   |
| `$in`          | `IN (...)`                 | `status: { $in: ["a", "b"] }`           |
| `$nin`         | `NOT IN`                   | `status: { $nin: ["x"] }`               |
| `$like`        | `LIKE`                     | `name: { $like: "%test%" }`             |
| `$ilike`       | `ILIKE`                    | `name: { $ilike: "%TEST%" }`            |
| `$starts_with` | `LIKE x \|\| '%'`          | `name: { $starts_with: "John" }`        |
| `$ends_with`   | `LIKE '%' \|\| x`          | `email: { $ends_with: "@example.com" }` |
| `$contains`    | `LIKE '%' \|\| x \|\| '%'` | `bio: { $contains: "rust" }`            |
| `$between`     | `BETWEEN`                  | `age: { $between: [18, 65] }`           |

## Logical Operators

```rust
// AND (implicit)
filter: { active: true, role: "admin" }

// OR
filter: { $or: { status: "pending", status: "review" } }

// NOT
filter: { $not: { deleted: true } }

// Combined
filter: {
    $and: {
        active: true,
        $or: { role: "admin", role: "moderator" }
    }
}
```

## Cursor Pagination

```rust
use mik_sql::prelude::*;

// Query with cursor
let (sql, params) = sql_read!(posts {
    select: [id, title, created_at],
    filter: { published: true },
    order: [-created_at, -id],  // - prefix = DESC
    after: req.query("after"),  // cursor from query string
    limit: 20,
});

// Build cursor for next page
let next_cursor = Cursor::new()
    .string("created_at", &last_item.created_at)
    .int("id", last_item.id)
    .encode();
// → "eyJjcmVhdGVkX2F0IjoiMjAyNS0wMS0xNSIsImlkIjoxMjN9"
```

## SQLite Dialect

Add `sqlite` as first parameter:

```rust
let (sql, params) = sql_read!(sqlite, users {
    select: [id, name],
    filter: { active: true },
});
// → "SELECT id, name FROM users WHERE active = ?1"
```

## Programmatic API

```rust
use mik_sql::{postgres, Operator, Value, SortDir};

let result = postgres("users")
    .fields(&["id", "name", "email"])
    .filter("active", Operator::Eq, Value::Bool(true))
    .filter("role", Operator::In, Value::Array(vec![
        Value::String("admin".into()),
        Value::String("mod".into()),
    ]))
    .sort("created_at", SortDir::Desc)
    .limit(20)
    .build();

println!("{}", result.sql);
// → SELECT id, name, email FROM users WHERE active = $1 AND role IN ($2, $3) ORDER BY created_at DESC LIMIT 20
```

## Requirements

- Rust 1.89+ (Edition 2024)

## License

Licensed under MIT license. See [LICENSE-MIT](LICENSE-MIT).
