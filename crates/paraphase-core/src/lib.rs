//! Paraphase: Type-driven data transformation pipeline
//!
//! Paraphase is a route planner for data conversion. Given source and target
//! properties, it finds a path through available converters.

mod converter;
mod executor;
mod pattern;
mod planner;
mod properties;
mod registry;
mod workflow;

pub use converter::{ConvertError, ConvertOutput, Converter, ConverterDecl, NamedInput, PortDecl};
#[cfg(feature = "parallel")]
pub use executor::ParallelExecutor;
pub use executor::{
    BoundedExecutor, ExecuteError, ExecutionContext, ExecutionResult, ExecutionStats, Executor,
    Job, MemoryBudget, MemoryPermit, SimpleExecutor, estimate_memory,
};
pub use pattern::{Predicate, PropertyPattern};
pub use planner::{Cardinality, OptimizeTarget, Plan, PlanStep, Planner};
pub use properties::{Properties, PropertiesExt, Value};
pub use registry::Registry;
pub use workflow::{Sink, Source, Step, Workflow, WorkflowError};
