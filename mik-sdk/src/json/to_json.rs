//! ToJson trait and all implementations.

use super::builder::{arr, float, int, null, str};
use super::value::JsonValue;

/// A trait for types that can be converted to JSON values.
///
/// This trait enables type inference in the `json!` and `ok!` macros,
/// allowing you to write:
///
/// ```ignore
/// ok!({ "name": name, "age": age })
/// ```
///
/// Instead of the more verbose:
///
/// ```ignore
/// ok!({ "name": str(name), "age": int(age) })
/// ```
///
/// # Implementations
///
/// This trait is implemented for:
/// - Strings: `String`, `&str`, `&String`, `Cow<str>`
/// - Integers: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `isize`
/// - Floats: `f32`, `f64`
/// - Boolean: `bool`
/// - Optional: `Option<T>` where T: ToJson (None becomes null)
/// - Arrays: `Vec<T>`, `&[T]` where T: ToJson
/// - JSON: `JsonValue` (pass-through)
///
/// # Example
///
/// ```ignore
/// use mik_sdk::json::{ToJson, JsonValue};
///
/// let name = "Alice".to_string();
/// let age = 30;
/// let active = true;
/// let tags: Vec<&str> = vec!["admin", "user"];
///
/// // All these types implement ToJson
/// let json = json::obj()
///     .set("name", name.to_json())
///     .set("age", age.to_json())
///     .set("active", active.to_json())
///     .set("tags", tags.to_json());
/// ```
pub trait ToJson {
    /// Convert this value to a JSON value.
    fn to_json(&self) -> JsonValue;
}

// === String implementations ===

impl ToJson for String {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(self)
    }
}

impl ToJson for &str {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(*self)
    }
}

impl ToJson for std::borrow::Cow<'_, str> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        str(self.as_ref())
    }
}

// === Integer implementations ===

impl ToJson for i8 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i16 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for i64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self)
    }
}

impl ToJson for isize {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self as i64)
    }
}

impl ToJson for u8 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u16 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(i64::from(*self))
    }
}

impl ToJson for u64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        // Note: Values > i64::MAX will be truncated
        int(*self as i64)
    }
}

impl ToJson for usize {
    #[inline]
    fn to_json(&self) -> JsonValue {
        int(*self as i64)
    }
}

// === Float implementations ===

impl ToJson for f32 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        float(f64::from(*self))
    }
}

impl ToJson for f64 {
    #[inline]
    fn to_json(&self) -> JsonValue {
        float(*self)
    }
}

// === Boolean implementation ===

impl ToJson for bool {
    #[inline]
    fn to_json(&self) -> JsonValue {
        super::builder::bool(*self)
    }
}

// === Option implementation (None becomes null) ===

impl<T: ToJson> ToJson for Option<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        self.as_ref().map_or_else(null, ToJson::to_json)
    }
}

// === Array implementations ===

impl<T: ToJson> ToJson for Vec<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in self {
            result = result.push(item.to_json());
        }
        result
    }
}

impl<T: ToJson> ToJson for &[T] {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in *self {
            result = result.push(item.to_json());
        }
        result
    }
}

// Fixed-size array implementations for common sizes
impl<T: ToJson, const N: usize> ToJson for [T; N] {
    #[inline]
    fn to_json(&self) -> JsonValue {
        let mut result = arr();
        for item in self {
            result = result.push(item.to_json());
        }
        result
    }
}

// === JsonValue pass-through ===

impl ToJson for JsonValue {
    #[inline]
    fn to_json(&self) -> JsonValue {
        self.clone()
    }
}

// === Reference implementations ===

impl<T: ToJson + ?Sized> ToJson for &T {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (*self).to_json()
    }
}

impl<T: ToJson + ?Sized> ToJson for &mut T {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson + ?Sized> ToJson for Box<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson> ToJson for std::rc::Rc<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}

impl<T: ToJson> ToJson for std::sync::Arc<T> {
    #[inline]
    fn to_json(&self) -> JsonValue {
        (**self).to_json()
    }
}
