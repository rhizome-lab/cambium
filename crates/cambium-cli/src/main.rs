//! Cambium CLI - type-driven data transformation

use anyhow::{Context, Result, bail};
use cambium::{
    Cardinality, ConvertOutput, Planner, Properties, PropertiesExt, PropertyPattern, Registry,
    Sink, Source, Workflow,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cambium")]
#[command(about = "Type-driven data transformation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available converters
    List,

    /// Plan a conversion (show steps without executing)
    Plan {
        /// Input file/format, or workflow file
        input: String,
        /// Output file/format (optional if input is a workflow)
        output: Option<String>,
        /// Explicit source format (overrides detection)
        #[arg(long)]
        from: Option<String>,
        /// Explicit target format (overrides detection)
        #[arg(long)]
        to: Option<String>,
    },

    /// Convert a file
    Convert {
        /// Input file
        input: PathBuf,
        /// Output file
        output: PathBuf,
        /// Explicit source format (overrides detection)
        #[arg(long)]
        from: Option<String>,
        /// Explicit target format (overrides detection)
        #[arg(long)]
        to: Option<String>,
    },

    /// Run a workflow file
    Run {
        /// Workflow file (YAML, TOML, or JSON)
        workflow: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create registry with serde converters
    let mut registry = Registry::new();
    cambium_serde::register_all(&mut registry);

    match cli.command {
        Commands::List => cmd_list(&registry),
        Commands::Plan {
            input,
            output,
            from,
            to,
        } => cmd_plan(&registry, &input, output, from, to),
        Commands::Convert {
            input,
            output,
            from,
            to,
        } => cmd_convert(&registry, &input, &output, from, to),
        Commands::Run { workflow } => cmd_run(&registry, &workflow),
    }
}

fn cmd_list(registry: &Registry) -> Result<()> {
    println!("Available converters:\n");

    for decl in registry.declarations() {
        let inputs: Vec<_> = decl.inputs.keys().collect();
        let outputs: Vec<_> = decl.outputs.keys().collect();

        println!("  {}", decl.id);
        if !decl.description.is_empty() {
            println!("    {}", decl.description);
        }
        println!("    inputs:  {:?}", inputs);
        println!("    outputs: {:?}", outputs);
        println!();
    }

    println!("Total: {} converters", registry.len());
    Ok(())
}

fn cmd_plan(
    registry: &Registry,
    input: &str,
    output: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    // Check if input is a workflow file
    if is_workflow_file(input) {
        return cmd_plan_workflow(registry, input);
    }

    // Otherwise, plan a simple conversion
    let output = output.context("Output required for non-workflow planning")?;

    let source_format = from
        .or_else(|| detect_format(input))
        .context("Could not detect source format. Use --from to specify.")?;

    let target_format = to
        .or_else(|| detect_format(&output))
        .context("Could not detect target format. Use --to to specify.")?;

    println!("Planning: {} -> {}", source_format, target_format);
    println!();

    let source_props = Properties::new().with("format", source_format.as_str());
    let target_pattern = PropertyPattern::new().eq("format", target_format.as_str());

    let planner = Planner::new(registry);
    let plan = planner
        .plan(
            &source_props,
            &target_pattern,
            Cardinality::One,
            Cardinality::One,
        )
        .context("No conversion path found")?;

    if plan.steps.is_empty() {
        println!("Already at target format (no conversion needed)");
    } else {
        println!("Steps:");
        for (i, step) in plan.steps.iter().enumerate() {
            println!(
                "  {}. {} ({} -> {})",
                i + 1,
                step.converter_id,
                step.input_port,
                step.output_port
            );
        }
        println!();
        println!("Total cost: {}", plan.cost);
    }

    Ok(())
}

fn cmd_plan_workflow(registry: &Registry, path: &str) -> Result<()> {
    let data = std::fs::read(path).context("Failed to read workflow file")?;
    let workflow = Workflow::from_bytes(&data, Some(path))
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow: {}", e))?;

    println!("Workflow: {}", path);
    println!();

    // Show source
    if let Some(ref source) = workflow.source {
        println!("Source:");
        match source {
            Source::File { path } => println!("  file: {}", path),
            Source::Glob { glob } => println!("  glob: {}", glob),
            Source::Properties { properties } => println!("  properties: {:?}", properties),
        }
        println!();
    } else {
        println!("Source: (not specified)");
        println!();
    }

    // Show sink
    if let Some(ref sink) = workflow.sink {
        println!("Sink:");
        match sink {
            Sink::File { path } => println!("  file: {}", path),
            Sink::Directory { directory } => println!("  directory: {}", directory),
            Sink::Properties { properties } => println!("  properties: {:?}", properties),
        }
        println!();
    } else {
        println!("Sink: (not specified)");
        println!();
    }

    // If steps are explicit, show them
    if !workflow.steps.is_empty() {
        println!("Explicit steps:");
        for (i, step) in workflow.steps.iter().enumerate() {
            println!("  {}. {}", i + 1, step.converter);
            if !step.options.is_empty() {
                println!("     options: {:?}", step.options);
            }
        }
        println!();
        println!("Status: Complete workflow (ready to run)");
    } else if workflow.needs_planning() {
        // Auto-plan
        println!("Steps: (auto-planning...)");
        println!();

        let source = workflow.source.as_ref().unwrap();
        let sink = workflow.sink.as_ref().unwrap();

        let source_props = source.to_properties();
        let target_pattern = sink.to_pattern();

        let source_cardinality = if source.is_batch() {
            Cardinality::Many
        } else {
            Cardinality::One
        };

        let planner = Planner::new(registry);
        match planner.plan(
            &source_props,
            &target_pattern,
            source_cardinality,
            Cardinality::One,
        ) {
            Some(plan) => {
                println!("Suggested steps:");
                for (i, step) in plan.steps.iter().enumerate() {
                    println!(
                        "  {}. {} ({} -> {})",
                        i + 1,
                        step.converter_id,
                        step.input_port,
                        step.output_port
                    );
                }
                println!();
                println!("Total cost: {}", plan.cost);
                println!();
                println!("Status: Incomplete workflow (add steps or use suggested plan)");
            }
            None => {
                println!("No conversion path found!");
                println!();
                println!("Status: Incomplete workflow (no valid path)");
            }
        }
    } else {
        println!("Status: Incomplete workflow (missing source or sink)");
    }

    Ok(())
}

fn cmd_run(registry: &Registry, workflow_path: &PathBuf) -> Result<()> {
    let data = std::fs::read(workflow_path).context("Failed to read workflow file")?;
    let workflow = Workflow::from_bytes(&data, Some(&workflow_path.to_string_lossy()))
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow: {}", e))?;

    // Get source and sink
    let source = workflow
        .source
        .as_ref()
        .context("Workflow missing source")?;
    let sink = workflow.sink.as_ref().context("Workflow missing sink")?;

    // Determine steps (explicit or auto-planned)
    let steps = if workflow.steps.is_empty() {
        // Auto-plan
        let source_props = source.to_properties();
        let target_pattern = sink.to_pattern();

        let source_cardinality = if source.is_batch() {
            Cardinality::Many
        } else {
            Cardinality::One
        };

        let planner = Planner::new(registry);
        let plan = planner
            .plan(
                &source_props,
                &target_pattern,
                source_cardinality,
                Cardinality::One,
            )
            .context("No conversion path found for workflow")?;

        plan.steps
            .iter()
            .map(|s| s.converter_id.clone())
            .collect::<Vec<_>>()
    } else {
        workflow.steps.iter().map(|s| s.converter.clone()).collect()
    };

    // Get input file path
    let input_path = match source {
        Source::File { path } => PathBuf::from(path),
        Source::Glob { .. } => bail!("Glob sources not yet implemented"),
        Source::Properties { .. } => bail!("Properties-only source cannot be executed"),
    };

    // Get output file path
    let output_path = match sink {
        Sink::File { path } => PathBuf::from(path),
        Sink::Directory { .. } => bail!("Directory sinks not yet implemented"),
        Sink::Properties { .. } => bail!("Properties-only sink cannot be executed"),
    };

    // Read input
    let input_data = std::fs::read(&input_path).context("Failed to read input file")?;

    // Execute steps
    let mut current_data = input_data;
    let mut current_props = source.to_properties();

    println!("Running workflow: {}", workflow_path.display());
    println!("  {} -> {}", input_path.display(), output_path.display());
    println!();

    for converter_id in &steps {
        let converter = registry
            .get(converter_id)
            .context(format!("Converter not found: {}", converter_id))?;

        println!("  Running: {}", converter_id);

        let result = converter
            .convert(&current_data, &current_props)
            .map_err(|e| anyhow::anyhow!("Conversion failed: {}", e))?;

        match result {
            ConvertOutput::Single(data, props) => {
                current_data = data;
                current_props = props;
            }
            ConvertOutput::Multiple(_) => {
                bail!("Unexpected multiple outputs from converter");
            }
        }
    }

    // Write output
    std::fs::write(&output_path, &current_data).context("Failed to write output file")?;

    println!();
    println!(
        "Completed: {} ({} bytes)",
        output_path.display(),
        current_data.len()
    );

    Ok(())
}

/// Check if a path looks like a workflow file.
fn is_workflow_file(path: &str) -> bool {
    // Check if file exists and has workflow-like structure
    // For now, just check extension isn't a known data format
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(
        ext.to_lowercase().as_str(),
        "yaml" | "yml" | "toml" | "json"
    ) && std::path::Path::new(path).exists()
}

fn cmd_convert(
    registry: &Registry,
    input: &PathBuf,
    output: &PathBuf,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    // Detect formats
    let source_format = from
        .or_else(|| detect_format(&input.to_string_lossy()))
        .context("Could not detect source format. Use --from to specify.")?;

    let target_format = to
        .or_else(|| detect_format(&output.to_string_lossy()))
        .context("Could not detect target format. Use --to to specify.")?;

    // Read input
    let input_data = std::fs::read(input).context("Failed to read input file")?;

    // Plan conversion
    let source_props = Properties::new().with("format", source_format.as_str());
    let target_pattern = PropertyPattern::new().eq("format", target_format.as_str());

    let planner = Planner::new(registry);
    let plan = planner
        .plan(
            &source_props,
            &target_pattern,
            Cardinality::One,
            Cardinality::One,
        )
        .context("No conversion path found")?;

    // Execute plan
    let mut current_data = input_data;
    let mut current_props = source_props;

    for step in &plan.steps {
        let converter = registry
            .get(&step.converter_id)
            .context(format!("Converter not found: {}", step.converter_id))?;

        let result = converter
            .convert(&current_data, &current_props)
            .map_err(|e| anyhow::anyhow!("Conversion failed: {}", e))?;

        match result {
            ConvertOutput::Single(data, props) => {
                current_data = data;
                current_props = props;
            }
            ConvertOutput::Multiple(_) => {
                bail!("Unexpected multiple outputs from converter");
            }
        }
    }

    // Write output
    std::fs::write(output, &current_data).context("Failed to write output file")?;

    println!(
        "Converted {} -> {} ({} bytes)",
        input.display(),
        output.display(),
        current_data.len()
    );

    Ok(())
}

/// Detect format from file extension.
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
        _ => None,
    }
}
