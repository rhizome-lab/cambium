//! Property patterns for matching and routing.

use crate::properties::{Properties, Value};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A predicate for matching a single value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    /// Matches any value (just checks existence).
    Any,
    /// Matches exact value.
    Eq(Value),
    /// Matches if not equal.
    Ne(Value),
    /// Numeric greater than.
    Gt(f64),
    /// Numeric greater than or equal.
    Gte(f64),
    /// Numeric less than.
    Lt(f64),
    /// Numeric less than or equal.
    Lte(f64),
    /// String starts with.
    StartsWith(String),
    /// String ends with.
    EndsWith(String),
    /// String contains.
    Contains(String),
    /// Value is one of these.
    OneOf(Vec<Value>),
}

impl Predicate {
    /// Check if a value matches this predicate.
    pub fn matches(&self, value: &Value) -> bool {
        match self {
            Predicate::Any => true,
            Predicate::Eq(expected) => value == expected,
            Predicate::Ne(expected) => value != expected,
            Predicate::Gt(n) => value.as_f64().is_some_and(|v| v > *n),
            Predicate::Gte(n) => value.as_f64().is_some_and(|v| v >= *n),
            Predicate::Lt(n) => value.as_f64().is_some_and(|v| v < *n),
            Predicate::Lte(n) => value.as_f64().is_some_and(|v| v <= *n),
            Predicate::StartsWith(prefix) => value.as_str().is_some_and(|s| s.starts_with(prefix)),
            Predicate::EndsWith(suffix) => value.as_str().is_some_and(|s| s.ends_with(suffix)),
            Predicate::Contains(substr) => value.as_str().is_some_and(|s| s.contains(substr)),
            Predicate::OneOf(values) => values.contains(value),
        }
    }
}

/// A pattern for matching property bags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PropertyPattern {
    /// Predicates that must match for this pattern to match.
    /// Key is property name, value is the predicate to apply.
    #[serde(flatten)]
    pub predicates: IndexMap<String, Predicate>,
}

impl PropertyPattern {
    /// Create an empty pattern (matches anything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a predicate for a property.
    pub fn with(mut self, key: impl Into<String>, predicate: Predicate) -> Self {
        self.predicates.insert(key.into(), predicate);
        self
    }

    /// Shorthand for exact match.
    pub fn eq(self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(key, Predicate::Eq(value.into()))
    }

    /// Shorthand for existence check.
    pub fn exists(self, key: impl Into<String>) -> Self {
        self.with(key, Predicate::Any)
    }

    /// Check if properties match this pattern.
    ///
    /// All predicates must match. Properties may have extra keys.
    pub fn matches(&self, props: &Properties) -> bool {
        self.predicates
            .iter()
            .all(|(key, predicate)| props.get(key).is_some_and(|value| predicate.matches(value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::PropertiesExt;

    #[test]
    fn test_exact_match() {
        let pattern = PropertyPattern::new().eq("format", "png");

        let props = Properties::new().with("format", "png");
        assert!(pattern.matches(&props));

        let props = Properties::new().with("format", "jpg");
        assert!(!pattern.matches(&props));
    }

    #[test]
    fn test_numeric_predicates() {
        let pattern = PropertyPattern::new()
            .with("width", Predicate::Gte(1024.0))
            .with("height", Predicate::Lt(2000.0));

        let props = Properties::new()
            .with("width", 1920i64)
            .with("height", 1080i64);
        assert!(pattern.matches(&props));

        let props = Properties::new()
            .with("width", 800i64)
            .with("height", 600i64);
        assert!(!pattern.matches(&props));
    }

    #[test]
    fn test_string_predicates() {
        let pattern = PropertyPattern::new().with("path", Predicate::EndsWith(".png".to_string()));

        let props = Properties::new().with("path", "image.png");
        assert!(pattern.matches(&props));

        let props = Properties::new().with("path", "image.jpg");
        assert!(!pattern.matches(&props));
    }

    #[test]
    fn test_existence() {
        let pattern = PropertyPattern::new().exists("format");

        let props = Properties::new().with("format", "png");
        assert!(pattern.matches(&props));

        let props = Properties::new().with("other", "value");
        assert!(!pattern.matches(&props));
    }

    #[test]
    fn test_extra_properties_allowed() {
        let pattern = PropertyPattern::new().eq("format", "png");

        let props = Properties::new()
            .with("format", "png")
            .with("width", 100i64)
            .with("extra", "data");
        assert!(pattern.matches(&props));
    }

    #[test]
    fn test_one_of() {
        let pattern = PropertyPattern::new().with(
            "format",
            Predicate::OneOf(vec![
                Value::from("png"),
                Value::from("jpg"),
                Value::from("gif"),
            ]),
        );

        assert!(pattern.matches(&Properties::new().with("format", "png")));
        assert!(pattern.matches(&Properties::new().with("format", "jpg")));
        assert!(!pattern.matches(&Properties::new().with("format", "webp")));
    }
}
