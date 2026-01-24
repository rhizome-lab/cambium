//! Registry for converters.

use crate::converter::{Converter, ConverterDecl};
use crate::properties::Properties;
use indexmap::IndexMap;
use std::sync::Arc;

/// Registry of available converters.
///
/// The registry holds converter declarations and (optionally) their implementations.
/// It provides methods for querying which converters can handle given properties.
#[derive(Clone)]
pub struct Registry {
    /// Converter declarations indexed by ID.
    declarations: IndexMap<String, ConverterDecl>,
    /// Converter implementations indexed by ID.
    implementations: IndexMap<String, Arc<dyn Converter>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            declarations: IndexMap::new(),
            implementations: IndexMap::new(),
        }
    }

    /// Register a converter declaration (without implementation).
    ///
    /// Useful for planning when the actual converter isn't available yet.
    pub fn register_decl(&mut self, decl: ConverterDecl) {
        self.declarations.insert(decl.id.clone(), decl);
    }

    /// Register a converter with its implementation.
    pub fn register(&mut self, converter: impl Converter + 'static) {
        let decl = converter.decl().clone();
        let id = decl.id.clone();
        self.declarations.insert(id.clone(), decl);
        self.implementations.insert(id, Arc::new(converter));
    }

    /// Get a converter declaration by ID.
    pub fn get_decl(&self, id: &str) -> Option<&ConverterDecl> {
        self.declarations.get(id)
    }

    /// Get a converter implementation by ID.
    pub fn get(&self, id: &str) -> Option<Arc<dyn Converter>> {
        self.implementations.get(id).cloned()
    }

    /// Iterate over all declarations.
    pub fn declarations(&self) -> impl Iterator<Item = &ConverterDecl> {
        self.declarations.values()
    }

    /// Find all converters that can handle the given input properties.
    ///
    /// Returns converter IDs and the name of the matching input port.
    pub fn find_matching(&self, props: &Properties) -> Vec<(&str, &str)> {
        self.declarations
            .iter()
            .filter_map(|(id, decl)| decl.matches_input(props).map(|port| (id.as_str(), port)))
            .collect()
    }

    /// Find simple (1→1) converters that can handle the given input properties.
    pub fn find_simple_matching(&self, props: &Properties) -> Vec<&ConverterDecl> {
        self.declarations
            .values()
            .filter(|decl| decl.is_simple() && decl.matches_input(props).is_some())
            .collect()
    }

    /// Number of registered converters.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::PortDecl;
    use crate::pattern::PropertyPattern;
    use crate::properties::PropertiesExt;

    fn make_test_registry() -> Registry {
        let mut registry = Registry::new();

        registry.register_decl(ConverterDecl::simple(
            "png-to-webp",
            PropertyPattern::new().eq("format", "png"),
            PropertyPattern::new().eq("format", "webp"),
        ));

        registry.register_decl(ConverterDecl::simple(
            "png-to-jpg",
            PropertyPattern::new().eq("format", "png"),
            PropertyPattern::new().eq("format", "jpg"),
        ));

        registry.register_decl(ConverterDecl::simple(
            "jpg-to-webp",
            PropertyPattern::new().eq("format", "jpg"),
            PropertyPattern::new().eq("format", "webp"),
        ));

        registry.register_decl(
            ConverterDecl::new("frames-to-gif")
                .input(
                    "frames",
                    PortDecl::list(PropertyPattern::new().eq("format", "png")),
                )
                .output(
                    "out",
                    PortDecl::single(PropertyPattern::new().eq("format", "gif")),
                ),
        );

        registry
    }

    #[test]
    fn test_find_matching() {
        let registry = make_test_registry();
        let props = Properties::new().with("format", "png");

        let matches = registry.find_matching(&props);

        // Should find png-to-webp, png-to-jpg, and frames-to-gif (which accepts png list)
        assert!(matches.iter().any(|(id, _)| *id == "png-to-webp"));
        assert!(matches.iter().any(|(id, _)| *id == "png-to-jpg"));
        assert!(matches.iter().any(|(id, _)| *id == "frames-to-gif"));
    }

    #[test]
    fn test_find_simple_matching() {
        let registry = make_test_registry();
        let props = Properties::new().with("format", "png");

        let matches = registry.find_simple_matching(&props);

        // Should find png-to-webp and png-to-jpg, but NOT frames-to-gif (not simple)
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|d| d.id == "png-to-webp"));
        assert!(matches.iter().any(|d| d.id == "png-to-jpg"));
    }

    #[test]
    fn test_no_match() {
        let registry = make_test_registry();
        let props = Properties::new().with("format", "bmp");

        let matches = registry.find_matching(&props);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_get_decl() {
        let registry = make_test_registry();

        assert!(registry.get_decl("png-to-webp").is_some());
        assert!(registry.get_decl("nonexistent").is_none());
    }
}
