# Quickstart

Get up and running with Paraphrase in minutes.

## Installation

```bash
cargo install paraphase-cli
```

This installs Paraphrase with default converters (common serde and image formats).

### Minimal or Custom Builds

```bash
# Only JSON and PNG
cargo install paraphase-cli --no-default-features \
  --features paraphase-serde/json,paraphase-image/png

# All serde formats, no image support
cargo install paraphase-cli --no-default-features --features serde-all

# Everything
cargo install paraphase-cli --features all
```

## Basic Usage

### Convert Files

Paraphrase auto-detects formats from file extensions:

```bash
# Config formats
paraphase convert config.json config.yaml
paraphase convert settings.yaml settings.toml

# Image formats
paraphase convert photo.png photo.webp
paraphase convert image.jpg image.gif
```

Override detection with explicit formats:

```bash
paraphase convert data.bin output.json --from msgpack --to json
```

### Plan Conversions

See what Paraphrase will do without executing:

```bash
paraphase plan input.json output.toml
```

Output:
```
Planning: json -> toml

Steps:
  1. serde.json-to-toml (default -> default)

Total cost: 1
```

### List Available Converters

```bash
paraphase list
```

Shows all registered converters with their input/output properties.

## Workflows

Workflows define pipelines in YAML, TOML, or JSON.

### Simple Workflow

```yaml
# workflow.yaml
source:
  path: input.json
sink:
  path: output.yaml
```

Run with auto-planning:

```bash
paraphase run workflow.yaml
```

Paraphrase finds the conversion path automatically.

### Explicit Steps

For precise control, specify converters:

```yaml
source:
  path: input.json
steps:
  - converter: serde.json-to-yaml
sink:
  path: output.yaml
```

## Library Usage

```rust
use paraphase::{Registry, Planner, Properties, PropertyPattern, Cardinality, PropertiesExt};

fn main() -> anyhow::Result<()> {
    // Create registry and register converters
    let mut registry = Registry::new();
    paraphase_serde::register_all(&mut registry);
    paraphase_image::register_all(&mut registry);

    // Plan a conversion
    let planner = Planner::new(&registry);
    let source = Properties::new().with("format", "json");
    let target = PropertyPattern::new().eq("format", "yaml");

    if let Some(plan) = planner.plan(&source, &target, Cardinality::One, Cardinality::One) {
        println!("Found path with {} steps:", plan.steps.len());
        for step in &plan.steps {
            println!("  {}", step.converter_id);
        }
    }

    Ok(())
}
```

## Next Steps

- [Formats Reference](./formats) - All supported formats
- [Workflow API](./workflow-api) - Full workflow specification
- [Philosophy](./philosophy) - Design principles
- [Use Cases](./use-cases) - Example scenarios
