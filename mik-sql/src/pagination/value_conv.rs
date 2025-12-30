//! Value conversion helpers for pagination.

use crate::builder::Value;

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_from_conversions() {
        let _: Value = 42i64.into();
        let _: Value = 42i32.into();
        let _: Value = 1.234f64.into();
        let _: Value = "hello".into();
        let _: Value = String::from("world").into();
        let _: Value = true.into();
    }
}
