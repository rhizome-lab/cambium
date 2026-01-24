# Workflow API

Design for programmatic workflow building, introspection, and serialization.

## Motivation

Agents need to:
1. Build conversion pipelines step by step
2. Inspect what will happen before executing
3. Serialize workflows for replay/sharing/caching
4. Load and modify existing workflows

## Core Concepts

### Workflow

A workflow is a sequence of steps that transform data from source to sink.

```rust
let workflow = Workflow::new()
    .preset("balanced")
    .source("sprites/*.png")
    .pipe("regex-extract", [("pattern", r"sprite_(?<id>\d+)_(?<frame>\d+)")])
    .pipe("spritesheet-pack", [("quality", 90)])
    .sink("output/sprites.png")
    .build()?;
```

### Plan (Introspection)

Before executing, inspect what will happen:

```rust
let plan = workflow.plan()?;

println!("Steps: {:?}", plan.steps);
println!("Required options: {:?}", plan.required_options);
println!("Optional: {:?}", plan.optional_options);
println!("Missing tools: {:?}", plan.missing_tools);
println!("Estimated: {:?}", plan.estimates);  // file count, size, etc.
```

### Incomplete Workflows

Workflows can be incomplete. `plan()` suggests missing pieces:

```rust
// Incomplete: only source and sink
let workflow = Workflow::new()
    .source("sprites/*.png")
    .sink("spritesheet.png")
    .build()?;

let plan = workflow.plan()?;
// plan.suggested_steps = ["regex-extract", "spritesheet-pack"]
// plan.required_options = [("regex-extract.pattern", "string")]
```

No separate `suggest` command - `plan` on incomplete workflow IS suggestion.

### Serialization

Workflows serialize to any supported format:

```rust
// Serialize
let yaml = workflow.to_format(Format::Yaml)?;
let toml = workflow.to_format(Format::Toml)?;
let json = workflow.to_format(Format::Json)?;

// Deserialize
let workflow = Workflow::from_bytes(&yaml, Format::Yaml)?;

// Auto-detect format
let workflow = Workflow::from_file("workflow.yaml")?;
```

Workflow files are just data - paraphase eats its own dogfood:
```bash
paraphase convert workflow.json workflow.yaml
```

## Workflow File Format

Format-agnostic. Example in YAML:

```yaml
# workflow.yaml
preset: balanced

source:
  glob: "sprites/*.png"

steps:
  - converter: regex-extract
    options:
      pattern: "sprite_(?<id>\\d+)_(?<frame>\\d+)"

  - converter: spritesheet-pack
    options:
      quality: 90
      padding: 2

sink:
  path: "output/sprites.png"
```

Equivalent TOML:

```toml
# workflow.toml
preset = "balanced"

[source]
glob = "sprites/*.png"

[[steps]]
converter = "regex-extract"
options = { pattern = "sprite_(?<id>\\d+)_(?<frame>\\d+)" }

[[steps]]
converter = "spritesheet-pack"
options = { quality = 90, padding = 2 }

[sink]
path = "output/sprites.png"
```

## CLI Integration

```bash
# Plan from CLI args (incomplete - suggests pipeline)
paraphase plan --from sprites/*.png --to spritesheet.png

# Plan from workflow file
paraphase plan workflow.yaml

# Plan incomplete workflow file (suggests missing pieces)
paraphase plan partial-workflow.yaml

# Execute workflow
paraphase run workflow.yaml

# Execute with overrides
paraphase run workflow.yaml --quality 95 --preset lossless
```

## Library API

```rust
use paraphase::{Workflow, Format, Options};

// Build programmatically
let workflow = Workflow::new()
    .preset("balanced")
    .source("input/*.png")
    .pipe("png-to-webp", [("quality", 80)])
    .sink("output/")
    .build()?;

// Introspect
let plan = workflow.plan()?;
if !plan.missing_tools.is_empty() {
    eprintln!("Missing: {:?}", plan.missing_tools);
    return Err(...);
}

// Execute
let results = workflow.execute()?;
for result in results {
    println!("Created: {} ({:?})", result.path, result.properties);
}

// Serialize for later
std::fs::write("workflow.yaml", workflow.to_format(Format::Yaml)?)?;
```

## Presets in Workflows

Presets provide base defaults. Steps can override:

```yaml
preset: balanced  # quality=80, compression=lossy, etc.

steps:
  - converter: png-to-webp
    options:
      quality: 95  # overrides preset's quality=80
```

Programmatic:
```rust
Workflow::new()
    .preset("balanced")
    .pipe("png-to-webp", [("quality", 95)])  // override
```

## Pattern Extraction (Plugin)

Pattern extraction is a plugin, not core. Uses regex:

```yaml
steps:
  - converter: regex-extract  # plugin
    options:
      pattern: "sprite_(?<id>\\d+)_(?<frame>\\d+)"
```

This enriches properties:
- Input: `{path: "sprite_001_002.png"}`
- Output: `{path: "sprite_001_002.png", id: "001", frame: "002"}`

Multiple pattern plugins can coexist. Regex is the standard one.

## Cardinality

Steps can be 1→1, 1→N, N→1, or N→M:

```yaml
steps:
  # N→N: each file gets properties extracted
  - converter: regex-extract

  # N→1: all files become one spritesheet
  - converter: spritesheet-pack
```

Cardinality is declared by the converter, not the workflow. Orchestration handles batching.

## Open Questions

See [open-questions.md](./open-questions.md) for:
- How orchestration collects/passes batches to N→1 converters
- Whether "canonical" output flag is needed for 1→N
- Exact workflow file schema
