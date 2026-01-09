//! Serde-based format converters for Cambium.
//!
//! This crate provides converters between various data serialization formats
//! using the serde ecosystem. Enable formats via feature flags.
//!
//! # Features
//!
//! ## Text formats (human-readable)
//! - `json` (default) - JSON via serde_json
//! - `yaml` (default) - YAML via serde_yaml
//! - `toml` (default) - TOML via toml
//! - `ron` - Rusty Object Notation
//! - `json5` - JSON5 (JSON with comments/trailing commas)
//! - `xml` - XML via quick-xml
//! - `lexpr` - S-expressions (Lisp-style)
//! - `urlencoded` - URL-encoded form data
//! - `qs` - Query strings
//! - `csv` - CSV (limited to arrays of flat objects)
//!
//! ## Binary formats (compact/efficient)
//! - `msgpack` - MessagePack binary format
//! - `cbor` - CBOR (RFC 8949)
//! - `bincode` - Fast binary encoding
//! - `postcard` - Embedded-friendly binary format
//! - `bson` - Binary JSON (MongoDB)
//! - `flexbuffers` - Schemaless FlatBuffers
//! - `bencode` - BitTorrent encoding
//! - `pickle` - Python's serialization format
//! - `plist` - Apple Property List
//!
//! ## Feature group
//! - `all` - All serde formats

use cambium::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled serde converters with the registry.
pub fn register_all(registry: &mut Registry) {
    let formats = enabled_formats();

    // Register converters between all pairs of enabled formats
    for from in &formats {
        for to in &formats {
            if from != to {
                registry.register(SerdeConverter::new(from, to));
            }
        }
    }
}

/// Get list of enabled formats based on feature flags.
pub fn enabled_formats() -> Vec<&'static str> {
    [
        // Text formats
        #[cfg(feature = "json")]
        "json",
        #[cfg(feature = "yaml")]
        "yaml",
        #[cfg(feature = "toml")]
        "toml",
        #[cfg(feature = "ron")]
        "ron",
        #[cfg(feature = "json5")]
        "json5",
        #[cfg(feature = "xml")]
        "xml",
        #[cfg(feature = "lexpr")]
        "lexpr",
        #[cfg(feature = "urlencoded")]
        "urlencoded",
        #[cfg(feature = "qs")]
        "qs",
        // Binary formats
        #[cfg(feature = "msgpack")]
        "msgpack",
        #[cfg(feature = "cbor")]
        "cbor",
        #[cfg(feature = "bincode")]
        "bincode",
        #[cfg(feature = "postcard")]
        "postcard",
        #[cfg(feature = "bson")]
        "bson",
        #[cfg(feature = "flexbuffers")]
        "flexbuffers",
        #[cfg(feature = "bencode")]
        "bencode",
        #[cfg(feature = "pickle")]
        "pickle",
        #[cfg(feature = "plist")]
        "plist",
        // CSV is special - only works with arrays of flat objects
        // Don't include in general conversion matrix
    ]
    .into()
}

/// A converter between two serde-compatible formats.
pub struct SerdeConverter {
    decl: ConverterDecl,
    from: &'static str,
    to: &'static str,
}

impl SerdeConverter {
    pub fn new(from: &'static str, to: &'static str) -> Self {
        let id = format!("serde.{}-to-{}", from, to);
        let decl = ConverterDecl::simple(
            &id,
            PropertyPattern::new().eq("format", from),
            PropertyPattern::new().eq("format", to),
        )
        .description(format!(
            "Convert {} to {} via serde",
            from.to_uppercase(),
            to.to_uppercase()
        ));

        Self { decl, from, to }
    }
}

impl Converter for SerdeConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Deserialize from source format
        let value: serde_json::Value = deserialize(self.from, input)?;

        // Serialize to target format
        let output = serialize(self.to, &value)?;

        // Update properties
        let mut out_props = props.clone();
        out_props.insert("format".into(), self.to.into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Deserialize bytes to a serde Value.
fn deserialize(format: &str, data: &[u8]) -> Result<serde_json::Value, ConvertError> {
    match format {
        // === Text formats ===
        #[cfg(feature = "json")]
        "json" => serde_json::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e))),

        #[cfg(feature = "yaml")]
        "yaml" => serde_yaml::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid YAML: {}", e))),

        #[cfg(feature = "toml")]
        "toml" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            toml::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid TOML: {}", e)))
        }

        #[cfg(feature = "ron")]
        "ron" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            ron::from_str(s).map_err(|e| ConvertError::InvalidInput(format!("Invalid RON: {}", e)))
        }

        #[cfg(feature = "json5")]
        "json5" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            json5::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON5: {}", e)))
        }

        #[cfg(feature = "xml")]
        "xml" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            quick_xml::de::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid XML: {}", e)))
        }

        #[cfg(feature = "lexpr")]
        "lexpr" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_lexpr::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid S-expression: {}", e)))
        }

        #[cfg(feature = "urlencoded")]
        "urlencoded" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_urlencoded::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid URL-encoded: {}", e)))
        }

        #[cfg(feature = "qs")]
        "qs" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_qs::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid query string: {}", e)))
        }

        // === Binary formats ===
        #[cfg(feature = "msgpack")]
        "msgpack" => rmp_serde::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid MessagePack: {}", e))),

        #[cfg(feature = "cbor")]
        "cbor" => ciborium::from_reader(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid CBOR: {}", e))),

        #[cfg(feature = "bincode")]
        "bincode" => {
            let (value, _): (serde_json::Value, _) =
                bincode::serde::decode_from_slice(data, bincode::config::standard())
                    .map_err(|e| ConvertError::InvalidInput(format!("Invalid Bincode: {}", e)))?;
            Ok(value)
        }

        #[cfg(feature = "postcard")]
        "postcard" => postcard::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Postcard: {}", e))),

        #[cfg(feature = "bson")]
        "bson" => bson::de::deserialize_from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid BSON: {}", e))),

        #[cfg(feature = "flexbuffers")]
        "flexbuffers" => flexbuffers::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid FlexBuffers: {}", e))),

        #[cfg(feature = "bencode")]
        "bencode" => serde_bencode::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Bencode: {}", e))),

        #[cfg(feature = "pickle")]
        "pickle" => serde_pickle::from_slice(data, serde_pickle::DeOptions::default())
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Pickle: {}", e))),

        #[cfg(feature = "plist")]
        "plist" => plist::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Property List: {}", e))),

        _ => Err(ConvertError::Failed(format!(
            "Unsupported source format: {}",
            format
        ))),
    }
}

/// Serialize a serde Value to bytes.
fn serialize(format: &str, value: &serde_json::Value) -> Result<Vec<u8>, ConvertError> {
    match format {
        // === Text formats ===
        #[cfg(feature = "json")]
        "json" => serde_json::to_vec_pretty(value)
            .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e))),

        #[cfg(feature = "yaml")]
        "yaml" => serde_yaml::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("YAML serialization failed: {}", e))),

        #[cfg(feature = "toml")]
        "toml" => toml::to_string_pretty(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("TOML serialization failed: {}", e))),

        #[cfg(feature = "ron")]
        "ron" => ron::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("RON serialization failed: {}", e))),

        #[cfg(feature = "json5")]
        "json5" => {
            // json5 crate doesn't have serialization, output as JSON (compatible)
            serde_json::to_vec_pretty(value)
                .map_err(|e| ConvertError::Failed(format!("JSON5 serialization failed: {}", e)))
        }

        #[cfg(feature = "xml")]
        "xml" => quick_xml::se::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("XML serialization failed: {}", e))),

        #[cfg(feature = "lexpr")]
        "lexpr" => serde_lexpr::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("S-expression serialization failed: {}", e))),

        #[cfg(feature = "urlencoded")]
        "urlencoded" => serde_urlencoded::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("URL-encoded serialization failed: {}", e))),

        #[cfg(feature = "qs")]
        "qs" => serde_qs::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("Query string serialization failed: {}", e))),

        // === Binary formats ===
        #[cfg(feature = "msgpack")]
        "msgpack" => rmp_serde::to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("MessagePack serialization failed: {}", e))),

        #[cfg(feature = "cbor")]
        "cbor" => {
            let mut buf = Vec::new();
            ciborium::into_writer(value, &mut buf)
                .map_err(|e| ConvertError::Failed(format!("CBOR serialization failed: {}", e)))?;
            Ok(buf)
        }

        #[cfg(feature = "bincode")]
        "bincode" => bincode::serde::encode_to_vec(value, bincode::config::standard())
            .map_err(|e| ConvertError::Failed(format!("Bincode serialization failed: {}", e))),

        #[cfg(feature = "postcard")]
        "postcard" => postcard::to_allocvec(value)
            .map_err(|e| ConvertError::Failed(format!("Postcard serialization failed: {}", e))),

        #[cfg(feature = "bson")]
        "bson" => bson::ser::serialize_to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("BSON serialization failed: {}", e))),

        #[cfg(feature = "flexbuffers")]
        "flexbuffers" => flexbuffers::to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("FlexBuffers serialization failed: {}", e))),

        #[cfg(feature = "bencode")]
        "bencode" => serde_bencode::to_bytes(value)
            .map_err(|e| ConvertError::Failed(format!("Bencode serialization failed: {}", e))),

        #[cfg(feature = "pickle")]
        "pickle" => serde_pickle::to_vec(value, serde_pickle::SerOptions::default())
            .map_err(|e| ConvertError::Failed(format!("Pickle serialization failed: {}", e))),

        #[cfg(feature = "plist")]
        "plist" => {
            let mut buf = Vec::new();
            plist::to_writer_binary(&mut buf, value).map_err(|e| {
                ConvertError::Failed(format!("Property List serialization failed: {}", e))
            })?;
            Ok(buf)
        }

        _ => Err(ConvertError::Failed(format!(
            "Unsupported target format: {}",
            format
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::PropertiesExt;

    #[test]
    #[cfg(all(feature = "json", feature = "yaml"))]
    fn test_json_to_yaml() {
        let converter = SerdeConverter::new("json", "yaml");
        let input = br#"{"name": "test", "value": 42}"#;
        let props = Properties::new().with("format", "json");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("name:"));
                assert!(output_str.contains("test"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("yaml"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(all(feature = "yaml", feature = "json"))]
    fn test_yaml_to_json() {
        let converter = SerdeConverter::new("yaml", "json");
        let input = b"name: test\nvalue: 42\n";
        let props = Properties::new().with("format", "yaml");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("\"name\""));
                assert!(output_str.contains("\"test\""));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("json"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(all(feature = "json", feature = "toml"))]
    fn test_json_to_toml() {
        let converter = SerdeConverter::new("json", "toml");
        let input = br#"{"name": "test", "value": 42}"#;
        let props = Properties::new().with("format", "json");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("name"));
                assert!(output_str.contains("test"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("toml"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    fn test_register_all() {
        let mut registry = Registry::new();
        register_all(&mut registry);

        // Should have n*(n-1) converters for n formats
        let n = enabled_formats().len();
        assert_eq!(registry.len(), n * (n - 1));
    }

    #[test]
    #[cfg(all(feature = "json", feature = "yaml"))]
    fn test_roundtrip() {
        let original = br#"{"name": "roundtrip", "nested": {"a": 1, "b": 2}}"#;

        let json_to_yaml = SerdeConverter::new("json", "yaml");
        let yaml_to_json = SerdeConverter::new("yaml", "json");

        let props = Properties::new().with("format", "json");

        // JSON -> YAML
        let yaml_result = json_to_yaml.convert(original, &props).unwrap();
        let (yaml_bytes, yaml_props) = match yaml_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        // YAML -> JSON
        let json_result = yaml_to_json.convert(&yaml_bytes, &yaml_props).unwrap();
        let (json_bytes, _) = match json_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        // Parse both and compare
        let original_value: serde_json::Value = serde_json::from_slice(original).unwrap();
        let roundtrip_value: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
        assert_eq!(original_value, roundtrip_value);
    }
}
