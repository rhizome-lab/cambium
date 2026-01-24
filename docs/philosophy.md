# Philosophy

Core design principles for Paraphrase.

## Paraphrase is a Pipeline Orchestrator

**Founding use case:** Game asset conversion - textures, meshes, audio, configs all need processing through diverse tools with inconsistent interfaces.

**Core job:** Unification. One vocabulary, one interface, many backends.

## The Agent Knowledge Gap

The deeper motivation: **agents suck at toolchains.**

When an AI agent (like moss) needs to convert data, it faces:
- "Do I run blender? gltf-pipeline? meshoptimizer?"
- "What flags? Is it installed? Which version?"
- Hallucinated flags, wrong CLI versions, failed optimizations

**Paraphrase solves this by being a route planner, not a task runner.**

```bash
# Task runner (make/just): agent must know the recipe
blender --background --python export.py -- input.blend output.glb
gltf-pipeline -i output.glb -o optimized.glb --draco.compressionLevel 7

# Paraphrase: agent only knows source and destination
paraphase convert model.blend optimized.glb --optimize
```

The agent says "I have X, I need Y" - paraphase finds the path through the graph.

**Why not existing tools?**

| Tool | Approach | Gap |
|------|----------|-----|
| Make | File-based, mtime-driven | You write the recipes |
| Just | Task runner | You write the recipes manually |
| Nix | Content-addressed, reproducible | Heavyweight, config-heavy |
| Paraphrase | Type-driven route planning | Agent just declares intent |

**Scope test:** If the transformation is "agent shouldn't need to know the toolchain," it's in scope. If it requires business logic or architectural decisions, it's out.

## Plan → Execute (Agent-Friendly Interface)

Conversions are two-phase:

**Phase 1: Plan** - Find path, surface required decisions.

```bash
paraphase plan --from sprites/*.png --to spritesheet.png

# Output:
# Suggested path: glob → regex-extract → spritesheet-pack
# Required:
#   regex-extract.pattern: string (regex with named groups)
# Optional:
#   spritesheet-pack.quality: 0-100 (default: 80)
#   spritesheet-pack.padding: int (default: 2)
# Presets: --preset lossless | balanced | crush
# Tools: all available ✓
```

**Phase 2: Execute** - Provide options, run the path.

```bash
paraphase convert sprites/*.png spritesheet.png \
    --pattern "sprite_(?<id>\d+)_(?<frame>\d+)" \
    --preset balanced
```

Incomplete plans = suggestions. No separate `suggest` command.

```bash
# Incomplete: only source and sink
paraphase plan --from input.png --to output.webp
# Paraphrase suggests pipeline + shows what options are available

# Complete: shows exact execution plan
paraphase plan workflow.yaml
```

## Normalized Options

Users learn ONE vocabulary. Paraphrase maps to tool-specific flags:

```bash
# Same --quality flag everywhere
paraphase convert image.png image.webp --quality 80   # → cwebp -q 80
paraphase convert video.mp4 video.webm --quality 80   # → ffmpeg -crf 23
paraphase convert model.glb model.glb --quality 80    # → draco level 7
```

Agent doesn't need to know that quality=80 means different flags for different tools.

## Presets

Declarative option bundles for common scenarios:

```toml
# presets.toml
[lossless]
quality = 100
compression = "lossless"

[balanced]
quality = 80
compression = "lossy"

[crush]
quality = 60
strip_metadata = true
```

```bash
paraphase convert image.png image.webp --preset crush
paraphase convert image.png image.webp --preset balanced --quality 90  # override
```

## Property Bags, Not Types

*See [ADR-0003](./architecture-decisions.md#adr-0003-property-bags-as-type-system)*

Data is described by property bags, not hierarchical types:

```
{format: "png", width: 1024, height: 768, colorspace: "srgb"}
```

Conversion = property transformation. Format is just another property.

```
{format: "png", width: 4096} → {format: "webp", width: 1024}
```

Same model handles format change, resize, transcode, etc.

**Property naming:** Flat by default, namespace only when semantics differ.
- `width`, `height`, `format` - universal
- `image.compression` vs `archive.compression` - different meanings

## Workflows

Workflows are serializable pipelines:

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
sink:
  path: "output/sprites.png"
```

Format-agnostic: YAML, TOML, JSON - paraphase eats its own dogfood.

```bash
paraphase convert workflow.json workflow.yaml  # convert workflow files too
```

Agents can build workflows programmatically:

```rust
Workflow::new()
    .preset("balanced")
    .source("sprites/*.png")
    .pipe("regex-extract", [("pattern", r"sprite_(?<id>\d+)_(?<frame>\d+)")])
    .pipe("spritesheet-pack", [("quality", 90)])
    .sink("output/sprites.png")
    .build()?
```

## N→M Cardinality

Not all conversions are 1→1:

| Pattern | Example |
|---------|---------|
| 1→1 | png → webp |
| 1→N | video → frames |
| N→1 | frames → video, files → manifest |
| N→M | batch tree conversion |

Converters declare their cardinality. Orchestration handles batching.

No special cases: sidecars, manifests, spritesheets are all just N→M conversions.

## Pattern Extraction (Plugin)

Structured filename parsing is a plugin, not core. Uses regex:

```bash
paraphase convert "sprites/*.png" spritesheet.png \
    --pattern "sprite_(?<id>\d+)_(?<frame>\d+)"
```

The `regex-extract` plugin parses filenames, enriches properties:
- Input: `{path: "sprite_001_002.png"}`
- Output: `{path: "sprite_001_002.png", id: "001", frame: "002"}`

Why regex: agents know regex (even if imperfectly), no new DSL to learn.

## Plugins, Not Monolith

Unlike pandoc/ffmpeg (which bundle everything), Paraphrase is:
- **Core**: property bags, graph traversal, workflow orchestration, CLI
- **Plugins**: converters, inspectors, pattern extractors

```bash
paraphase plugin add paraphase-images   # png, jpg, webp, etc.
paraphase plugin add paraphase-ffmpeg   # video/audio via ffmpeg
paraphase plugin add paraphase-regex    # pattern extraction
```

Plugins are C ABI dynamic libraries. See [ADR-0001](./architecture-decisions.md#adr-0001-plugin-format---c-abi-dynamic-libraries).

## Library-First

Paraphrase is a library with a CLI wrapper, not vice versa.

```rust
use paraphase::{Registry, Workflow};

let registry = Registry::with_default_plugins()?;
let plan = registry.plan(&from_props, &to_props)?;
let result = registry.execute(&plan, &input, &options)?;
```

See [ADR-0002](./architecture-decisions.md#adr-0002-library-first-design).

## No Special Cases

Design principle: if something feels like a special case, generalize it.

- "Sidecars" → just 1→N conversion
- "Manifests" → just N→1 conversion
- "Presets" → just option bundles
- "Pattern extraction" → just a property-enriching converter
- "Suggest" → just `plan` on incomplete workflow

One model, many uses.

## Prior Art & Inspiration

Tools that informed Paraphrase's design:

| Tool | What It Does | What We Take |
|------|--------------|--------------|
| **[CyberChef](https://github.com/gchq/CyberChef)** | "Cyber Swiss Army Knife" - browser-based encoding, compression, hashing, data analysis | Recipe-based pipelines, comprehensive format coverage, "bake" metaphor |
| **[Pandoc](https://pandoc.org)** | Universal document converter | Format graph traversal, intermediate representation idea |
| **[FFmpeg](https://ffmpeg.org)** | Media transcoding | Filter graphs, format negotiation |
| **[ImageMagick](https://imagemagick.org)** | Image manipulation | Batch processing, format detection |
| **[jq](https://jqlang.github.io/jq/)** | JSON processor | Streaming, composable transformations |

**Key difference:** These tools are format-specific or monolithic. Paraphrase is a **unified orchestrator** - one interface that routes to the right tool for each conversion.
