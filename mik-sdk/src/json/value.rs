//! JsonValue struct and all its methods.

use super::lazy;
use miniserde::json::{Array, Number, Object, Value};
use std::rc::Rc;

/// Internal representation of a JSON value.
/// Supports both lazy (byte scanning) and parsed (tree) modes.
#[derive(Clone)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) enum JsonInner {
    /// Lazy mode: stores raw bytes, uses scanning for path_* methods.
    Lazy { bytes: Rc<[u8]> },
    /// Parsed mode: fully parsed tree (used for builder APIs and tree traversal).
    Parsed(Rc<Value>),
}

/// A JSON value with fluent builder API and lazy parsing.
///
/// # Lazy Parsing
///
/// When created via `json::try_parse()`, the value starts in lazy mode.
/// The `path_*` methods scan the raw bytes without building a full tree,
/// which is **10-40x faster** when you only need a few fields.
///
/// Operations that require the full tree (`get()`, `at()`, `keys()`, etc.)
/// trigger a full parse on first access, which is then cached.
///
/// # Thread Safety
///
/// `JsonValue` uses `Rc<Value>` internally and is **not** `Send` or `Sync`.
/// It cannot be shared across threads. This is intentional for WASM targets
/// where single-threaded execution is the norm and `Rc` provides cheaper
/// reference counting than `Arc`.
///
/// If you need thread-safe JSON values, consider using a different JSON
/// library like `serde_json` with its thread-safe `Value` type.
#[derive(Clone)]
pub struct JsonValue {
    pub(crate) inner: JsonInner,
}

impl JsonValue {
    /// Create a JsonValue from a parsed Value (eager mode).
    pub(crate) fn new(v: Value) -> Self {
        Self {
            inner: JsonInner::Parsed(Rc::new(v)),
        }
    }

    /// Create a JsonValue from raw bytes (lazy mode).
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            inner: JsonInner::Lazy {
                bytes: Rc::from(bytes),
            },
        }
    }

    pub(crate) fn null() -> Self {
        Self::new(Value::Null)
    }

    /// Get the raw bytes if in lazy mode.
    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        match &self.inner {
            JsonInner::Lazy { bytes } => Some(bytes),
            JsonInner::Parsed(_) => None,
        }
    }

    /// Parse the bytes and return the Value. Used for tree operations.
    pub(crate) fn parse_bytes(bytes: &[u8]) -> Option<Value> {
        let s = std::str::from_utf8(bytes).ok()?;
        miniserde::json::from_str(s).ok()
    }

    /// Get the Value reference, parsing if needed.
    /// For methods that need the full tree.
    pub(crate) fn value(&self) -> &Value {
        // Static null for returning when parse fails
        static NULL: Value = Value::Null;

        match &self.inner {
            JsonInner::Parsed(v) => v,
            JsonInner::Lazy { .. } => &NULL,
        }
    }

    // === Reading (chainable) ===

    /// Get the Value for tree operations, parsing if in lazy mode.
    fn get_value_for_tree(&self) -> Value {
        match &self.inner {
            JsonInner::Parsed(v) => (**v).clone(),
            JsonInner::Lazy { bytes } => Self::parse_bytes(bytes).unwrap_or(Value::Null),
        }
    }

    /// Get object field (returns null if missing or not an object).
    ///
    /// Note: This triggers a full parse if in lazy mode. For extracting
    /// specific fields, prefer `path_str()`, `path_int()`, etc. which use
    /// lazy scanning.
    #[must_use]
    pub fn get(&self, key: &str) -> Self {
        match self.get_value_for_tree() {
            Value::Object(obj) => obj.get(key).cloned().map_or_else(Self::null, Self::new),
            _ => Self::null(),
        }
    }

    /// Get array element (returns null if out of bounds or not an array).
    ///
    /// Note: This triggers a full parse if in lazy mode and clones the
    /// underlying Value. For parsing large arrays, use `map_array()` or
    /// `try_map_array()` instead for better performance.
    #[must_use]
    pub fn at(&self, index: usize) -> Self {
        match self.get_value_for_tree() {
            Value::Array(arr) => arr.get(index).cloned().map_or_else(Self::null, Self::new),
            _ => Self::null(),
        }
    }

    /// Process array elements without per-element cloning.
    ///
    /// This is more efficient than calling `at(i)` in a loop because it
    /// avoids cloning each element's Value. Returns `None` if not an array.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    ///
    /// # Example
    ///
    /// ```
    /// # use mik_sdk::json::{self, RawValue};
    /// let value = json::arr()
    ///     .push(json::str("hello"))
    ///     .push(json::str("world"));
    /// let strings: Option<Vec<String>> = value.map_array(|v| {
    ///     match v {
    ///         RawValue::String(s) => Some(s.clone()),
    ///         _ => None,
    ///     }
    /// });
    /// assert_eq!(strings, Some(vec!["hello".to_string(), "world".to_string()]));
    /// ```
    #[must_use]
    pub fn map_array<T, F>(&self, f: F) -> Option<Vec<T>>
    where
        F: Fn(&Value) -> Option<T>,
    {
        match self.get_value_for_tree() {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for elem in &arr {
                    result.push(f(elem)?);
                }
                Some(result)
            },
            _ => None,
        }
    }

    /// Process array elements with error handling, without per-element cloning.
    ///
    /// Like `map_array()`, but the function can return errors.
    /// Returns `None` if not an array, `Some(Err(_))` if parsing fails.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn try_map_array<T, E, F>(&self, f: F) -> Option<Result<Vec<T>, E>>
    where
        F: Fn(&Value) -> Result<T, E>,
    {
        match self.get_value_for_tree() {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for elem in &arr {
                    match f(elem) {
                        Ok(v) => result.push(v),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(result))
            },
            _ => None,
        }
    }

    /// Wrap a raw Value reference in a temporary JsonValue for parsing.
    ///
    /// This is useful inside `map_array`/`try_map_array` callbacks when you
    /// need to use JsonValue methods like `get()` or `str()`.
    ///
    /// Note: The returned JsonValue clones the Value, so use sparingly.
    #[must_use]
    pub fn from_raw(value: &Value) -> Self {
        Self::new(value.clone())
    }

    /// As string, None if not a string.
    #[must_use]
    pub fn str(&self) -> Option<String> {
        match self.get_value_for_tree() {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// As string, or default if not a string.
    #[must_use]
    pub fn str_or(&self, default: &str) -> String {
        self.str().unwrap_or_else(|| default.to_string())
    }

    /// As integer, None if not a number.
    #[must_use]
    pub fn int(&self) -> Option<i64> {
        match self.get_value_for_tree() {
            Value::Number(n) => match n {
                Number::I64(i) => Some(i),
                Number::U64(u) => u.try_into().ok(),
                Number::F64(f) => {
                    const MAX_SAFE_INT: f64 = 9007199254740992.0; // 2^53
                    if f.is_finite() && f.abs() <= MAX_SAFE_INT {
                        Some(f as i64)
                    } else {
                        None
                    }
                },
            },
            _ => None,
        }
    }

    /// As integer, or default if not a number.
    #[must_use]
    pub fn int_or(&self, default: i64) -> i64 {
        self.int().unwrap_or(default)
    }

    /// As float, None if not a number.
    ///
    /// # Precision Warning
    ///
    /// Converting large integers to f64 may lose precision. Integers with
    /// absolute value > 2^53 (9,007,199,254,740,992) cannot be represented
    /// exactly in f64. For large integers, use [`int()`](Self::int) instead.
    ///
    /// Non-finite values (NaN, Infinity) return `None`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Documented: large i64/u64 may lose precision
    pub fn float(&self) -> Option<f64> {
        match self.get_value_for_tree() {
            Value::Number(n) => match n {
                Number::F64(f) if f.is_finite() => Some(f),
                Number::I64(i) => Some(i as f64),
                Number::U64(u) => Some(u as f64),
                Number::F64(_) => None, // Non-finite f64
            },
            _ => None,
        }
    }

    /// As float, or default if not a number.
    ///
    /// See [`float()`](Self::float) for precision warnings.
    #[must_use]
    pub fn float_or(&self, default: f64) -> f64 {
        self.float().unwrap_or(default)
    }

    /// As boolean, None if not a boolean.
    #[must_use]
    pub fn bool(&self) -> Option<bool> {
        match self.get_value_for_tree() {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// As boolean, or default if not a boolean.
    #[must_use]
    pub fn bool_or(&self, default: bool) -> bool {
        self.bool().unwrap_or(default)
    }

    /// Is this value null?
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self.get_value_for_tree(), Value::Null)
    }

    /// Get object keys (empty if not an object).
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        match self.get_value_for_tree() {
            Value::Object(obj) => obj.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// Get array/object length.
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn len(&self) -> Option<usize> {
        match self.get_value_for_tree() {
            Value::Array(arr) => Some(arr.len()),
            Value::Object(obj) => Some(obj.len()),
            _ => None,
        }
    }

    /// Is this an empty array/object?
    ///
    /// Note: This triggers a full parse if in lazy mode.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len().is_some_and(|l| l == 0)
    }

    // === Path-based accessors (lazy scanning when possible) ===

    /// Navigate to a nested value by path, returning a reference to the raw Value.
    ///
    /// This requires a full parse. For lazy scanning, use `path_str`, `path_int`, etc.
    fn get_path(&self, path: &[&str]) -> Option<&Value> {
        let mut current = self.value();
        for key in path {
            match current {
                Value::Object(obj) => {
                    current = obj.get(*key)?;
                },
                _ => return None,
            }
        }
        Some(current)
    }

    /// Get string at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    /// This is **10-40x faster** than full parsing when you only need a few fields.
    ///
    /// # Example
    ///
    /// ```
    /// # use mik_sdk::json;
    /// let body = br#"{"user":{"name":"Alice"}}"#;
    /// let parsed = json::try_parse(body).unwrap();
    /// let name = parsed.path_str(&["user", "name"]);  // Lazy scan: ~500ns
    /// assert_eq!(name, Some("Alice".to_string()));
    /// ```
    #[must_use]
    pub fn path_str(&self, path: &[&str]) -> Option<String> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_str(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Get string at path, or default.
    #[must_use]
    pub fn path_str_or(&self, path: &[&str], default: &str) -> String {
        self.path_str(path).unwrap_or_else(|| default.to_string())
    }

    /// Get integer at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_int(&self, path: &[&str]) -> Option<i64> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_int(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::Number(n) => match n {
                Number::I64(i) => Some(*i),
                Number::U64(u) => (*u).try_into().ok(),
                Number::F64(f) => {
                    const MAX_SAFE_INT: f64 = 9007199254740992.0;
                    if f.is_finite() && f.abs() <= MAX_SAFE_INT {
                        Some(*f as i64)
                    } else {
                        None
                    }
                },
            },
            _ => None,
        }
    }

    /// Get integer at path, or default.
    #[must_use]
    pub fn path_int_or(&self, path: &[&str], default: i64) -> i64 {
        self.path_int(path).unwrap_or(default)
    }

    /// Get float at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Documented: large i64/u64 may lose precision
    pub fn path_float(&self, path: &[&str]) -> Option<f64> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_float(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::Number(n) => match n {
                Number::F64(f) if f.is_finite() => Some(*f),
                Number::I64(i) => Some(*i as f64),
                Number::U64(u) => Some(*u as f64),
                Number::F64(_) => None, // Non-finite f64
            },
            _ => None,
        }
    }

    /// Get float at path, or default.
    #[must_use]
    pub fn path_float_or(&self, path: &[&str], default: f64) -> f64 {
        self.path_float(path).unwrap_or(default)
    }

    /// Get boolean at path.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_bool(&self, path: &[&str]) -> Option<bool> {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_bool(bytes, path);
        }

        // Fallback: tree traversal
        match self.get_path(path)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get boolean at path, or default.
    #[must_use]
    pub fn path_bool_or(&self, path: &[&str], default: bool) -> bool {
        self.path_bool(path).unwrap_or(default)
    }

    /// Check if value at path is null.
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_is_null(&self, path: &[&str]) -> bool {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_is_null(bytes, path);
        }

        // Fallback: tree traversal
        matches!(self.get_path(path), Some(Value::Null))
    }

    /// Check if path exists (even if null).
    ///
    /// When in lazy mode, this scans the raw bytes without parsing the full tree.
    #[must_use]
    pub fn path_exists(&self, path: &[&str]) -> bool {
        // Fast path: lazy scanning
        if let Some(bytes) = self.bytes() {
            return lazy::path_exists(bytes, path);
        }

        // Fallback: tree traversal
        self.get_path(path).is_some()
    }

    // === Building (fluent) ===

    /// Get mutable access to the parsed value, converting from lazy if needed.
    fn get_parsed_mut(&mut self) -> &mut Rc<Value> {
        // First, ensure we're in parsed mode
        if let JsonInner::Lazy { bytes } = &self.inner {
            let value = Self::parse_bytes(bytes).unwrap_or(Value::Null);
            self.inner = JsonInner::Parsed(Rc::new(value));
        }

        // Now we're guaranteed to be in Parsed mode
        match &mut self.inner {
            JsonInner::Parsed(rc) => rc,
            JsonInner::Lazy { .. } => unreachable!(),
        }
    }

    /// Set object field (creates object if needed).
    ///
    /// Uses copy-on-write via `Rc::make_mut` - only clones the object if
    /// there are multiple references. For typical builder patterns like
    /// `obj().set("a", v1).set("b", v2)`, this is O(1) per set, not O(n).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // API design: take ownership for builder pattern
    pub fn set(mut self, key: &str, value: Self) -> Self {
        let inner_val = value.value().clone();
        let rc = self.get_parsed_mut();
        let val_mut = Rc::make_mut(rc);

        if let Value::Object(obj) = val_mut {
            obj.insert(key.to_string(), inner_val);
        } else {
            // Not an object, create new one
            let mut obj = Object::new();
            obj.insert(key.to_string(), inner_val);
            *val_mut = Value::Object(obj);
        }

        self
    }

    /// Push to array (creates array if needed).
    ///
    /// Uses copy-on-write via `Rc::make_mut` - only clones the array if
    /// there are multiple references. For typical builder patterns like
    /// `arr().push(v1).push(v2)`, this is O(1) per push, not O(n).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // API design: take ownership for builder pattern
    pub fn push(mut self, value: Self) -> Self {
        let inner_val = value.value().clone();
        let rc = self.get_parsed_mut();
        let val_mut = Rc::make_mut(rc);

        if let Value::Array(arr) = val_mut {
            arr.push(inner_val);
        } else {
            // Not an array, create new one
            let mut arr = Array::new();
            arr.push(inner_val);
            *val_mut = Value::Array(arr);
        }

        self
    }

    // === Output ===

    /// Serialize to JSON bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl std::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            // Lazy mode: bytes are already valid JSON, write directly
            JsonInner::Lazy { bytes } => {
                // Safe: parse() validated UTF-8 before creating Lazy
                let s = std::str::from_utf8(bytes).unwrap_or("null");
                f.write_str(s)
            },
            // Parsed mode: serialize the value
            JsonInner::Parsed(v) => {
                write!(f, "{}", miniserde::json::to_string(&**v))
            },
        }
    }
}

impl std::fmt::Debug for JsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
