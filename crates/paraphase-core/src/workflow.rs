//! Workflow definition and execution.
//!
//! Workflows are serializable pipelines that define:
//! - Source: where input comes from (file, glob)
//! - Steps: converters to apply (optional for auto-planning)
//! - Sink: where output goes
//!
//! Incomplete workflows (missing steps) trigger auto-planning.

use crate::pattern::PropertyPattern;
use crate::properties::{Properties, Value};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Workflow {
    /// Optional preset to apply.
    #[serde(default)]
    pub preset: Option<String>,

    /// Source specification.
    #[serde(default)]
    pub source: Option<Source>,

    /// Explicit steps (if empty, planner will suggest).
    #[serde(default)]
    pub steps: Vec<Step>,

    /// Sink specification.
    #[serde(default)]
    pub sink: Option<Sink>,

    /// Global options that apply to all steps.
    #[serde(default)]
    pub options: IndexMap<String, Value>,
}

/// Source specification - where input comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// Single file path.
    File { path: String },
    /// Glob pattern for multiple files.
    Glob { glob: String },
    /// Inline properties (for planning without files).
    Properties { properties: Properties },
}

impl Source {
    /// Get source properties for planning.
    pub fn to_properties(&self) -> Properties {
        match self {
            Source::File { path } => {
                let mut props = Properties::new();
                props.insert("path".into(), Value::String(path.clone()));
                if let Some(format) = detect_format(path) {
                    props.insert("format".into(), Value::String(format));
                }
                props
            }
            Source::Glob { glob } => {
                let mut props = Properties::new();
                props.insert("glob".into(), Value::String(glob.clone()));
                // Try to detect format from glob pattern
                if let Some(format) = detect_format(glob) {
                    props.insert("format".into(), Value::String(format));
                }
                props
            }
            Source::Properties { properties } => properties.clone(),
        }
    }

    /// Check if this source represents multiple files.
    pub fn is_batch(&self) -> bool {
        matches!(self, Source::Glob { .. })
    }
}

/// Sink specification - where output goes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sink {
    /// Single file path.
    File { path: String },
    /// Directory (for batch output).
    Directory { directory: String },
    /// Target properties (for planning).
    Properties { properties: Properties },
}

impl Sink {
    /// Get target pattern for planning.
    pub fn to_pattern(&self) -> PropertyPattern {
        match self {
            Sink::File { path } => {
                let mut pattern = PropertyPattern::new();
                if let Some(format) = detect_format(path) {
                    pattern = pattern.eq("format", format);
                }
                pattern
            }
            Sink::Directory { directory } => {
                // Can't determine format from directory alone
                let _ = directory;
                PropertyPattern::new()
            }
            Sink::Properties { properties } => {
                let mut pattern = PropertyPattern::new();
                for (key, value) in properties {
                    pattern = pattern.eq(key.clone(), value.clone());
                }
                pattern
            }
        }
    }
}

/// A step in the workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Converter ID to use.
    pub converter: String,

    /// Options for this converter.
    #[serde(default)]
    pub options: IndexMap<String, Value>,

    /// Optional ID for this step (for referencing outputs).
    #[serde(default)]
    pub id: Option<String>,

    /// Input port to use (defaults to first/only input).
    #[serde(default)]
    pub input: Option<String>,

    /// Output port to use (defaults to first/only output).
    #[serde(default)]
    pub output: Option<String>,
}

impl Workflow {
    /// Create a new empty workflow.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source.
    pub fn source(mut self, source: Source) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the source from a file path.
    pub fn source_file(self, path: impl Into<String>) -> Self {
        self.source(Source::File { path: path.into() })
    }

    /// Set the source from a glob pattern.
    pub fn source_glob(self, glob: impl Into<String>) -> Self {
        self.source(Source::Glob { glob: glob.into() })
    }

    /// Add a step.
    pub fn step(mut self, converter: impl Into<String>) -> Self {
        self.steps.push(Step {
            converter: converter.into(),
            options: IndexMap::new(),
            id: None,
            input: None,
            output: None,
        });
        self
    }

    /// Set the sink.
    pub fn sink(mut self, sink: Sink) -> Self {
        self.sink = Some(sink);
        self
    }

    /// Set the sink to a file path.
    pub fn sink_file(self, path: impl Into<String>) -> Self {
        self.sink(Sink::File { path: path.into() })
    }

    /// Check if this workflow is complete (has source, sink, and steps).
    pub fn is_complete(&self) -> bool {
        self.source.is_some() && self.sink.is_some() && !self.steps.is_empty()
    }

    /// Check if this workflow needs auto-planning (has source and sink but no steps).
    pub fn needs_planning(&self) -> bool {
        self.source.is_some() && self.sink.is_some() && self.steps.is_empty()
    }

    /// Parse workflow from bytes, auto-detecting format.
    pub fn from_bytes(data: &[u8], path: Option<&str>) -> Result<Self, WorkflowError> {
        let format = path
            .and_then(detect_format)
            .unwrap_or_else(|| "yaml".to_string());

        Self::from_bytes_format(data, &format)
    }

    /// Parse workflow from bytes with explicit format.
    pub fn from_bytes_format(data: &[u8], format: &str) -> Result<Self, WorkflowError> {
        match format {
            "json" => serde_json::from_slice(data).map_err(|e| WorkflowError::Parse(e.to_string())),
            "yaml" | "yml" => {
                serde_yaml::from_slice(data).map_err(|e| WorkflowError::Parse(e.to_string()))
            }
            "toml" => {
                let s = std::str::from_utf8(data)
                    .map_err(|e| WorkflowError::Parse(format!("Invalid UTF-8: {}", e)))?;
                toml::from_str(s).map_err(|e| WorkflowError::Parse(e.to_string()))
            }
            _ => Err(WorkflowError::Parse(format!(
                "Unsupported workflow format: {}",
                format
            ))),
        }
    }

    /// Serialize workflow to bytes.
    pub fn to_bytes(&self, format: &str) -> Result<Vec<u8>, WorkflowError> {
        match format {
            "json" => {
                serde_json::to_vec_pretty(self).map_err(|e| WorkflowError::Parse(e.to_string()))
            }
            "yaml" | "yml" => serde_yaml::to_string(self)
                .map(|s| s.into_bytes())
                .map_err(|e| WorkflowError::Parse(e.to_string())),
            "toml" => toml::to_string_pretty(self)
                .map(|s| s.into_bytes())
                .map_err(|e| WorkflowError::Parse(e.to_string())),
            _ => Err(WorkflowError::Parse(format!(
                "Unsupported workflow format: {}",
                format
            ))),
        }
    }
}

/// Errors related to workflow parsing and execution.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("failed to parse workflow: {0}")]
    Parse(String),

    #[error("incomplete workflow: {0}")]
    Incomplete(String),

    #[error("workflow execution failed: {0}")]
    Execution(String),
}

/// Detect format from file path extension.
fn detect_format(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext.to_lowercase().as_str() {
        "json" => Some("json".into()),
        "yaml" | "yml" => Some("yaml".into()),
        "toml" => Some("toml".into()),
        "ron" => Some("ron".into()),
        "msgpack" | "mp" => Some("msgpack".into()),
        "cbor" => Some("cbor".into()),
        "csv" => Some("csv".into()),
        "png" => Some("png".into()),
        "jpg" | "jpeg" => Some("jpg".into()),
        "webp" => Some("webp".into()),
        "gif" => Some("gif".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::PropertiesExt;

    #[test]
    fn test_workflow_builder() {
        let workflow = Workflow::new()
            .source_file("input.json")
            .step("serde.json-to-yaml")
            .sink_file("output.yaml");

        assert!(workflow.is_complete());
        assert!(!workflow.needs_planning());
    }

    #[test]
    fn test_incomplete_workflow() {
        let workflow = Workflow::new()
            .source_file("input.json")
            .sink_file("output.yaml");
        // No steps

        assert!(!workflow.is_complete());
        assert!(workflow.needs_planning());
    }

    #[test]
    fn test_source_properties() {
        let source = Source::File {
            path: "test.json".into(),
        };
        let props = source.to_properties();

        assert_eq!(props.get("path").unwrap().as_str(), Some("test.json"));
        assert_eq!(props.get("format").unwrap().as_str(), Some("json"));
    }

    #[test]
    fn test_sink_pattern() {
        let sink = Sink::File {
            path: "output.yaml".into(),
        };
        let pattern = sink.to_pattern();

        let props = Properties::new().with("format", "yaml");
        assert!(pattern.matches(&props));

        let props = Properties::new().with("format", "json");
        assert!(!pattern.matches(&props));
    }

    #[test]
    fn test_workflow_json_roundtrip() {
        let workflow = Workflow::new()
            .source_file("input.json")
            .step("serde.json-to-yaml")
            .sink_file("output.yaml");

        let bytes = workflow.to_bytes("json").unwrap();
        let parsed = Workflow::from_bytes_format(&bytes, "json").unwrap();

        assert_eq!(parsed.steps.len(), 1);
        assert_eq!(parsed.steps[0].converter, "serde.json-to-yaml");
    }
}
