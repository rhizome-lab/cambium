//! Planning: find conversion paths through the registry.
//!
//! Given source and target properties, the planner searches for a
//! sequence of converters that transforms the source to the target.

use crate::converter::ConverterDecl;
use crate::pattern::PropertyPattern;
use crate::properties::Properties;
use crate::registry::Registry;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// Optimization target for path selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OptimizeTarget {
    /// Minimize quality loss (prefer lossless or high-quality paths).
    Quality,
    /// Minimize processing time (prefer fast converters).
    #[default]
    Speed,
    /// Minimize output size (prefer compression, lossy formats).
    Size,
}

/// A planned conversion path.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Steps in the plan, in execution order.
    pub steps: Vec<PlanStep>,
    /// Total estimated cost.
    pub cost: f64,
}

/// A single step in a conversion plan.
#[derive(Debug, Clone)]
pub struct PlanStep {
    /// Converter ID.
    pub converter_id: String,
    /// Input port name.
    pub input_port: String,
    /// Output port name.
    pub output_port: String,
    /// Expected output properties after this step.
    pub output_properties: Properties,
}

/// Cardinality of the data flowing through the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Single item.
    One,
    /// Multiple items.
    Many,
}

/// A node in the search space.
#[derive(Debug, Clone)]
struct SearchNode {
    /// Current properties (what we have).
    properties: Properties,
    /// Current cardinality.
    cardinality: Cardinality,
    /// Steps taken to reach this node.
    steps: Vec<PlanStep>,
    /// Cost so far (g in A*).
    cost: f64,
    /// Estimated total cost (f = g + h in A*).
    estimated_total: f64,
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.estimated_total == other.estimated_total
    }
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap (lower cost = higher priority)
        other
            .estimated_total
            .partial_cmp(&self.estimated_total)
            .unwrap_or(Ordering::Equal)
    }
}

/// Planner for finding conversion paths.
pub struct Planner<'a> {
    registry: &'a Registry,
    max_depth: usize,
    optimize: OptimizeTarget,
}

impl<'a> Planner<'a> {
    /// Create a new planner with the given registry.
    pub fn new(registry: &'a Registry) -> Self {
        Self {
            registry,
            max_depth: 10,
            optimize: OptimizeTarget::default(),
        }
    }

    /// Set maximum search depth.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Set optimization target for path selection.
    pub fn optimize(mut self, target: OptimizeTarget) -> Self {
        self.optimize = target;
        self
    }

    /// Find a conversion path from source to target properties.
    ///
    /// Uses A* search to find the lowest-cost path.
    pub fn plan(
        &self,
        source: &Properties,
        target: &PropertyPattern,
        source_cardinality: Cardinality,
        target_cardinality: Cardinality,
    ) -> Option<Plan> {
        // Check if we're already at the goal
        if target.matches(source) && source_cardinality == target_cardinality {
            return Some(Plan {
                steps: vec![],
                cost: 0.0,
            });
        }

        let mut frontier = BinaryHeap::new();
        let mut visited = HashSet::new();

        // Create initial node
        let initial = SearchNode {
            properties: source.clone(),
            cardinality: source_cardinality,
            steps: vec![],
            cost: 0.0,
            estimated_total: self.heuristic(source, target),
        };
        frontier.push(initial);

        while let Some(current) = frontier.pop() {
            // Check depth limit
            if current.steps.len() >= self.max_depth {
                continue;
            }

            // Create a state key for visited check
            let state_key = self.state_key(&current.properties, current.cardinality);
            if visited.contains(&state_key) {
                continue;
            }
            visited.insert(state_key);

            // Check if we've reached the goal
            if target.matches(&current.properties) && current.cardinality == target_cardinality {
                return Some(Plan {
                    steps: current.steps,
                    cost: current.cost,
                });
            }

            // Expand neighbors
            for decl in self.registry.declarations() {
                if let Some(neighbor) = self.try_apply(decl, &current, target, target_cardinality) {
                    let neighbor_key = self.state_key(&neighbor.properties, neighbor.cardinality);
                    if !visited.contains(&neighbor_key) {
                        frontier.push(neighbor);
                    }
                }
            }
        }

        None
    }

    /// Try to apply a converter to the current state.
    fn try_apply(
        &self,
        decl: &ConverterDecl,
        current: &SearchNode,
        target: &PropertyPattern,
        target_cardinality: Cardinality,
    ) -> Option<SearchNode> {
        // Find matching input port
        let (input_port, input_decl) = decl
            .inputs
            .iter()
            .find(|(_, port)| port.pattern.matches(&current.properties))?;

        // Check cardinality compatibility
        let new_cardinality = match (current.cardinality, input_decl.list) {
            // Single item, converter expects single -> OK, stays single
            (Cardinality::One, false) => {
                // Check output cardinality
                if decl.outputs.values().any(|p| p.list) {
                    Cardinality::Many // expander
                } else {
                    Cardinality::One
                }
            }
            // Many items, converter expects single -> OK, maps over batch
            (Cardinality::Many, false) => {
                if decl.outputs.values().any(|p| p.list) {
                    // Each item expands, still many (actually more)
                    Cardinality::Many
                } else {
                    Cardinality::Many
                }
            }
            // Single item, converter expects list -> need aggregation context
            (Cardinality::One, true) => {
                // Can't aggregate a single item (need Many)
                // Unless target is One and we want to "wrap" as 1-item list
                if target_cardinality == Cardinality::One {
                    return None; // Don't auto-aggregate single items
                }
                return None;
            }
            // Many items, converter expects list -> aggregation
            (Cardinality::Many, true) => {
                if decl.outputs.values().any(|p| p.list) {
                    Cardinality::Many // N->M
                } else {
                    Cardinality::One // N->1 aggregation
                }
            }
        };

        // Get the first output port (simple case)
        // TODO: handle multi-output properly
        let (output_port, output_decl) = decl.outputs.iter().next()?;

        // Compute output properties by applying the output pattern
        let mut output_props = current.properties.clone();
        for (key, pred) in &output_decl.pattern.predicates {
            if let crate::pattern::Predicate::Eq(value) = pred {
                output_props.insert(key.clone(), value.clone());
            }
        }

        // Calculate step cost based on optimization target
        let step_cost = self.cost_for_converter(decl);

        let new_cost = current.cost + step_cost;
        let heuristic = self.heuristic(&output_props, target);

        let step = PlanStep {
            converter_id: decl.id.clone(),
            input_port: input_port.clone(),
            output_port: output_port.clone(),
            output_properties: output_props.clone(),
        };

        let mut new_steps = current.steps.clone();
        new_steps.push(step);

        Some(SearchNode {
            properties: output_props,
            cardinality: new_cardinality,
            steps: new_steps,
            cost: new_cost,
            estimated_total: new_cost + heuristic,
        })
    }

    /// Heuristic: estimate remaining cost to goal.
    ///
    /// Currently just counts mismatched properties.
    fn heuristic(&self, current: &Properties, target: &PropertyPattern) -> f64 {
        let mut mismatches = 0;
        for (key, predicate) in &target.predicates {
            if !current.get(key).is_some_and(|v| predicate.matches(v)) {
                mismatches += 1;
            }
        }
        mismatches as f64
    }

    /// Create a state key for visited tracking.
    fn state_key(&self, props: &Properties, cardinality: Cardinality) -> String {
        // Simple key based on format property and cardinality
        // TODO: more sophisticated state representation
        let format = props
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        format!("{}:{:?}", format, cardinality)
    }

    /// Get the cost for a converter based on optimization target.
    ///
    /// Cost properties:
    /// - `quality_loss`: higher = more quality degradation (used for Quality optimization)
    /// - `speed`: higher = slower (used for Speed optimization)
    /// - `size`: higher = larger output (used for Size optimization)
    ///
    /// Falls back to generic `cost` property, then to 1.0.
    fn cost_for_converter(&self, decl: &ConverterDecl) -> f64 {
        let cost_key = match self.optimize {
            OptimizeTarget::Quality => "quality_loss",
            OptimizeTarget::Speed => "speed",
            OptimizeTarget::Size => "size",
        };

        // Try optimization-specific cost, fall back to generic "cost", then 1.0
        decl.costs
            .get(cost_key)
            .and_then(|v| v.as_f64())
            .or_else(|| decl.costs.get("cost").and_then(|v| v.as_f64()))
            .unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::PortDecl;
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

        registry.register_decl(ConverterDecl::simple(
            "webp-to-gif",
            PropertyPattern::new().eq("format", "webp"),
            PropertyPattern::new().eq("format", "gif"),
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
    fn test_direct_conversion() {
        let registry = make_test_registry();
        let planner = Planner::new(&registry);

        let source = Properties::new().with("format", "png");
        let target = PropertyPattern::new().eq("format", "webp");

        let plan = planner
            .plan(&source, &target, Cardinality::One, Cardinality::One)
            .expect("should find plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].converter_id, "png-to-webp");
    }

    #[test]
    fn test_multi_hop_conversion() {
        let registry = make_test_registry();
        let planner = Planner::new(&registry);

        let source = Properties::new().with("format", "png");
        let target = PropertyPattern::new().eq("format", "gif");

        let plan = planner
            .plan(&source, &target, Cardinality::One, Cardinality::One)
            .expect("should find plan");

        // Should find png -> webp -> gif (2 hops)
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].converter_id, "png-to-webp");
        assert_eq!(plan.steps[1].converter_id, "webp-to-gif");
    }

    #[test]
    fn test_already_at_goal() {
        let registry = make_test_registry();
        let planner = Planner::new(&registry);

        let source = Properties::new().with("format", "webp");
        let target = PropertyPattern::new().eq("format", "webp");

        let plan = planner
            .plan(&source, &target, Cardinality::One, Cardinality::One)
            .expect("should find plan");

        assert_eq!(plan.steps.len(), 0);
    }

    #[test]
    fn test_no_path() {
        let registry = make_test_registry();
        let planner = Planner::new(&registry);

        let source = Properties::new().with("format", "bmp");
        let target = PropertyPattern::new().eq("format", "webp");

        let plan = planner.plan(&source, &target, Cardinality::One, Cardinality::One);
        assert!(plan.is_none());
    }

    #[test]
    fn test_aggregation() {
        let registry = make_test_registry();
        let planner = Planner::new(&registry);

        let source = Properties::new().with("format", "png");
        let target = PropertyPattern::new().eq("format", "gif");

        // Many PNGs -> One GIF (should use frames-to-gif aggregator)
        let plan = planner
            .plan(&source, &target, Cardinality::Many, Cardinality::One)
            .expect("should find plan");

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].converter_id, "frames-to-gif");
    }

    #[test]
    fn test_optimize_quality_vs_speed() {
        // Two paths from A to C:
        // - A -> B -> C (fast but lossy)
        // - A -> C (slow but lossless)
        let mut registry = Registry::new();

        // Fast path: A -> B (fast, lossy)
        registry.register_decl(
            ConverterDecl::simple(
                "a-to-b-fast",
                PropertyPattern::new().eq("format", "a"),
                PropertyPattern::new().eq("format", "b"),
            )
            .cost("speed", 0.5)
            .cost("quality_loss", 0.8),
        );

        // Fast path: B -> C (fast, lossy)
        registry.register_decl(
            ConverterDecl::simple(
                "b-to-c-fast",
                PropertyPattern::new().eq("format", "b"),
                PropertyPattern::new().eq("format", "c"),
            )
            .cost("speed", 0.5)
            .cost("quality_loss", 0.8),
        );

        // Slow path: A -> C (slow, lossless)
        registry.register_decl(
            ConverterDecl::simple(
                "a-to-c-slow",
                PropertyPattern::new().eq("format", "a"),
                PropertyPattern::new().eq("format", "c"),
            )
            .cost("speed", 5.0)
            .cost("quality_loss", 0.0),
        );

        let source = Properties::new().with("format", "a");
        let target = PropertyPattern::new().eq("format", "c");

        // Optimize for speed: should take fast 2-hop path (0.5 + 0.5 = 1.0 < 5.0)
        let speed_plan = Planner::new(&registry)
            .optimize(OptimizeTarget::Speed)
            .plan(&source, &target, Cardinality::One, Cardinality::One)
            .expect("should find plan");

        assert_eq!(speed_plan.steps.len(), 2);
        assert!(speed_plan.cost < 2.0);

        // Optimize for quality: should take direct slow path (0.0 < 0.8 + 0.8)
        let quality_plan = Planner::new(&registry)
            .optimize(OptimizeTarget::Quality)
            .plan(&source, &target, Cardinality::One, Cardinality::One)
            .expect("should find plan");

        assert_eq!(quality_plan.steps.len(), 1);
        assert_eq!(quality_plan.steps[0].converter_id, "a-to-c-slow");
    }
}
