//! Property bags: the core data model for Cambium.
//!
//! Data is described by property bags, not hierarchical types.
//! Format is just another property.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A JSON-like value that can represent any property.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(IndexMap<String, Value>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&IndexMap<String, Value>> {
        match self {
            Value::Object(obj) => Some(obj),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Int(n as i64)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Float(n)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(arr: Vec<T>) -> Self {
        Value::Array(arr.into_iter().map(Into::into).collect())
    }
}

/// A bag of properties describing some data.
///
/// Properties are flat by default. Use namespacing only when
/// semantics differ (e.g., `image.compression` vs `archive.compression`).
pub type Properties = IndexMap<String, Value>;

/// Extension trait for building Properties ergonomically.
pub trait PropertiesExt {
    fn with(self, key: impl Into<String>, value: impl Into<Value>) -> Self;
}

impl PropertiesExt for Properties {
    fn with(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversions() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i64), Value::Int(42));
        assert_eq!(Value::from(3.15f64), Value::Float(3.15));
        assert_eq!(Value::from("hello"), Value::String("hello".into()));
    }

    #[test]
    fn test_properties_builder() {
        let props = Properties::new()
            .with("format", "png")
            .with("width", 1024i64)
            .with("height", 768i64);

        assert_eq!(props.get("format").and_then(Value::as_str), Some("png"));
        assert_eq!(props.get("width").and_then(Value::as_i64), Some(1024));
    }

    #[test]
    fn test_value_accessors() {
        let v = Value::Int(42);
        assert_eq!(v.as_i64(), Some(42));
        assert_eq!(v.as_f64(), Some(42.0));
        assert_eq!(v.as_str(), None);
    }
}
