//! Executor: abstraction for how conversion plans are run.
//!
//! The Executor trait separates WHAT to convert (Planner) from HOW to run it
//! (resource management, parallelism, memory constraints). Core stays pure;
//! execution policy is pluggable.
//!
//! See ADR-0006 for design rationale.

use crate::converter::ConvertError;
use crate::planner::Plan;
use crate::properties::Properties;
use crate::registry::Registry;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Context for executing conversion plans.
#[derive(Clone)]
pub struct ExecutionContext {
    /// Registry of available converters.
    pub registry: Arc<Registry>,
    /// Optional memory limit in bytes.
    pub memory_limit: Option<usize>,
    /// Optional parallelism limit (max concurrent jobs).
    pub parallelism: Option<usize>,
}

impl ExecutionContext {
    /// Create a new execution context with the given registry.
    pub fn new(registry: Arc<Registry>) -> Self {
        Self {
            registry,
            memory_limit: None,
            parallelism: None,
        }
    }

    /// Set memory limit.
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.memory_limit = Some(bytes);
        self
    }

    /// Set parallelism limit.
    pub fn with_parallelism(mut self, jobs: usize) -> Self {
        self.parallelism = Some(jobs);
        self
    }
}

/// Result of executing a conversion plan.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Output data.
    pub data: Vec<u8>,
    /// Output properties.
    pub props: Properties,
    /// Execution statistics.
    pub stats: ExecutionStats,
}

/// Statistics from plan execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// Total execution duration.
    pub duration: Duration,
    /// Peak memory usage estimate (bytes).
    pub peak_memory: usize,
    /// Number of converter steps executed.
    pub steps_executed: usize,
}

/// A conversion job for batch processing.
pub struct Job {
    /// The plan to execute.
    pub plan: Plan,
    /// Input data.
    pub input: Vec<u8>,
    /// Input properties.
    pub props: Properties,
}

impl Job {
    /// Create a new job.
    pub fn new(plan: Plan, input: Vec<u8>, props: Properties) -> Self {
        Self { plan, input, props }
    }
}

/// Errors that can occur during plan execution.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error("conversion failed at step {step}: {source}")]
    ConversionFailed {
        step: usize,
        #[source]
        source: ConvertError,
    },

    #[error("converter not found: {0}")]
    ConverterNotFound(String),

    #[error("memory limit exceeded: need {needed} bytes, limit {limit} bytes")]
    MemoryLimitExceeded { needed: usize, limit: usize },

    #[error("empty plan")]
    EmptyPlan,
}

/// Executor determines HOW a plan runs.
///
/// Different executors provide different resource management policies:
/// - `SimpleExecutor`: Sequential, unbounded memory (default)
/// - `BoundedExecutor`: Sequential with memory limit checking (fail-fast)
/// - `ParallelExecutor`: Parallel with memory budget (requires `parallel` feature)
pub trait Executor: Send + Sync {
    /// Execute a single conversion plan.
    fn execute(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: Vec<u8>,
        props: Properties,
    ) -> Result<ExecutionResult, ExecuteError>;

    /// Execute a batch of independent conversion jobs.
    ///
    /// Default implementation runs sequentially.
    fn execute_batch(
        &self,
        ctx: &ExecutionContext,
        jobs: Vec<Job>,
    ) -> Vec<Result<ExecutionResult, ExecuteError>> {
        jobs.into_iter()
            .map(|job| self.execute(ctx, &job.plan, job.input, job.props))
            .collect()
    }
}

/// Simple sequential executor with no resource limits.
///
/// Suitable for CLI single-file conversions where memory isn't a concern.
#[derive(Debug, Clone, Default)]
pub struct SimpleExecutor;

impl SimpleExecutor {
    /// Create a new simple executor.
    pub fn new() -> Self {
        Self
    }
}

impl Executor for SimpleExecutor {
    fn execute(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: Vec<u8>,
        props: Properties,
    ) -> Result<ExecutionResult, ExecuteError> {
        let start = Instant::now();
        let mut current_data = input;
        let mut current_props = props;
        let mut peak_memory = current_data.len();

        for (step_idx, step) in plan.steps.iter().enumerate() {
            let converter = ctx
                .registry
                .get(&step.converter_id)
                .ok_or_else(|| ExecuteError::ConverterNotFound(step.converter_id.clone()))?;

            let output = converter
                .convert(&current_data, &current_props)
                .map_err(|e| ExecuteError::ConversionFailed {
                    step: step_idx,
                    source: e,
                })?;

            // Extract single output
            let (data, props) = match output {
                crate::ConvertOutput::Single(data, props) => (data, props),
                crate::ConvertOutput::Multiple(mut outputs) => {
                    // For simple execution, take first output
                    outputs.pop().ok_or(ExecuteError::EmptyPlan)?
                }
            };

            // Track peak memory
            peak_memory = peak_memory.max(data.len());

            current_data = data;
            current_props = props;
        }

        Ok(ExecutionResult {
            data: current_data,
            props: current_props,
            stats: ExecutionStats {
                duration: start.elapsed(),
                peak_memory,
                steps_executed: plan.steps.len(),
            },
        })
    }
}

// ============================================================================
// Memory Budget
// ============================================================================

/// Memory budget for controlling concurrent memory usage.
///
/// Uses atomic operations to track memory reservations. When a permit is
/// acquired, that memory is "reserved" and released when the permit is dropped.
#[derive(Debug)]
pub struct MemoryBudget {
    /// Maximum allowed memory in bytes.
    limit: usize,
    /// Currently reserved memory in bytes.
    used: AtomicUsize,
}

impl MemoryBudget {
    /// Create a new memory budget with the given limit.
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }

    /// Try to reserve memory. Returns a permit if successful, None if would exceed limit.
    pub fn try_reserve(&self, bytes: usize) -> Option<MemoryPermit<'_>> {
        loop {
            let current = self.used.load(Ordering::Acquire);
            let new_used = current.checked_add(bytes)?;

            if new_used > self.limit {
                return None;
            }

            if self
                .used
                .compare_exchange_weak(current, new_used, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(MemoryPermit {
                    budget: self,
                    bytes,
                });
            }
            // CAS failed, retry
        }
    }

    /// Get current memory usage.
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }

    /// Get memory limit.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Release reserved memory (called by MemoryPermit::drop).
    fn release(&self, bytes: usize) {
        self.used.fetch_sub(bytes, Ordering::AcqRel);
    }
}

/// RAII guard for reserved memory. Releases memory when dropped.
#[derive(Debug)]
pub struct MemoryPermit<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}

impl<'a> MemoryPermit<'a> {
    /// Get the number of bytes reserved by this permit.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for MemoryPermit<'_> {
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}

// ============================================================================
// Bounded Executor
// ============================================================================

/// Sequential executor with memory limit checking.
///
/// Checks estimated memory usage before execution and fails fast if it
/// would exceed the configured limit. Useful for preventing OOM on large files.
#[derive(Debug, Clone, Default)]
pub struct BoundedExecutor;

impl BoundedExecutor {
    /// Create a new bounded executor.
    pub fn new() -> Self {
        Self
    }
}

impl Executor for BoundedExecutor {
    fn execute(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: Vec<u8>,
        props: Properties,
    ) -> Result<ExecutionResult, ExecuteError> {
        // Check memory limit before starting
        if let Some(limit) = ctx.memory_limit {
            let estimated = estimate_memory(input.len(), plan);
            if estimated > limit {
                return Err(ExecuteError::MemoryLimitExceeded {
                    needed: estimated,
                    limit,
                });
            }
        }

        // Delegate to simple executor for actual execution
        let start = Instant::now();
        let mut current_data = input;
        let mut current_props = props;
        let mut peak_memory = current_data.len();

        for (step_idx, step) in plan.steps.iter().enumerate() {
            let converter = ctx
                .registry
                .get(&step.converter_id)
                .ok_or_else(|| ExecuteError::ConverterNotFound(step.converter_id.clone()))?;

            let output = converter
                .convert(&current_data, &current_props)
                .map_err(|e| ExecuteError::ConversionFailed {
                    step: step_idx,
                    source: e,
                })?;

            let (data, props) = match output {
                crate::ConvertOutput::Single(data, props) => (data, props),
                crate::ConvertOutput::Multiple(mut outputs) => {
                    outputs.pop().ok_or(ExecuteError::EmptyPlan)?
                }
            };

            peak_memory = peak_memory.max(data.len());
            current_data = data;
            current_props = props;
        }

        Ok(ExecutionResult {
            data: current_data,
            props: current_props,
            stats: ExecutionStats {
                duration: start.elapsed(),
                peak_memory,
                steps_executed: plan.steps.len(),
            },
        })
    }
}

// ============================================================================
// Parallel Executor (requires "parallel" feature)
// ============================================================================

/// Parallel executor with memory budget for batch processing.
///
/// Uses rayon for parallel execution with backpressure based on memory budget.
/// Jobs that would exceed the memory limit are skipped (returned as errors).
#[cfg(feature = "parallel")]
#[derive(Debug, Clone, Default)]
pub struct ParallelExecutor;

#[cfg(feature = "parallel")]
impl ParallelExecutor {
    /// Create a new parallel executor.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "parallel")]
impl Executor for ParallelExecutor {
    fn execute(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: Vec<u8>,
        props: Properties,
    ) -> Result<ExecutionResult, ExecuteError> {
        // Single execution uses bounded executor
        BoundedExecutor.execute(ctx, plan, input, props)
    }

    fn execute_batch(
        &self,
        ctx: &ExecutionContext,
        jobs: Vec<Job>,
    ) -> Vec<Result<ExecutionResult, ExecuteError>> {
        use rayon::prelude::*;

        let memory_limit = ctx.memory_limit.unwrap_or(usize::MAX);
        let budget = Arc::new(MemoryBudget::new(memory_limit));

        // Configure thread pool size if specified
        let pool = if let Some(parallelism) = ctx.parallelism {
            rayon::ThreadPoolBuilder::new()
                .num_threads(parallelism)
                .build()
                .ok()
        } else {
            None
        };

        let execute_job = |job: Job| {
            let estimated = estimate_memory(job.input.len(), &job.plan);

            // Try to acquire memory permit
            let _permit = match budget.try_reserve(estimated) {
                Some(permit) => permit,
                None => {
                    return Err(ExecuteError::MemoryLimitExceeded {
                        needed: estimated,
                        limit: memory_limit,
                    });
                }
            };

            // Execute with permit held (released on drop)
            BoundedExecutor.execute(ctx, &job.plan, job.input, job.props)
        };

        if let Some(pool) = pool {
            pool.install(|| jobs.into_par_iter().map(execute_job).collect())
        } else {
            jobs.into_par_iter().map(execute_job).collect()
        }
    }
}

// ============================================================================
// Memory Estimation
// ============================================================================

/// Estimate peak memory for a conversion plan.
///
/// This is a heuristic based on typical expansion factors:
/// - Audio: ~10x (compressed to PCM)
/// - Images: ~4x (compressed to RGBA)
/// - Video: ~100x (compressed to raw frames)
/// - Serde: ~1x (roughly same size)
pub fn estimate_memory(input_size: usize, plan: &Plan) -> usize {
    let mut estimate = input_size;

    for step in &plan.steps {
        estimate = match step.converter_id.as_str() {
            s if s.starts_with("audio.") => estimate.saturating_mul(10),
            s if s.starts_with("image.") => estimate.saturating_mul(4),
            s if s.starts_with("video.") => estimate.saturating_mul(100),
            _ => estimate,
        };
    }

    estimate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConvertOutput, Converter, ConverterDecl, PropertiesExt, PropertyPattern};

    struct IdentityConverter {
        decl: ConverterDecl,
    }

    impl IdentityConverter {
        fn new(from: &str, to: &str) -> Self {
            let id = format!("test.{}-to-{}", from, to);
            let decl = ConverterDecl::simple(
                &id,
                PropertyPattern::new().eq("format", from),
                PropertyPattern::new().eq("format", to),
            );
            Self { decl }
        }
    }

    impl Converter for IdentityConverter {
        fn decl(&self) -> &ConverterDecl {
            &self.decl
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let mut out_props = props.clone();
            // Update format to output format
            let to_format = self
                .decl
                .outputs
                .get("out")
                .and_then(|p| p.pattern.predicates.get("format"))
                .and_then(|pred| {
                    if let crate::Predicate::Eq(v) = pred {
                        v.as_str()
                    } else {
                        None
                    }
                })
                .unwrap_or("unknown");
            out_props.insert("format".into(), to_format.into());
            Ok(ConvertOutput::Single(input.to_vec(), out_props))
        }
    }

    #[test]
    fn test_simple_executor() {
        let mut registry = Registry::new();
        registry.register(IdentityConverter::new("a", "b"));
        registry.register(IdentityConverter::new("b", "c"));

        let ctx = ExecutionContext::new(Arc::new(registry));

        let plan = Plan {
            steps: vec![
                crate::PlanStep {
                    converter_id: "test.a-to-b".into(),
                    input_port: "in".into(),
                    output_port: "out".into(),
                    output_properties: Properties::new().with("format", "b"),
                },
                crate::PlanStep {
                    converter_id: "test.b-to-c".into(),
                    input_port: "in".into(),
                    output_port: "out".into(),
                    output_properties: Properties::new().with("format", "c"),
                },
            ],
            cost: 2.0,
        };

        let executor = SimpleExecutor::new();
        let input = b"test data".to_vec();
        let props = Properties::new().with("format", "a");

        let result = executor.execute(&ctx, &plan, input.clone(), props).unwrap();

        assert_eq!(result.data, input);
        assert_eq!(
            result.props.get("format").and_then(|v| v.as_str()),
            Some("c")
        );
        assert_eq!(result.stats.steps_executed, 2);
    }

    #[test]
    fn test_execute_empty_plan() {
        let registry = Registry::new();
        let ctx = ExecutionContext::new(Arc::new(registry));

        let plan = Plan {
            steps: vec![],
            cost: 0.0,
        };

        let executor = SimpleExecutor::new();
        let input = b"test data".to_vec();
        let props = Properties::new().with("format", "a");

        let result = executor
            .execute(&ctx, &plan, input.clone(), props.clone())
            .unwrap();

        // Empty plan should return input unchanged
        assert_eq!(result.data, input);
        assert_eq!(result.props, props);
        assert_eq!(result.stats.steps_executed, 0);
    }

    #[test]
    fn test_estimate_memory() {
        let plan = Plan {
            steps: vec![crate::PlanStep {
                converter_id: "audio.mp3-to-wav".into(),
                input_port: "in".into(),
                output_port: "out".into(),
                output_properties: Properties::new(),
            }],
            cost: 1.0,
        };

        let estimate = estimate_memory(1000, &plan);
        assert_eq!(estimate, 10000); // 10x for audio
    }

    #[test]
    fn test_memory_budget() {
        let budget = MemoryBudget::new(100);

        // Can reserve within limit
        let permit1 = budget.try_reserve(40).expect("should succeed");
        assert_eq!(budget.used(), 40);

        let permit2 = budget.try_reserve(40).expect("should succeed");
        assert_eq!(budget.used(), 80);

        // Cannot exceed limit
        assert!(budget.try_reserve(30).is_none());
        assert_eq!(budget.used(), 80);

        // Release frees memory
        drop(permit1);
        assert_eq!(budget.used(), 40);

        // Can reserve again
        let _permit3 = budget.try_reserve(50).expect("should succeed");
        assert_eq!(budget.used(), 90);

        drop(permit2);
        assert_eq!(budget.used(), 50);
    }

    #[test]
    fn test_bounded_executor_within_limit() {
        let mut registry = Registry::new();
        registry.register(IdentityConverter::new("a", "b"));

        let ctx = ExecutionContext::new(Arc::new(registry)).with_memory_limit(1000);

        let plan = Plan {
            steps: vec![crate::PlanStep {
                converter_id: "test.a-to-b".into(),
                input_port: "in".into(),
                output_port: "out".into(),
                output_properties: Properties::new().with("format", "b"),
            }],
            cost: 1.0,
        };

        let executor = BoundedExecutor::new();
        let input = b"small".to_vec();
        let props = Properties::new().with("format", "a");

        let result = executor.execute(&ctx, &plan, input, props);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bounded_executor_exceeds_limit() {
        let mut registry = Registry::new();
        registry.register(IdentityConverter::new("a", "b"));

        // Set a very small limit
        let ctx = ExecutionContext::new(Arc::new(registry)).with_memory_limit(1);

        let plan = Plan {
            steps: vec![crate::PlanStep {
                converter_id: "test.a-to-b".into(),
                input_port: "in".into(),
                output_port: "out".into(),
                output_properties: Properties::new().with("format", "b"),
            }],
            cost: 1.0,
        };

        let executor = BoundedExecutor::new();
        let input = b"this is too large".to_vec();
        let props = Properties::new().with("format", "a");

        let result = executor.execute(&ctx, &plan, input, props);
        assert!(matches!(
            result,
            Err(ExecuteError::MemoryLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_execute_batch() {
        let mut registry = Registry::new();
        registry.register(IdentityConverter::new("a", "b"));

        let ctx = ExecutionContext::new(Arc::new(registry));

        let plan = Plan {
            steps: vec![crate::PlanStep {
                converter_id: "test.a-to-b".into(),
                input_port: "in".into(),
                output_port: "out".into(),
                output_properties: Properties::new().with("format", "b"),
            }],
            cost: 1.0,
        };

        let jobs = vec![
            Job::new(
                plan.clone(),
                b"one".to_vec(),
                Properties::new().with("format", "a"),
            ),
            Job::new(
                plan.clone(),
                b"two".to_vec(),
                Properties::new().with("format", "a"),
            ),
            Job::new(
                plan,
                b"three".to_vec(),
                Properties::new().with("format", "a"),
            ),
        ];

        let executor = SimpleExecutor::new();
        let results = executor.execute_batch(&ctx, jobs);

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}
