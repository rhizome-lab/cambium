//! Converter declarations and traits.

use crate::pattern::PropertyPattern;
use crate::properties::Properties;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Declaration of a port (input or output) on a converter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortDecl {
    /// Pattern that data on this port must match.
    pub pattern: PropertyPattern,
    /// If true, this port expects/produces a list of items.
    /// If false, this port handles single items.
    #[serde(default)]
    pub list: bool,
}

impl PortDecl {
    /// Create a new port declaration for single items.
    pub fn single(pattern: PropertyPattern) -> Self {
        Self {
            pattern,
            list: false,
        }
    }

    /// Create a new port declaration for lists.
    pub fn list(pattern: PropertyPattern) -> Self {
        Self {
            pattern,
            list: true,
        }
    }
}

/// Declaration of a converter's interface.
///
/// Describes what properties a converter requires and produces,
/// without containing the actual conversion logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConverterDecl {
    /// Unique identifier for this converter.
    pub id: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Named input ports.
    pub inputs: IndexMap<String, PortDecl>,
    /// Named output ports.
    pub outputs: IndexMap<String, PortDecl>,
    /// Cost metrics for path optimization.
    #[serde(default)]
    pub costs: Properties,
}

impl ConverterDecl {
    /// Create a new converter declaration.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: String::new(),
            inputs: IndexMap::new(),
            outputs: IndexMap::new(),
            costs: Properties::new(),
        }
    }

    /// Set the description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add an input port.
    pub fn input(mut self, name: impl Into<String>, port: PortDecl) -> Self {
        self.inputs.insert(name.into(), port);
        self
    }

    /// Add an output port.
    pub fn output(mut self, name: impl Into<String>, port: PortDecl) -> Self {
        self.outputs.insert(name.into(), port);
        self
    }

    /// Convenience: single input, single output converter.
    pub fn simple(
        id: impl Into<String>,
        input_pattern: PropertyPattern,
        output_pattern: PropertyPattern,
    ) -> Self {
        Self::new(id)
            .input("in", PortDecl::single(input_pattern))
            .output("out", PortDecl::single(output_pattern))
    }

    /// Set a cost property for path optimization.
    ///
    /// Common cost keys:
    /// - `quality_loss`: higher = more quality degradation
    /// - `speed`: higher = slower processing
    /// - `size`: higher = larger output
    /// - `cost`: generic fallback cost
    pub fn cost(mut self, key: impl Into<String>, value: f64) -> Self {
        use crate::properties::Value;
        self.costs.insert(key.into(), Value::from(value));
        self
    }

    /// Check if this converter can handle the given input properties.
    ///
    /// For single-input converters, checks the "in" port.
    /// Returns the name of the matching input port, if any.
    pub fn matches_input(&self, props: &Properties) -> Option<&str> {
        self.inputs
            .iter()
            .find(|(_, port)| port.pattern.matches(props))
            .map(|(name, _)| name.as_str())
    }

    /// Get the output pattern for a named port.
    pub fn output_pattern(&self, port_name: &str) -> Option<&PropertyPattern> {
        self.outputs.get(port_name).map(|p| &p.pattern)
    }

    /// Check if this is a simple 1→1 converter (single non-list input and output).
    pub fn is_simple(&self) -> bool {
        self.inputs.len() == 1
            && self.outputs.len() == 1
            && !self.inputs.values().next().unwrap().list
            && !self.outputs.values().next().unwrap().list
    }

    /// Check if this converter aggregates (any input port expects a list).
    pub fn aggregates(&self) -> bool {
        self.inputs.values().any(|p| p.list)
    }

    /// Check if this converter expands (any output port produces a list).
    pub fn expands(&self) -> bool {
        self.outputs.values().any(|p| p.list)
    }

    /// Check if this converter has multiple input ports.
    pub fn has_multi_input(&self) -> bool {
        self.inputs.len() > 1
    }

    /// Get the names of all input ports.
    pub fn input_names(&self) -> impl Iterator<Item = &str> {
        self.inputs.keys().map(|s| s.as_str())
    }
}

/// Result of a conversion operation.
pub enum ConvertOutput {
    /// Single output item.
    Single(Vec<u8>, Properties),
    /// Multiple output items (for expanders or multi-output).
    Multiple(Vec<(Vec<u8>, Properties)>),
}

/// A named input for multi-input converters.
pub struct NamedInput<'a> {
    pub data: &'a [u8],
    pub props: &'a Properties,
}

/// Trait for implementing converters.
///
/// Converters transform data from one form to another.
pub trait Converter: Send + Sync {
    /// Get the declaration for this converter.
    fn decl(&self) -> &ConverterDecl;

    /// Convert a single input (for simple converters with one "in" port).
    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError>;

    /// Convert with multiple named inputs (for multi-input converters).
    ///
    /// The keys in `inputs` correspond to the input port names in the declaration.
    /// Default implementation returns an error - override for multi-input converters.
    fn convert_multi(
        &self,
        inputs: &IndexMap<String, NamedInput<'_>>,
    ) -> Result<ConvertOutput, ConvertError> {
        let _ = inputs;
        Err(ConvertError::MultiInputNotSupported)
    }

    /// Convert a batch of inputs (for aggregating converters).
    ///
    /// Default implementation returns an error.
    fn convert_batch(
        &self,
        inputs: &[(&[u8], &Properties)],
    ) -> Result<ConvertOutput, ConvertError> {
        let _ = inputs;
        Err(ConvertError::BatchNotSupported)
    }
}

/// Errors that can occur during conversion.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("conversion failed: {0}")]
    Failed(String),

    #[error("batch conversion not supported by this converter")]
    BatchNotSupported,

    #[error("multi-input conversion not supported by this converter")]
    MultiInputNotSupported,

    #[error("missing required input port: {0}")]
    MissingInput(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("missing required property: {0}")]
    MissingProperty(String),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_converter_decl() {
        let decl = ConverterDecl::simple(
            "png-to-webp",
            PropertyPattern::new().eq("format", "png"),
            PropertyPattern::new().eq("format", "webp"),
        );

        assert_eq!(decl.id, "png-to-webp");
        assert!(decl.is_simple());
        assert!(!decl.aggregates());
        assert!(!decl.expands());
    }

    #[test]
    fn test_aggregator_decl() {
        let decl = ConverterDecl::new("frames-to-video")
            .input(
                "frames",
                PortDecl::list(PropertyPattern::new().eq("format", "png")),
            )
            .output(
                "video",
                PortDecl::single(PropertyPattern::new().eq("format", "mp4")),
            );

        assert!(!decl.is_simple());
        assert!(decl.aggregates());
        assert!(!decl.expands());
    }

    #[test]
    fn test_expander_decl() {
        let decl = ConverterDecl::new("video-to-frames")
            .input(
                "video",
                PortDecl::single(PropertyPattern::new().eq("format", "mp4")),
            )
            .output(
                "frames",
                PortDecl::list(PropertyPattern::new().eq("format", "png")),
            );

        assert!(!decl.is_simple());
        assert!(!decl.aggregates());
        assert!(decl.expands());
    }

    #[test]
    fn test_multi_output_decl() {
        let decl = ConverterDecl::new("with-sidecar")
            .input(
                "in",
                PortDecl::single(PropertyPattern::new().eq("format", "png")),
            )
            .output(
                "image",
                PortDecl::single(PropertyPattern::new().eq("format", "webp")),
            )
            .output(
                "sidecar",
                PortDecl::single(PropertyPattern::new().eq("format", "json")),
            );

        assert!(!decl.is_simple()); // multiple outputs
        assert!(!decl.aggregates());
        assert!(!decl.expands());
        assert_eq!(decl.outputs.len(), 2);
    }

    #[test]
    fn test_matches_input() {
        let decl = ConverterDecl::simple(
            "png-to-webp",
            PropertyPattern::new().eq("format", "png"),
            PropertyPattern::new().eq("format", "webp"),
        );

        use crate::properties::PropertiesExt;

        let png_props = Properties::new().with("format", "png");
        let jpg_props = Properties::new().with("format", "jpg");

        assert_eq!(decl.matches_input(&png_props), Some("in"));
        assert_eq!(decl.matches_input(&jpg_props), None);
    }
}
