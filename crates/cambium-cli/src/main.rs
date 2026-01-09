//! Cambium CLI - type-driven data transformation

use anyhow::{Context, Result, bail};
use cambium::{
    Cardinality, ConvertOutput, Planner, Properties, PropertiesExt, PropertyPattern, Registry,
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
        /// Input file or format
        input: String,
        /// Output file or format
        output: String,
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
        } => cmd_plan(&registry, &input, &output, from, to),
        Commands::Convert {
            input,
            output,
            from,
            to,
        } => cmd_convert(&registry, &input, &output, from, to),
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
    output: &str,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    let source_format = from
        .or_else(|| detect_format(input))
        .context("Could not detect source format. Use --from to specify.")?;

    let target_format = to
        .or_else(|| detect_format(output))
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
