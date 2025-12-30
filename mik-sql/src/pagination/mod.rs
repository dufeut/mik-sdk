//! Pagination utilities for cursor and keyset pagination.
//!
//! # Pagination Strategies
//!
//! | Strategy   | Jump to Page | Performance | Stability | Use Case               |
//! |------------|--------------|-------------|-----------|------------------------|
//! | **Offset** | Yes          | O(n) skip   | Unstable* | Admin panels, reports  |
//! | **Cursor** | No           | O(1)        | Stable    | Feeds, infinite scroll |
//! | **Keyset** | No           | O(1)        | Stable    | Large datasets, APIs   |
//!
//! *Unstable = results shift if data changes between requests
//!
//! # Cursor Pagination Example
//!
//! ```
//! use mik_sql::{postgres, Cursor, PageInfo, SortDir};
//!
//! // Build cursor from last item's values
//! let cursor = Cursor::new()
//!     .string("created_at", "2024-01-15T10:00:00Z")
//!     .int("id", 42);
//!
//! // Build query with cursor pagination
//! let result = postgres("users")
//!     .fields(&["id", "name", "created_at"])
//!     .sort("created_at", SortDir::Desc)
//!     .sort("id", SortDir::Asc)
//!     .after_cursor(cursor)
//!     .limit(20)
//!     .build();
//!
//! assert!(result.sql.contains("ORDER BY created_at DESC, id ASC"));
//!
//! // Create page info for response
//! let page_info = PageInfo::new(20, 20)
//!     .with_next_cursor(Some("encoded_cursor".to_string()));
//! assert!(page_info.has_next);
//! ```
//!
//! # DX Features
//!
//! The `after_cursor` and `before_cursor` methods accept any type implementing `IntoCursor`:
//! - `&Cursor` - Already parsed cursor
//! - `&str` / `String` - Automatically decoded from base64
//! - `Option<&str>` - Perfect for `req.query("after")`
//!
//! Invalid or missing cursors are silently ignored, making the API resilient.

mod cursor;
mod encoding;
mod keyset;
mod page_info;
mod value_conv;

// Re-export all public items
pub use cursor::{Cursor, CursorError, IntoCursor};
pub use keyset::KeysetCondition;
pub use page_info::PageInfo;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::Value;

    #[test]
    fn test_base64_roundtrip() {
        let original = "{\"id\":100,\"name\":\"test\"}";
        let encoded = encoding::base64_encode(original);
        let decoded = encoding::base64_decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_cursor_from_helper() {
        #[derive(Debug)]
        struct User {
            id: i64,
        }

        let user = User { id: 42 };
        let cursor = PageInfo::cursor_from(Some(&user), |u| Cursor::new().int("id", u.id));

        assert!(cursor.is_some());
        let decoded = Cursor::decode(&cursor.unwrap()).unwrap();
        assert_eq!(decoded.fields[0], ("id".to_string(), Value::Int(42)));
    }
}
