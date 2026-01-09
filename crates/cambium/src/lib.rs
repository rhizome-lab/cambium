//! Cambium: Type-driven data transformation pipeline
//!
//! Cambium is a route planner for data conversion. Given source and target
//! properties, it finds a path through available converters.

mod converter;
mod pattern;
mod planner;
mod properties;
mod registry;
mod workflow;

pub use converter::{ConvertError, ConvertOutput, Converter, ConverterDecl, PortDecl};
pub use pattern::{Predicate, PropertyPattern};
pub use planner::{Cardinality, Plan, PlanStep, Planner};
pub use properties::{Properties, PropertiesExt, Value};
pub use registry::Registry;
pub use workflow::{Sink, Source, Step, Workflow, WorkflowError};
