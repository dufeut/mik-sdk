# VS Code Snippets for mik-sdk

This folder contains code snippets that help you write mik-sdk code faster with autocomplete suggestions.

## How to Use Snippets

1. **Start typing** a snippet prefix (e.g., `ok`, `sql_read`, `derive-type`)
2. **Press Tab or Enter** when you see the suggestion popup
3. **Fill in the placeholders** using Tab to jump between them

### Example

Type `ok` and press Tab:

```rust
// This appears:
ok!({
    "key": value
})
//   ^^^   ^^^^^
//   |     └── Tab again to edit this
//   └── Edit this first, then press Tab
```

## Quick Reference

### Most Common Snippets

| Type this... | To get... |
|--------------|-----------|
| `ok` | `ok!({ ... })` - Return 200 with JSON |
| `error` | `error! { status, title, detail }` |
| `guard` | `guard!(condition, status, "message")` |
| `ensure` | `let x = ensure!(expr, status, "message")` |
| `sql_read` | SELECT query with filter, order, limit |
| `sql_create` | INSERT query |
| `fetch` | HTTP GET request |
| `derive-type` | `#[derive(Type)]` struct |
| `derive-query` | `#[derive(Query)]` struct with pagination |
| `routes` | Full routes! macro with CRUD endpoints |

### Field Attributes

When inside a struct, type `field-` to see all options:

| Type this... | To get... |
|--------------|-----------|
| `field-default` | `#[field(default = value)]` |
| `field-min` | `#[field(min = 1)]` |
| `field-max` | `#[field(max = 100)]` |
| `field-format` | `#[field(format = "email")]` with picker |
| `field-pattern` | `#[field(pattern = r"^[a-z]+$")]` |
| `field-rename` | `#[field(rename = "jsonName")]` |
| `field-docs` | `#[field(docs = "Description")]` |

### SQL Queries

| Type this... | To get... |
|--------------|-----------|
| `sql_read` | Basic SELECT |
| `sql_read-page` | SELECT with offset pagination |
| `sql_read-cursor` | SELECT with cursor pagination |
| `sql_read-merge` | SELECT with runtime filter parsing |
| `sql_create` | INSERT with returning |
| `sql_update` | UPDATE with filter |
| `sql_delete` | DELETE with filter |
| `sql-filter` | Filter with operator picker (`$eq`, `$gt`, etc.) |
| `$or` | OR condition |
| `$and` | AND condition |

### HTTP Client

| Type this... | To get... |
|--------------|-----------|
| `fetch` | Simple GET request |
| `fetch-post` | POST with JSON body |
| `fetch-headers` | Request with custom headers |
| `fetch-timeout` | Request with timeout |
| `fetch-ssrf` | Request with SSRF protection |

### Response Helpers

| Type this... | To get... |
|--------------|-----------|
| `ok` | 200 OK with JSON |
| `created` | 201 Created with Location header |
| `no_content` | 204 No Content |
| `not_found` | 404 Not Found |
| `bad_request` | 400 Bad Request |
| `conflict` | 409 Conflict |
| `forbidden` | 403 Forbidden |
| `error` | RFC 7807 error with status picker |

### Complete File Template

Type `mik-handler` to get a complete handler file with:
- Imports
- Type definitions
- Query struct with pagination
- Routes
- Handler functions

## Tips

1. **Use Tab** to jump between placeholders
2. **Use Shift+Tab** to go back
3. **Press Escape** to exit snippet mode
4. **Type partial prefixes** - `sql_r` will match `sql_read`

## Not Seeing Snippets?

1. Make sure you're in a `.rs` file
2. Check that rust-analyzer extension is installed
3. Try pressing `Ctrl+Space` to manually trigger suggestions

## Recommended Extensions

This folder also includes `extensions.json` which recommends:
- **rust-analyzer** - Rust language support
- **Even Better TOML** - Cargo.toml support
- **crates** - Dependency version hints

VS Code will prompt you to install these when you open the project.
