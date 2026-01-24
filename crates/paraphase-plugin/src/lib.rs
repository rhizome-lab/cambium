//! Plugin authoring helpers for Cambium.
//!
//! This crate provides utilities for writing Cambium plugins,
//! including the C ABI exports and procedural macros.

pub use rhi_paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, PortDecl, Predicate, Properties,
    PropertiesExt, PropertyPattern, Value,
};

// TODO: Add #[paraphase_converter] proc macro
// TODO: Add C ABI export helpers
