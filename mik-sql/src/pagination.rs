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
//! ```ignore
//! use mik_sql::{postgres, Cursor, PageInfo, SortDir};
//!
//! // Build query with cursor pagination from request query params
//! // after_cursor accepts: &Cursor, &str, String, Option<&str>, etc.
//! let result = postgres("users")
//!     .fields(&["id", "name", "created_at"])
//!     .sort("created_at", SortDir::Desc)
//!     .sort("id", SortDir::Asc)
//!     .after_cursor(req.query("after"))  // Silently ignored if None or invalid
//!     .limit(20)
//!     .build();
//!
//! // Execute query and create response with page info
//! let items = db.query(&result.sql, &result.params);
//! let page_info = PageInfo::new(items.len(), 20)
//!     .with_next_cursor(PageInfo::cursor_from(items.last(), |u| {
//!         Cursor::new()
//!             .string("created_at", &u.created_at)
//!             .int("id", u.id)
//!     }));
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

use crate::builder::{
    CompoundFilter, Filter, FilterExpr, LogicalOp, Operator, SortDir, SortField, Value,
};

/// Maximum allowed cursor size in bytes (4KB).
/// This prevents DoS attacks via oversized cursor payloads.
const MAX_CURSOR_SIZE: usize = 4 * 1024;

/// Maximum number of fields allowed in a cursor.
/// This prevents DoS attacks via cursors with many tiny fields
/// (e.g., `{"a":1,"b":2,...}` with hundreds of fields).
const MAX_CURSOR_FIELDS: usize = 16;

/// A cursor for cursor-based pagination.
///
/// Cursors encode the position in a result set as a base64 JSON object.
/// The cursor contains the values of the sort fields for the last item.
///
/// # Security Note
///
/// Cursors use simple base64 encoding, **not encryption**. The cursor content
/// is easily decoded by clients. This is intentional - cursors are opaque
/// pagination tokens, not security mechanisms.
///
/// **Do not include sensitive data in cursor fields.** Only include the
/// values needed for pagination (e.g., `id`, `created_at`).
///
/// If you need to prevent cursor tampering, validate cursor values against
/// expected ranges or sign cursors server-side.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    /// Field values that define the cursor position.
    pub fields: Vec<(String, Value)>,
}

impl Cursor {
    /// Create a new empty cursor.
    #[must_use]
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a field value to the cursor.
    pub fn field(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// Add an integer field.
    pub fn int(self, name: impl Into<String>, value: i64) -> Self {
        self.field(name, Value::Int(value))
    }

    /// Add a string field.
    pub fn string(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.field(name, Value::String(value.into()))
    }

    /// Encode the cursor to a base64 string.
    ///
    /// Note: This uses simple base64, not encryption. See [`Cursor`] security note.
    #[must_use]
    pub fn encode(&self) -> String {
        let json = self.to_json();
        base64_encode(&json)
    }

    /// Decode a cursor from a base64 string.
    ///
    /// Returns an error if the cursor exceeds `MAX_CURSOR_SIZE` (4KB).
    pub fn decode(encoded: &str) -> Result<Self, CursorError> {
        // Check size before decoding to prevent DoS attacks
        if encoded.len() > MAX_CURSOR_SIZE {
            return Err(CursorError::TooLarge);
        }
        let json = base64_decode(encoded).map_err(|()| CursorError::InvalidBase64)?;
        Self::from_json(&json)
    }

    /// Convert cursor to JSON string.
    fn to_json(&self) -> String {
        let mut parts = Vec::new();
        for (name, value) in &self.fields {
            let val_str = match value {
                Value::Null => "null".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::String(s) => format!("\"{}\"", escape_json(s)),
                Value::Array(_) => continue, // Skip arrays in cursors
            };
            parts.push(format!("\"{name}\":{val_str}"));
        }
        format!("{{{}}}", parts.join(","))
    }

    /// Parse cursor from JSON string.
    fn from_json(json: &str) -> Result<Self, CursorError> {
        let mut cursor = Cursor::new();
        let json = json.trim();

        if !json.starts_with('{') || !json.ends_with('}') {
            return Err(CursorError::InvalidFormat);
        }

        let inner = &json[1..json.len() - 1];
        if inner.is_empty() {
            return Ok(cursor);
        }

        // Simple JSON parser for cursor format
        for pair in split_json_pairs(inner) {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }

            let colon_idx = pair.find(':').ok_or(CursorError::InvalidFormat)?;
            let key = pair[..colon_idx].trim();
            let value = pair[colon_idx + 1..].trim();

            // Parse key (remove quotes)
            if !key.starts_with('"') || !key.ends_with('"') {
                return Err(CursorError::InvalidFormat);
            }
            let key = &key[1..key.len() - 1];

            // Parse value
            let parsed_value = if value == "null" {
                Value::Null
            } else if value == "true" {
                Value::Bool(true)
            } else if value == "false" {
                Value::Bool(false)
            } else if value.starts_with('"') && value.ends_with('"') {
                Value::String(unescape_json(&value[1..value.len() - 1]))
            } else if value.contains('.') {
                value
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| CursorError::InvalidFormat)?
            } else {
                value
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| CursorError::InvalidFormat)?
            };

            cursor.fields.push((key.to_string(), parsed_value));

            // Limit field count to prevent DoS via many tiny fields
            if cursor.fields.len() > MAX_CURSOR_FIELDS {
                return Err(CursorError::TooManyFields);
            }
        }

        Ok(cursor)
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when parsing a cursor.
#[derive(Debug, Clone, PartialEq)]
pub enum CursorError {
    /// The base64 encoding is invalid.
    InvalidBase64,
    /// The cursor format is invalid.
    InvalidFormat,
    /// The cursor exceeds the maximum allowed size.
    TooLarge,
    /// The cursor has too many fields.
    TooManyFields,
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBase64 => write!(f, "Invalid base64 encoding"),
            Self::InvalidFormat => write!(f, "Invalid cursor format"),
            Self::TooLarge => write!(f, "Cursor exceeds maximum size"),
            Self::TooManyFields => write!(f, "Cursor has too many fields"),
        }
    }
}

impl std::error::Error for CursorError {}

/// Trait for types that can be converted into a cursor.
///
/// This provides flexible DX for cursor pagination methods:
/// - `Cursor` - zero-cost move when you own the cursor
/// - `&str` - automatically decodes, returns None if invalid
/// - `Option<&str>` - perfect for `req.query("after")` results
///
/// # Example
///
/// ```ignore
/// // All of these work:
/// .after_cursor(cursor)            // Cursor (zero-cost move)
/// .after_cursor(cursor.clone())    // explicit clone if you need to keep it
/// .after_cursor("eyJpZCI6MTAwfQ") // &str (base64 encoded)
/// .after_cursor(req.query("after")) // Option<&str>
/// ```
pub trait IntoCursor {
    /// Convert into an optional cursor.
    /// Returns None if the input is invalid or missing.
    fn into_cursor(self) -> Option<Cursor>;
}

impl IntoCursor for Cursor {
    fn into_cursor(self) -> Option<Cursor> {
        // Empty cursor should not add any conditions
        if self.fields.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

impl IntoCursor for &str {
    fn into_cursor(self) -> Option<Cursor> {
        if self.is_empty() || self.len() > MAX_CURSOR_SIZE {
            return None;
        }
        Cursor::decode(self).ok()
    }
}

impl IntoCursor for String {
    fn into_cursor(self) -> Option<Cursor> {
        self.as_str().into_cursor()
    }
}

impl IntoCursor for &String {
    fn into_cursor(self) -> Option<Cursor> {
        self.as_str().into_cursor()
    }
}

impl<T: IntoCursor> IntoCursor for Option<T> {
    fn into_cursor(self) -> Option<Cursor> {
        self.and_then(IntoCursor::into_cursor)
    }
}

/// Page information for paginated responses.
///
/// # Example
///
/// ```ignore
/// let page_info = PageInfo::new(items.len(), limit)
///     .with_next_cursor(next_cursor)
///     .with_prev_cursor(prev_cursor);
///
/// ok!({
///     "data": items,
///     "page_info": {
///         "has_next": page_info.has_next,
///         "has_prev": page_info.has_prev,
///         "next_cursor": page_info.next_cursor,
///         "prev_cursor": page_info.prev_cursor
///     }
/// })
/// ```
#[derive(Debug, Clone, Default)]
pub struct PageInfo {
    /// Whether there are more items after this page.
    pub has_next: bool,
    /// Whether there are items before this page.
    pub has_prev: bool,
    /// Cursor to fetch the next page.
    pub next_cursor: Option<String>,
    /// Cursor to fetch the previous page.
    pub prev_cursor: Option<String>,
    /// Total count (if available).
    pub total: Option<u64>,
}

impl PageInfo {
    /// Create page info based on returned count vs requested limit.
    ///
    /// If `count >= limit`, assumes there are more items.
    #[must_use]
    pub fn new(count: usize, limit: usize) -> Self {
        Self {
            has_next: count >= limit,
            has_prev: false,
            next_cursor: None,
            prev_cursor: None,
            total: None,
        }
    }

    /// Set whether there are previous items.
    #[must_use]
    pub fn with_has_prev(mut self, has_prev: bool) -> Self {
        self.has_prev = has_prev;
        self
    }

    /// Set the next cursor.
    #[must_use]
    pub fn with_next_cursor(mut self, cursor: Option<String>) -> Self {
        self.next_cursor = cursor;
        if self.next_cursor.is_some() {
            self.has_next = true;
        }
        self
    }

    /// Set the previous cursor.
    #[must_use]
    pub fn with_prev_cursor(mut self, cursor: Option<String>) -> Self {
        self.prev_cursor = cursor;
        if self.prev_cursor.is_some() {
            self.has_prev = true;
        }
        self
    }

    /// Set the total count.
    #[must_use]
    pub fn with_total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }

    /// Create cursor from the last item using a builder function.
    pub fn cursor_from<T, F>(item: Option<&T>, builder: F) -> Option<String>
    where
        F: FnOnce(&T) -> Cursor,
    {
        item.map(|item| builder(item).encode())
    }
}

/// Keyset pagination condition.
///
/// Generates efficient `(col1, col2) > ($1, $2)` style WHERE clauses
/// for keyset/seek pagination.
#[derive(Debug, Clone)]
pub struct KeysetCondition {
    /// The sort fields and their directions.
    pub sort_fields: Vec<SortField>,
    /// The cursor values for each field.
    pub cursor_values: Vec<Value>,
    /// Direction: true for "after", false for "before".
    pub forward: bool,
}

impl KeysetCondition {
    /// Create a new keyset condition for paginating after a cursor.
    #[must_use]
    pub fn after(sorts: &[SortField], cursor: &Cursor) -> Option<Self> {
        Self::new(sorts, cursor, true)
    }

    /// Create a new keyset condition for paginating before a cursor.
    #[must_use]
    pub fn before(sorts: &[SortField], cursor: &Cursor) -> Option<Self> {
        Self::new(sorts, cursor, false)
    }

    fn new(sorts: &[SortField], cursor: &Cursor, forward: bool) -> Option<Self> {
        if sorts.is_empty() {
            return None;
        }

        // Match cursor fields to sort fields
        let mut cursor_values = Vec::new();
        for sort in sorts {
            let value = cursor
                .fields
                .iter()
                .find(|(name, _)| name == &sort.field)
                .map(|(_, v)| v.clone())?;
            cursor_values.push(value);
        }

        Some(Self {
            sort_fields: sorts.to_vec(),
            cursor_values,
            forward,
        })
    }

    /// Convert to a filter expression for the query builder.
    ///
    /// For a single field, generates: `field > $1` (or `<` for DESC)
    ///
    /// For multiple fields, generates proper compound OR conditions:
    /// `(a, b) > (1, 2)` becomes: `(a > 1) OR (a = 1 AND b > 2)`
    ///
    /// For 3+ fields: `(a > 1) OR (a = 1 AND b > 2) OR (a = 1 AND b = 2 AND c > 3)`
    ///
    /// This follows the keyset pagination standard used by PostgreSQL, GraphQL Relay,
    /// and major ORMs. See: <https://use-the-index-luke.com/no-offset>
    #[must_use]
    pub fn to_filter_expr(&self) -> FilterExpr {
        if self.sort_fields.is_empty() || self.cursor_values.is_empty() {
            // Return a tautology (always true) - will be optimized away
            return FilterExpr::Simple(Filter {
                field: "1".to_string(),
                op: Operator::Eq,
                value: Value::Int(1),
            });
        }

        if self.sort_fields.len() == 1 {
            // Simple case: single field comparison
            let sort = &self.sort_fields[0];
            let value = &self.cursor_values[0];
            let op = self.get_operator(sort.dir);

            return FilterExpr::Simple(Filter {
                field: sort.field.clone(),
                op,
                value: value.clone(),
            });
        }

        // Multi-field keyset: generate OR conditions
        // (a, b, c) > (1, 2, 3) expands to:
        //   (a > 1)
        //   OR (a = 1 AND b > 2)
        //   OR (a = 1 AND b = 2 AND c > 3)
        let mut or_conditions: Vec<FilterExpr> = Vec::new();

        for i in 0..self.sort_fields.len() {
            // Build: equality on fields 0..i, then comparison on field i
            let mut and_conditions: Vec<FilterExpr> = Vec::new();

            // Add equality conditions for all preceding fields
            for j in 0..i {
                and_conditions.push(FilterExpr::Simple(Filter {
                    field: self.sort_fields[j].field.clone(),
                    op: Operator::Eq,
                    value: self.cursor_values[j].clone(),
                }));
            }

            // Add comparison condition for current field
            let sort = &self.sort_fields[i];
            let value = &self.cursor_values[i];
            let op = self.get_operator(sort.dir);
            and_conditions.push(FilterExpr::Simple(Filter {
                field: sort.field.clone(),
                op,
                value: value.clone(),
            }));

            // Combine with AND
            let condition = if and_conditions.len() == 1 {
                and_conditions.into_iter().next().unwrap()
            } else {
                FilterExpr::Compound(CompoundFilter {
                    op: LogicalOp::And,
                    filters: and_conditions,
                })
            };

            or_conditions.push(condition);
        }

        // Combine all with OR
        if or_conditions.len() == 1 {
            or_conditions.into_iter().next().unwrap()
        } else {
            FilterExpr::Compound(CompoundFilter {
                op: LogicalOp::Or,
                filters: or_conditions,
            })
        }
    }

    fn get_operator(&self, dir: SortDir) -> Operator {
        match (self.forward, dir) {
            (true, SortDir::Asc) => Operator::Gt,
            (true, SortDir::Desc) => Operator::Lt,
            (false, SortDir::Asc) => Operator::Lt,
            (false, SortDir::Desc) => Operator::Gt,
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Simple base64 encoding (URL-safe, no padding).
///
/// # Why Custom Implementation?
///
/// This crate avoids external dependencies for base64 encoding/decoding to:
/// 1. Minimize binary size in WASM targets
/// 2. Avoid dependency version conflicts
/// 3. Keep the implementation simple and auditable
///
/// The implementation uses URL-safe alphabet (`-_` instead of `+/`) and omits
/// padding, making cursors safe for use in query strings without additional encoding.
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));

        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(n & 0x3F) as usize] as char);
        }
    }

    result
}

/// Simple base64 decoding (URL-safe, no padding).
///
/// Accepts both URL-safe (`-_`) and standard (`+/`) alphabet for compatibility.
/// See [`base64_encode`] for rationale on custom implementation.
fn base64_decode(input: &str) -> Result<String, ()> {
    const DECODE: [i8; 128] = {
        let mut table = [-1i8; 128];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i as i8;
            table[(b'a' + i) as usize] = (i + 26) as i8;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            table[(b'0' + i) as usize] = (i + 52) as i8;
            i += 1;
        }
        table[b'-' as usize] = 62;
        table[b'_' as usize] = 63;
        // Also support standard base64
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let bytes: Vec<u8> = input.bytes().collect();
    let mut result = Vec::new();

    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        let mut valid_chars = 0;

        for (i, &b) in chunk.iter().enumerate() {
            if b as usize >= 128 {
                return Err(());
            }
            let val = DECODE[b as usize];
            if val < 0 {
                return Err(());
            }
            n |= (val as u32) << (18 - i * 6);
            valid_chars += 1;
        }

        result.push((n >> 16) as u8);
        if valid_chars > 2 {
            result.push((n >> 8) as u8);
        }
        if valid_chars > 3 {
            result.push(n as u8);
        }
    }

    String::from_utf8(result).map_err(|_| ())
}

/// Escape a string for JSON per RFC 8259.
///
/// Escapes:
/// - `"` → `\"`
/// - `\` → `\\`
/// - Control characters (U+0000 to U+001F) → `\uXXXX` or named escapes
fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"), // backspace
            '\x0C' => result.push_str("\\f"), // form feed
            // Other control characters (U+0000 to U+001F)
            c if c.is_control() && (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            },
            c => result.push(c),
        }
    }
    result
}

/// Unescape a JSON string per RFC 8259.
fn unescape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('u') => {
                    // Parse \uXXXX escape
                    let mut hex = String::with_capacity(4);
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            hex.push(h);
                        }
                    }
                    if let Ok(code) = u32::from_str_radix(&hex, 16)
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                },
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Split JSON object into key:value pairs, respecting nesting.
fn split_json_pairs(s: &str) -> Vec<&str> {
    let mut pairs = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, c) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }

        match c {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                pairs.push(&s[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }

    if start < s.len() {
        pairs.push(&s[start..]);
    }

    pairs
}

// ============================================================================
// Value conversion helpers
// ============================================================================

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(i64::from(v))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_encode_decode() {
        let cursor = Cursor::new().int("id", 100).string("name", "Alice");

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(cursor.fields, decoded.fields);
    }

    #[test]
    fn test_cursor_empty() {
        let cursor = Cursor::new();
        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();
        assert!(decoded.fields.is_empty());
    }

    #[test]
    fn test_cursor_with_special_chars() {
        let cursor = Cursor::new().string("name", "Hello \"World\"");

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(cursor.fields, decoded.fields);
    }

    #[test]
    fn test_cursor_with_float() {
        let cursor = Cursor::new().field("score", 1.234f64);

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(decoded.fields.len(), 1);
        match &decoded.fields[0].1 {
            Value::Float(f) => assert!((f - 1.234).abs() < 0.001),
            _ => panic!("Expected float"),
        }
    }

    #[test]
    fn test_cursor_invalid_base64() {
        let result = Cursor::decode("not valid base64!!!");
        assert!(matches!(result, Err(CursorError::InvalidBase64)));
    }

    #[test]
    fn test_cursor_too_large() {
        // Create a cursor string larger than MAX_CURSOR_SIZE (4KB)
        let oversized = "a".repeat(5 * 1024);
        let result = Cursor::decode(&oversized);
        assert!(matches!(result, Err(CursorError::TooLarge)));

        // IntoCursor should return None for oversized cursors
        let cursor: Option<Cursor> = oversized.as_str().into_cursor();
        assert!(cursor.is_none());
    }

    #[test]
    fn test_cursor_too_many_fields() {
        // Create JSON with more than MAX_CURSOR_FIELDS (16) fields
        let mut fields = Vec::new();
        for i in 0..20 {
            fields.push(format!("\"f{}\":1", i));
        }
        let json = format!("{{{}}}", fields.join(","));
        let encoded = base64_encode(&json);

        let result = Cursor::decode(&encoded);
        assert!(matches!(result, Err(CursorError::TooManyFields)));

        // IntoCursor should return None for cursors with too many fields
        let cursor: Option<Cursor> = encoded.as_str().into_cursor();
        assert!(cursor.is_none());
    }

    #[test]
    fn test_page_info_basic() {
        let info = PageInfo::new(20, 20);
        assert!(info.has_next);
        assert!(!info.has_prev);

        let info = PageInfo::new(15, 20);
        assert!(!info.has_next);
    }

    #[test]
    fn test_page_info_with_cursors() {
        let info = PageInfo::new(20, 20)
            .with_next_cursor(Some("abc".to_string()))
            .with_prev_cursor(Some("xyz".to_string()))
            .with_total(100);

        assert!(info.has_next);
        assert!(info.has_prev);
        assert_eq!(info.next_cursor, Some("abc".to_string()));
        assert_eq!(info.prev_cursor, Some("xyz".to_string()));
        assert_eq!(info.total, Some(100));
    }

    #[test]
    fn test_keyset_condition_asc() {
        let sorts = vec![SortField::new("id", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.field, "id");
                assert_eq!(f.op, Operator::Gt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_desc() {
        let sorts = vec![SortField::new("created_at", SortDir::Desc)];
        let cursor = Cursor::new().string("created_at", "2024-01-01");

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.op, Operator::Lt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_before() {
        let sorts = vec![SortField::new("id", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::before(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Simple(f) => {
                assert_eq!(f.op, Operator::Lt);
            },
            _ => panic!("Expected simple filter"),
        }
    }

    #[test]
    fn test_keyset_condition_multi_field_asc_asc() {
        // Test: (created_at, id) > ('2024-01-01', 100)
        // Should generate: (created_at > '2024-01-01') OR (created_at = '2024-01-01' AND id > 100)
        let sorts = vec![
            SortField::new("created_at", SortDir::Asc),
            SortField::new("id", SortDir::Asc),
        ];
        let cursor = Cursor::new()
            .string("created_at", "2024-01-01")
            .int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        // Should be OR compound
        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);
                assert_eq!(compound.filters.len(), 2);

                // First: created_at > '2024-01-01'
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "created_at");
                        assert_eq!(f.op, Operator::Gt);
                    },
                    _ => panic!("Expected simple filter for first condition"),
                }

                // Second: (created_at = '2024-01-01' AND id > 100)
                match &compound.filters[1] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.op, LogicalOp::And);
                        assert_eq!(and_compound.filters.len(), 2);
                    },
                    _ => panic!("Expected compound AND filter for second condition"),
                }
            },
            _ => panic!("Expected compound OR filter for multi-field keyset"),
        }
    }

    #[test]
    fn test_keyset_condition_multi_field_desc_asc() {
        // Test: ORDER BY created_at DESC, id ASC with cursor after
        let sorts = vec![
            SortField::new("created_at", SortDir::Desc),
            SortField::new("id", SortDir::Asc),
        ];
        let cursor = Cursor::new()
            .string("created_at", "2024-01-01")
            .int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);

                // First condition: created_at < '2024-01-01' (DESC means <)
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "created_at");
                        assert_eq!(f.op, Operator::Lt); // DESC + After = Lt
                    },
                    _ => panic!("Expected simple filter"),
                }
            },
            _ => panic!("Expected compound filter"),
        }
    }

    #[test]
    fn test_keyset_condition_three_fields() {
        // Test: (a, b, c) > (1, 2, 3) expands to:
        //   (a > 1)
        //   OR (a = 1 AND b > 2)
        //   OR (a = 1 AND b = 2 AND c > 3)
        let sorts = vec![
            SortField::new("a", SortDir::Asc),
            SortField::new("b", SortDir::Asc),
            SortField::new("c", SortDir::Asc),
        ];
        let cursor = Cursor::new().int("a", 1).int("b", 2).int("c", 3);

        let condition = KeysetCondition::after(&sorts, &cursor).unwrap();
        let expr = condition.to_filter_expr();

        match expr {
            FilterExpr::Compound(compound) => {
                assert_eq!(compound.op, LogicalOp::Or);
                assert_eq!(compound.filters.len(), 3);

                // First: a > 1 (simple)
                match &compound.filters[0] {
                    FilterExpr::Simple(f) => {
                        assert_eq!(f.field, "a");
                        assert_eq!(f.op, Operator::Gt);
                    },
                    _ => panic!("Expected simple filter"),
                }

                // Second: a = 1 AND b > 2
                match &compound.filters[1] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.filters.len(), 2);
                    },
                    _ => panic!("Expected compound filter"),
                }

                // Third: a = 1 AND b = 2 AND c > 3
                match &compound.filters[2] {
                    FilterExpr::Compound(and_compound) => {
                        assert_eq!(and_compound.filters.len(), 3);
                    },
                    _ => panic!("Expected compound filter"),
                }
            },
            _ => panic!("Expected compound filter"),
        }
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "{\"id\":100,\"name\":\"test\"}";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
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

    #[test]
    fn test_value_from_conversions() {
        let _: Value = 42i64.into();
        let _: Value = 42i32.into();
        let _: Value = 1.234f64.into();
        let _: Value = "hello".into();
        let _: Value = String::from("world").into();
        let _: Value = true.into();
    }

    // =========================================================================
    // CURSOR NEAR-LIMIT SCENARIO TESTS
    // =========================================================================

    #[test]
    fn test_cursor_exactly_at_max_fields() {
        // Create JSON with exactly MAX_CURSOR_FIELDS (16) fields - should succeed
        let mut fields = Vec::new();
        for i in 0..16 {
            fields.push(format!("\"f{}\":1", i));
        }
        let json = format!("{{{}}}", fields.join(","));
        let encoded = base64_encode(&json);

        let result = Cursor::decode(&encoded);
        assert!(
            result.is_ok(),
            "Cursor with exactly 16 fields should succeed"
        );
        assert_eq!(result.unwrap().fields.len(), 16);
    }

    #[test]
    fn test_cursor_one_under_max_fields() {
        // Create JSON with MAX_CURSOR_FIELDS - 1 (15) fields - should succeed
        let mut fields = Vec::new();
        for i in 0..15 {
            fields.push(format!("\"f{}\":1", i));
        }
        let json = format!("{{{}}}", fields.join(","));
        let encoded = base64_encode(&json);

        let result = Cursor::decode(&encoded);
        assert!(result.is_ok(), "Cursor with 15 fields should succeed");
        assert_eq!(result.unwrap().fields.len(), 15);
    }

    #[test]
    fn test_cursor_one_over_max_fields() {
        // Create JSON with MAX_CURSOR_FIELDS + 1 (17) fields - should fail
        let mut fields = Vec::new();
        for i in 0..17 {
            fields.push(format!("\"f{}\":1", i));
        }
        let json = format!("{{{}}}", fields.join(","));
        let encoded = base64_encode(&json);

        let result = Cursor::decode(&encoded);
        assert!(matches!(result, Err(CursorError::TooManyFields)));
    }

    #[test]
    fn test_cursor_near_max_size() {
        // Create a cursor near MAX_CURSOR_SIZE (4KB) but under
        // Each field "fXXX":1 is about 9 chars, we need ~450 fields for 4KB
        // But we're limited to 16 fields, so use long string values instead
        let long_value = "x".repeat(200);
        let cursor = Cursor::new()
            .string("f1", &long_value)
            .string("f2", &long_value)
            .string("f3", &long_value)
            .string("f4", &long_value);

        let encoded = cursor.encode();
        assert!(encoded.len() < 4096, "Cursor should be under 4KB limit");

        // Should decode successfully
        let decoded = Cursor::decode(&encoded);
        assert!(decoded.is_ok());
    }

    #[test]
    fn test_cursor_exactly_at_max_size_boundary() {
        // The check is `> MAX_CURSOR_SIZE`, so exactly 4096 passes
        // Test cursor at exactly 4097 bytes (should fail)
        let oversized = "a".repeat(4097);
        let result = Cursor::decode(&oversized);
        assert!(matches!(result, Err(CursorError::TooLarge)));

        // Test at exactly 4096 bytes (should attempt decode, not TooLarge)
        let at_limit = "a".repeat(4096);
        let result = Cursor::decode(&at_limit);
        // May be InvalidBase64 or InvalidFormat, but not TooLarge
        assert!(!matches!(result, Err(CursorError::TooLarge)));
    }

    #[test]
    fn test_into_cursor_boundary_behavior() {
        // Empty string
        let cursor: Option<Cursor> = "".into_cursor();
        assert!(cursor.is_none(), "Empty string should return None");

        // At size limit
        let oversized = "a".repeat(4097);
        let cursor: Option<Cursor> = oversized.as_str().into_cursor();
        assert!(cursor.is_none(), "Oversized cursor should return None");
    }

    #[test]
    fn test_cursor_with_various_value_types() {
        // Test cursor with all supported value types
        let cursor = Cursor::new()
            .int("int_field", 42)
            .string("str_field", "hello")
            .field("float_field", 1.234f64)
            .field("bool_field", true);

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(decoded.fields.len(), 4);

        // Verify each field type
        assert!(matches!(
            decoded.fields.iter().find(|(k, _)| k == "int_field"),
            Some((_, Value::Int(42)))
        ));
        assert!(matches!(
            decoded.fields.iter().find(|(k, _)| k == "str_field"),
            Some((_, Value::String(s))) if s == "hello"
        ));
    }

    #[test]
    fn test_cursor_with_special_json_characters() {
        // Test cursor with values that need JSON escaping
        let cursor = Cursor::new()
            .string("quotes", "say \"hello\"")
            .string("backslash", "path\\to\\file")
            .string("newline", "line1\nline2");

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(decoded.fields.len(), 3);
    }

    #[test]
    fn test_keyset_with_missing_cursor_field() {
        // Sort by field not in cursor should return None
        let sorts = vec![SortField::new("missing_field", SortDir::Asc)];
        let cursor = Cursor::new().int("id", 100);

        let condition = KeysetCondition::after(&sorts, &cursor);
        assert!(
            condition.is_none(),
            "Should return None when cursor missing required field"
        );
    }

    #[test]
    fn test_keyset_with_empty_sorts() {
        let cursor = Cursor::new().int("id", 100);
        let condition = KeysetCondition::after(&[], &cursor);
        assert!(
            condition.is_none(),
            "Should return None for empty sort list"
        );
    }
}
