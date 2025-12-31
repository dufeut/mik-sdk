//! Cursor encoding/decoding for pagination.

use crate::builder::Value;

use super::encoding::{base64_decode, base64_encode, escape_json, split_json_pairs, unescape_json};

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
#[non_exhaustive]
#[must_use = "cursor must be encoded with .encode() or used with a query builder"]
pub struct Cursor {
    /// Field values that define the cursor position.
    pub fields: Vec<(String, Value)>,
}

impl Cursor {
    /// Create a new empty cursor.
    #[must_use]
    pub const fn new() -> Self {
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
        let mut cursor = Self::new();
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
            Self::InvalidBase64 => write!(f, "invalid base64 encoding"),
            Self::InvalidFormat => write!(f, "invalid cursor format"),
            Self::TooLarge => write!(f, "cursor exceeds maximum size"),
            Self::TooManyFields => write!(f, "cursor has too many fields"),
        }
    }
}

impl std::error::Error for CursorError {}

/// Trait for types that can be converted into a cursor.
///
/// Provides flexible DX for cursor pagination methods.
///
/// # Example
///
/// ```
/// # use mik_sql::{Cursor, IntoCursor};
/// // Cursor directly
/// let cursor = Cursor::new().int("id", 100);
/// assert!(cursor.into_cursor().is_some());
///
/// // Base64 encoded string
/// let encoded = Cursor::new().int("id", 42).encode();
/// let decoded: Option<Cursor> = encoded.as_str().into_cursor();
/// assert!(decoded.is_some());
///
/// // Option<&str> - None returns None
/// let none: Option<&str> = None;
/// assert!(none.into_cursor().is_none());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pagination::encoding::base64_encode;

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
            fields.push(format!("\"f{i}\":1"));
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
    fn test_cursor_exactly_at_max_fields() {
        // Create JSON with exactly MAX_CURSOR_FIELDS (16) fields - should succeed
        let mut fields = Vec::new();
        for i in 0..16 {
            fields.push(format!("\"f{i}\":1"));
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
            fields.push(format!("\"f{i}\":1"));
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
            fields.push(format!("\"f{i}\":1"));
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
    fn test_cursor_from_helper() {
        use super::super::PageInfo;

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
