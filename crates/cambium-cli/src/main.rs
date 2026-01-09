//! Cambium CLI - type-driven data transformation

use anyhow::{Context, Result, bail};
use cambium::{
    BoundedExecutor, Cardinality, ConvertOutput, ExecutionContext, Executor, NamedInput, Plan,
    Planner, Properties, PropertiesExt, PropertyPattern, Registry, SimpleExecutor, Sink, Source,
    Workflow,
};
use clap::{Parser, Subcommand};
use indexmap::IndexMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Options for image/video transforms passed to converters.
#[derive(Default)]
struct ConvertOptions {
    max_width: Option<u32>,
    max_height: Option<u32>,
    scale: Option<f64>,
    aspect: Option<String>,
    gravity: String,
    // Watermark options
    watermark: Option<PathBuf>,
    watermark_position: String,
    watermark_opacity: f64,
    watermark_margin: u32,
    // Video options
    quality: Option<String>,
}

#[derive(Parser)]
#[command(name = "cambium")]
#[command(about = "Type-driven data transformation", long_about = None)]
struct Cli {
    /// Memory limit in bytes (e.g., 100000000 for 100MB). Fails fast if exceeded.
    #[arg(long, global = true)]
    memory_limit: Option<usize>,

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

        // Image transform options
        /// Maximum width (fit within, preserves aspect ratio)
        #[arg(long)]
        max_width: Option<u32>,
        /// Maximum height (fit within, preserves aspect ratio)
        #[arg(long)]
        max_height: Option<u32>,
        /// Scale factor (e.g., 0.5 for half size)
        #[arg(long)]
        scale: Option<f64>,
        /// Target aspect ratio (e.g., "16:9" or "1.778")
        #[arg(long)]
        aspect: Option<String>,
        /// Gravity/anchor for cropping (center, top, bottom, left, right, top-left, etc.)
        #[arg(long, default_value = "center")]
        gravity: String,

        // Watermark options
        /// Watermark image file to composite onto the image
        #[arg(long)]
        watermark: Option<PathBuf>,
        /// Watermark position (bottom-right, top-left, center, etc.)
        #[arg(long, default_value = "bottom-right")]
        watermark_position: String,
        /// Watermark opacity (0.0 to 1.0)
        #[arg(long, default_value = "0.5")]
        watermark_opacity: f64,
        /// Watermark margin from edge in pixels
        #[arg(long, default_value = "10")]
        watermark_margin: u32,

        // Video options
        /// Video quality preset (low, medium, high, lossless)
        #[arg(long)]
        quality: Option<String>,
    },

    /// Run a workflow file
    Run {
        /// Workflow file (YAML, TOML, or JSON)
        workflow: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create registry with enabled converters
    let mut registry = Registry::new();

    #[cfg(feature = "serde")]
    cambium_serde::register_all(&mut registry);

    #[cfg(feature = "image")]
    cambium_image::register_all(&mut registry);

    #[cfg(feature = "video")]
    cambium_video::register_all(&mut registry);

    #[cfg(feature = "audio")]
    cambium_audio::register_all(&mut registry);

    let memory_limit = cli.memory_limit;

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
            max_width,
            max_height,
            scale,
            aspect,
            gravity,
            watermark,
            watermark_position,
            watermark_opacity,
            watermark_margin,
            quality,
        } => cmd_convert(
            &registry,
            &input,
            &output,
            from,
            to,
            ConvertOptions {
                max_width,
                max_height,
                scale,
                aspect,
                gravity,
                watermark,
                watermark_position,
                watermark_opacity,
                watermark_margin,
                quality,
            },
            memory_limit,
        ),
        Commands::Run { workflow } => cmd_run(&registry, &workflow, memory_limit),
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

fn cmd_run(
    registry: &Registry,
    workflow_path: &PathBuf,
    memory_limit: Option<usize>,
) -> Result<()> {
    let data = std::fs::read(workflow_path).context("Failed to read workflow file")?;
    let workflow = Workflow::from_bytes(&data, Some(&workflow_path.to_string_lossy()))
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow: {}", e))?;

    // Get source and sink
    let source = workflow
        .source
        .as_ref()
        .context("Workflow missing source")?;
    let sink = workflow.sink.as_ref().context("Workflow missing sink")?;

    // Determine plan (explicit steps or auto-planned)
    let plan = if workflow.steps.is_empty() {
        // Auto-plan
        let source_props = source.to_properties();
        let target_pattern = sink.to_pattern();

        let source_cardinality = if source.is_batch() {
            Cardinality::Many
        } else {
            Cardinality::One
        };

        let planner = Planner::new(registry);
        planner
            .plan(
                &source_props,
                &target_pattern,
                source_cardinality,
                Cardinality::One,
            )
            .context("No conversion path found for workflow")?
    } else {
        // Build plan from explicit steps
        Plan {
            steps: workflow
                .steps
                .iter()
                .map(|s| cambium::PlanStep {
                    converter_id: s.converter.clone(),
                    input_port: "in".into(),
                    output_port: "out".into(),
                    output_properties: Properties::new(),
                })
                .collect(),
            cost: workflow.steps.len() as f64,
        }
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
    let input_props = source.to_properties();

    println!("Running workflow: {}", workflow_path.display());
    println!("  {} -> {}", input_path.display(), output_path.display());
    println!();

    for step in &plan.steps {
        println!("  Running: {}", step.converter_id);
    }

    // Execute using appropriate executor
    let mut ctx = ExecutionContext::new(Arc::new(registry.clone()));
    if let Some(limit) = memory_limit {
        ctx = ctx.with_memory_limit(limit);
    }

    let result = if memory_limit.is_some() {
        BoundedExecutor::new().execute(&ctx, &plan, input_data, input_props)
    } else {
        SimpleExecutor::new().execute(&ctx, &plan, input_data, input_props)
    }
    .map_err(|e| anyhow::anyhow!("Execution failed: {}", e))?;

    // Write output
    std::fs::write(&output_path, &result.data).context("Failed to write output file")?;

    println!();
    println!(
        "Completed: {} ({} bytes, {:?})",
        output_path.display(),
        result.data.len(),
        result.stats.duration
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
    opts: ConvertOptions,
    memory_limit: Option<usize>,
) -> Result<()> {
    // Detect formats
    let source_format = from
        .or_else(|| detect_format(&input.to_string_lossy()))
        .context("Could not detect source format. Use --from to specify.")?;

    let target_format = to
        .or_else(|| detect_format(&output.to_string_lossy()))
        .context("Could not detect target format. Use --to to specify.")?;

    // Read input
    let mut current_data = std::fs::read(input).context("Failed to read input file")?;
    let mut current_props = Properties::new().with("format", source_format.as_str());

    // Apply image transforms if any options are set
    let needs_resize =
        opts.max_width.is_some() || opts.max_height.is_some() || opts.scale.is_some();
    let needs_crop = opts.aspect.is_some();

    if needs_resize || needs_crop {
        // Get image dimensions first (we need them for the converters)
        #[cfg(feature = "image")]
        {
            // Decode to get dimensions
            let img = image::load_from_memory(&current_data)
                .context("Failed to decode image for transform")?;
            current_props.insert("width".into(), (img.width() as i64).into());
            current_props.insert("height".into(), (img.height() as i64).into());
        }

        // Apply aspect crop first (before resize)
        if let Some(ref aspect) = opts.aspect {
            current_props.insert("aspect".into(), aspect.clone().into());
            current_props.insert("gravity".into(), opts.gravity.clone().into());

            let crop_converter = registry
                .get("image.crop-aspect")
                .context("Crop converter not available")?;

            let result = crop_converter
                .convert(&current_data, &current_props)
                .map_err(|e| anyhow::anyhow!("Crop failed: {}", e))?;

            match result {
                ConvertOutput::Single(data, props) => {
                    current_data = data;
                    current_props = props;
                }
                _ => bail!("Unexpected output from crop converter"),
            }

            // Remove crop-specific props
            current_props.shift_remove("aspect");
            current_props.shift_remove("gravity");
        }

        // Apply resize
        if needs_resize {
            if let Some(mw) = opts.max_width {
                current_props.insert("max_width".into(), (mw as i64).into());
            }
            if let Some(mh) = opts.max_height {
                current_props.insert("max_height".into(), (mh as i64).into());
            }
            if let Some(s) = opts.scale {
                current_props.insert("scale".into(), s.into());
            }

            let resize_converter = registry
                .get("image.resize")
                .context("Resize converter not available")?;

            let result = resize_converter
                .convert(&current_data, &current_props)
                .map_err(|e| anyhow::anyhow!("Resize failed: {}", e))?;

            match result {
                ConvertOutput::Single(data, props) => {
                    current_data = data;
                    current_props = props;
                }
                _ => bail!("Unexpected output from resize converter"),
            }

            // Remove resize-specific props
            current_props.shift_remove("max_width");
            current_props.shift_remove("max_height");
            current_props.shift_remove("scale");
        }
    }

    // Apply watermark if specified
    if let Some(ref watermark_path) = opts.watermark {
        #[cfg(feature = "image")]
        {
            // Read watermark file
            let watermark_data =
                std::fs::read(watermark_path).context("Failed to read watermark file")?;

            // Get watermark dimensions
            let watermark_img = image::load_from_memory(&watermark_data)
                .context("Failed to decode watermark image")?;
            let mut watermark_props = Properties::new();
            watermark_props.insert("width".into(), (watermark_img.width() as i64).into());
            watermark_props.insert("height".into(), (watermark_img.height() as i64).into());

            // Ensure base image has dimensions
            if current_props.get("width").is_none() {
                let img = image::load_from_memory(&current_data)
                    .context("Failed to decode base image for watermark")?;
                current_props.insert("width".into(), (img.width() as i64).into());
                current_props.insert("height".into(), (img.height() as i64).into());
            }

            // Set watermark options on base image props
            current_props.insert("position".into(), opts.watermark_position.clone().into());
            current_props.insert("opacity".into(), opts.watermark_opacity.into());
            current_props.insert("margin".into(), (opts.watermark_margin as i64).into());

            // Build multi-input map
            let mut inputs = IndexMap::new();
            inputs.insert(
                "image".to_string(),
                NamedInput {
                    data: &current_data,
                    props: &current_props,
                },
            );
            inputs.insert(
                "watermark".to_string(),
                NamedInput {
                    data: &watermark_data,
                    props: &watermark_props,
                },
            );

            let watermark_converter = registry
                .get("image.watermark")
                .context("Watermark converter not available")?;

            let result = watermark_converter
                .convert_multi(&inputs)
                .map_err(|e| anyhow::anyhow!("Watermark failed: {}", e))?;

            match result {
                ConvertOutput::Single(data, props) => {
                    current_data = data;
                    current_props = props;
                }
                _ => bail!("Unexpected output from watermark converter"),
            }

            // Remove watermark-specific props
            current_props.shift_remove("position");
            current_props.shift_remove("opacity");
            current_props.shift_remove("margin");
        }

        #[cfg(not(feature = "image"))]
        bail!("Watermark requires the 'image' feature");
    }

    // Plan format conversion (if formats differ)
    if source_format != target_format {
        let target_pattern = PropertyPattern::new().eq("format", target_format.as_str());

        let planner = Planner::new(registry);
        let plan = planner
            .plan(
                &current_props,
                &target_pattern,
                Cardinality::One,
                Cardinality::One,
            )
            .context("No conversion path found")?;

        // Execute format conversion plan using appropriate executor
        let mut ctx = ExecutionContext::new(Arc::new(registry.clone()));
        if let Some(limit) = memory_limit {
            ctx = ctx.with_memory_limit(limit);
        }

        let result = if memory_limit.is_some() {
            BoundedExecutor::new().execute(&ctx, &plan, current_data, current_props)
        } else {
            SimpleExecutor::new().execute(&ctx, &plan, current_data, current_props)
        }
        .map_err(|e| anyhow::anyhow!("Conversion failed: {}", e))?;

        current_data = result.data;
        current_props = result.props;
    }

    // Write output
    std::fs::write(output, &current_data).context("Failed to write output file")?;

    // Report what was done
    let has_watermark = opts.watermark.is_some();
    let transform_info = if needs_resize || needs_crop || has_watermark {
        let w = current_props
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let h = current_props
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let wm = if has_watermark { " +watermark" } else { "" };
        format!(" ({}x{}{})", w, h, wm)
    } else {
        String::new()
    };

    println!(
        "Converted {} -> {}{} ({} bytes)",
        input.display(),
        output.display(),
        transform_info,
        current_data.len()
    );

    Ok(())
}

/// Detect format from file extension.
fn detect_format(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext.to_lowercase().as_str() {
        // Serde text formats
        "json" => Some("json".into()),
        "yaml" | "yml" => Some("yaml".into()),
        "toml" => Some("toml".into()),
        "ron" => Some("ron".into()),
        "json5" => Some("json5".into()),
        "xml" => Some("xml".into()),
        "lisp" | "sexp" | "lexpr" => Some("lexpr".into()),
        "csv" => Some("csv".into()),
        // Serde binary formats
        "msgpack" | "mp" => Some("msgpack".into()),
        "cbor" => Some("cbor".into()),
        "bincode" | "bc" => Some("bincode".into()),
        "postcard" | "pc" => Some("postcard".into()),
        "bson" => Some("bson".into()),
        "flexbuf" | "flexbuffers" => Some("flexbuffers".into()),
        "bencode" | "torrent" => Some("bencode".into()),
        "pickle" | "pkl" => Some("pickle".into()),
        "plist" => Some("plist".into()),
        // Image formats
        "png" => Some("png".into()),
        "jpg" | "jpeg" => Some("jpg".into()),
        "webp" => Some("webp".into()),
        "gif" => Some("gif".into()),
        "bmp" => Some("bmp".into()),
        "ico" => Some("ico".into()),
        "tif" | "tiff" => Some("tiff".into()),
        "tga" => Some("tga".into()),
        "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Some("pnm".into()),
        "ff" | "farbfeld" => Some("farbfeld".into()),
        "qoi" => Some("qoi".into()),
        "avif" => Some("avif".into()),
        "exr" => Some("exr".into()),
        "hdr" => Some("hdr".into()),
        // Video formats
        "mp4" | "m4v" => Some("mp4".into()),
        "webm" => Some("webm".into()),
        "mkv" => Some("mkv".into()),
        "avi" => Some("avi".into()),
        "mov" | "qt" => Some("mov".into()),
        // Audio formats
        "wav" | "wave" => Some("wav".into()),
        "flac" => Some("flac".into()),
        "mp3" => Some("mp3".into()),
        "ogg" | "oga" => Some("ogg".into()),
        "aac" | "m4a" => Some("aac".into()),
        _ => None,
    }
}
