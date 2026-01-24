# Architecture Decisions

Record of key technical choices for Paraphrase.

## ADR-0001: Plugin Format - C ABI Dynamic Libraries

**Status:** Accepted

**Context:**

Paraphrase needs a plugin system for converters. Options considered:

| Format | Authoring | Performance | Sandboxing | Distribution |
|--------|-----------|-------------|------------|--------------|
| Rust crates (static) | Rust-only | Native | None | cargo |
| WASM | Any→wasm | Near-native | Yes | .wasm files |
| Executables | Any language | Subprocess overhead | OS-level | PATH |
| C ABI dylibs | Any→C ABI | Native | None | .so/.dylib/.dll |

**Decision:** C ABI dynamic libraries (`.so`/`.dylib`/`.dll`)

**Rationale:**

1. **Rust has no stable ABI** - dynamic loading requires C ABI anyway
2. **Universal authoring** - any language that can produce C-compatible shared libraries works (Rust, C, C++, Zig, Go via cgo)
3. **Native performance** - no subprocess overhead, no interpreter
4. **Proven model** - VST/AU plugins, SQLite extensions, Lua C modules, OBS/GIMP/Blender plugins all use this
5. **Can link libraries directly** - plugins can use libvips, libav*, etc. via FFI without subprocess indirection

**Plugin C API:**

```c
// paraphase_plugin.h

#include <stdint.h>
#include <stddef.h>

#define CAMBIUM_PLUGIN_API_VERSION 1

// Converter metadata
typedef struct {
    const char* id;           // unique identifier, e.g. "serde.json_to_yaml"
    const char* from_type;    // e.g. "json"
    const char* to_type;      // e.g. "yaml"
    uint32_t flags;           // CAMBIUM_FLAG_* bitmask
} ParaphraseConverter;

// Flags
#define CAMBIUM_FLAG_LOSSLESS   (1 << 0)
#define CAMBIUM_FLAG_STREAMING  (1 << 1)

// Plugin exports these symbols:

// Called once on load, returns API version for compatibility check
uint32_t paraphase_plugin_version(void);

// List available converters (caller does NOT free)
const ParaphraseConverter* paraphase_list_converters(size_t* count);

// Perform conversion
// Returns 0 on success, non-zero error code on failure
// On success, *output and *output_len are set (caller must free with paraphase_free)
// options_json may be NULL
int paraphase_convert(
    const char* converter_id,
    const uint8_t* input, size_t input_len,
    uint8_t** output, size_t* output_len,
    const char* options_json
);

// Free memory allocated by paraphase_convert
void paraphase_free(void* ptr);

// Optional: get error message for last failure (may return NULL)
const char* paraphase_last_error(void);
```

**Rust Plugin Authoring:**

`paraphase-plugin` crate provides ergonomic wrapper:

```rust
use paraphase_plugin::prelude::*;

#[paraphase_converter(from = "json", to = "yaml", lossless)]
fn json_to_yaml(input: &[u8], _opts: &Options) -> Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(input)?;
    Ok(serde_yaml::to_vec(&value)?)
}

// Macro generates:
// - #[no_mangle] extern "C" fn paraphase_plugin_version() -> u32
// - #[no_mangle] extern "C" fn paraphase_list_converters(*mut usize) -> *const ParaphraseConverter
// - #[no_mangle] extern "C" fn paraphase_convert(...) -> i32
// - #[no_mangle] extern "C" fn paraphase_free(*mut c_void)
// - #[no_mangle] extern "C" fn paraphase_last_error() -> *const c_char

paraphase_plugin::export![json_to_yaml];
```

**Plugin Discovery:**

Plugins are discovered from (in order):
1. Built-in converters (compiled into paraphase binary)
2. `$CAMBIUM_PLUGIN_PATH` (colon-separated)
3. `~/.paraphase/plugins/*.{so,dylib,dll}`
4. Project-local `./paraphase-plugins/*.{so,dylib,dll}`

Later sources can override earlier ones (project-local wins).

**Loading:**

```rust
// Pseudocode
fn load_plugin(path: &Path) -> Result<Plugin> {
    let lib = libloading::Library::new(path)?;

    let version: Symbol<fn() -> u32> = lib.get(b"paraphase_plugin_version")?;
    if version() != CAMBIUM_PLUGIN_API_VERSION {
        return Err(IncompatibleVersion);
    }

    let list: Symbol<fn(*mut usize) -> *const ParaphraseConverter> =
        lib.get(b"paraphase_list_converters")?;
    // ... register converters
}
```

**Consequences:**

- (+) Native performance, no subprocess overhead
- (+) Plugins can link C libraries (libvips, ffmpeg) directly
- (+) Any language can author plugins
- (-) No sandboxing - plugins run in-process with full trust
- (-) Platform-specific binaries (.so vs .dylib vs .dll)
- (-) ABI stability burden - must version the C API carefully

**Future considerations:**

- WASM plugins could be added later for sandboxed/portable plugins
- Subprocess fallback for tools that only exist as CLIs (e.g., pandoc)

---

## ADR-0002: Library-First Design

**Status:** Accepted

**Context:**

Paraphrase can be designed as either:
1. **Library-first** - Rust crate with CLI as thin wrapper
2. **CLI-first** - Command-line tool that can also be used as library

**Decision:** Library-first

**Rationale:**

1. **Rhizome ecosystem is Rust** - Resin and other tools want direct integration without subprocess overhead
2. **Zero-copy possible** - Library can pass `&[u8]` directly; CLI requires file I/O or pipes
3. **Introspection** - Programmatic access to converter graph (list converters, find paths, query capabilities)
4. **Forces clean design** - Library API must be coherent; CLI can always wrap, but library can't unwrap CLI
5. **CLI is trivial wrapper** - Once library exists, CLI is ~100 lines

**Library API sketch:**

```rust
// paraphase/src/lib.rs

/// Registry of available converters
pub struct Registry { /* ... */ }

impl Registry {
    /// Empty registry
    pub fn new() -> Self;

    /// Load plugins from default locations
    pub fn with_default_plugins() -> Result<Self>;

    /// Load a specific plugin
    pub fn load_plugin(&mut self, path: &Path) -> Result<()>;

    /// Register a converter directly (for built-ins or testing)
    pub fn register<C: Converter>(&mut self, converter: C);

    /// List all registered converters
    pub fn converters(&self) -> impl Iterator<Item = ConverterInfo>;

    /// Find conversion path from source to target type
    pub fn find_path(&self, from: &str, to: &str) -> Option<Vec<ConverterInfo>>;
}

/// Plan a conversion (phase 1)
pub struct Plan {
    path: Vec<ConverterInfo>,
    required_options: Vec<OptionSpec>,
    optional_options: Vec<OptionSpec>,
    missing_tools: Vec<String>,
}

impl Registry {
    /// Plan a conversion without executing
    pub fn plan(&self, from: &str, to: &str) -> Result<Plan>;

    /// Execute a planned conversion (phase 2)
    pub fn execute(&self, plan: &Plan, input: &[u8], options: &Options) -> Result<Vec<u8>>;

    /// Convenience: plan + execute with defaults
    pub fn convert(&self, from: &str, to: &str, input: &[u8], options: &Options) -> Result<Vec<u8>>;
}

/// Option specification surfaced to caller
pub struct OptionSpec {
    name: String,           // e.g. "compression_level"
    description: String,    // human-readable
    typ: OptionType,        // Int { min, max }, Bool, Enum { choices }, etc.
    default: Option<Value>, // sensible default if any
}

/// Converter trait for built-in converters
pub trait Converter: Send + Sync {
    fn info(&self) -> ConverterInfo;

    /// Declare options this converter accepts
    fn options(&self) -> Vec<OptionSpec>;

    fn convert(&self, input: &[u8], options: &Options) -> Result<Vec<u8>>;
}
```

**Presets (declarative defaults):**

Instead of hardcoded defaults, presets are declarative option bundles:

```toml
# presets.toml (shipped with paraphase or user-defined)

[presets.lossless]
description = "Preserve quality, larger files"
quality = 100
compression = "lossless"

[presets.balanced]
description = "Good quality, reasonable size (default)"
quality = 80
compression = "lossy"

[presets.crush]
description = "Minimize size, acceptable quality loss"
quality = 60
compression = "lossy"
strip_metadata = true
```

Usage:
```bash
paraphase convert image.png image.webp --preset crush
paraphase convert video.mp4 video.webp --preset lossless

# Preset + overrides
paraphase convert image.png image.webp --preset balanced --quality 90
```

Presets map normalized options to converter-specific flags:
- `quality=80` → `-q 80` (cwebp), `-crf 23` (ffmpeg), `--draco.compressionLevel 7` (gltf-pipeline)

Converters declare how they interpret normalized options; presets just set those options.

**CLI as wrapper:**

```rust
// paraphase-cli/src/main.rs

use paraphase::{Registry, Options};
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let registry = Registry::with_default_plugins()?;

    match args.command {
        Command::Plan { input, to } => {
            let from = detect_type(&input)?;
            let plan = registry.plan(&from, &to)?;

            println!("Path: {}", plan.path_display());
            println!("\nRequired options:");
            for opt in plan.required_options() {
                println!("  --{}: {} ({})", opt.name, opt.typ, opt.description);
            }
            println!("\nOptional:");
            for opt in plan.optional_options() {
                let default = opt.default.map(|d| format!(" [default: {}]", d)).unwrap_or_default();
                println!("  --{}: {}{}", opt.name, opt.typ, default);
            }
            if !plan.missing_tools().is_empty() {
                println!("\n⚠ Missing tools: {:?}", plan.missing_tools());
            }
        }
        Command::Convert { input, output, from, to, preset, options } => {
            let from = from.or_else(|| detect_type(&input));
            let to = to.or_else(|| detect_type(&output));

            let mut opts = Options::from_preset(preset.as_deref().unwrap_or("balanced"))?;
            opts.merge(&options);  // CLI flags override preset

            let data = std::fs::read(&input)?;
            let result = registry.convert(&from, &to, &data, &opts)?;
            std::fs::write(&output, result)?;
        }
        Command::List => {
            for c in registry.converters() {
                println!("{} -> {}", c.from_type, c.to_type);
            }
        }
    }
    Ok(())
}
```

**Consequences:**

- (+) Resin can use paraphase with zero overhead
- (+) In-memory conversions without temp files
- (+) Testable without spawning processes
- (+) Can introspect and optimize conversion paths
- (-) Rust-only for direct usage (others use CLI)
- (-) Must maintain semver stability for library API

**Crate structure:**

```
crates/
  paraphase/           # library (pub API)
  paraphase-cli/       # binary (thin wrapper)
  paraphase-plugin/    # plugin authoring helpers
```

---

## ADR-0003: Property Bags as Type System

**Status:** Accepted

**Context:**

Paraphrase needs a way to represent "what kind of data is this" for routing conversions. Options considered:

| Model | Example | Expressiveness |
|-------|---------|----------------|
| Flat strings | `"png"`, `"mp4"` | Low - can't express params |
| Hierarchical | `image/png` | Medium - grouping only |
| Type + params | `video[pixfmt=yuv411]` | High - but type is privileged |
| Property bags | `{format: png, width: 1024}` | Highest - uniform |
| Bags + schema | Same + validation | Highest + structure |

**Decision:** Pure property bags. Schemas are optional (plugin).

**Rationale:**

1. **Maximum generality** - Format is just another property, not privileged
2. **Uniform model** - Format change, resize, and transcode are all "property transformations"
3. **Domain-agnostic core** - Core knows nothing about images, video, etc.
4. **Schemas as optional layer** - Validation can be a plugin for those who want it

**Core data model:**

```rust
/// Properties describe data - core doesn't interpret these
pub type Properties = HashMap<String, Value>;

/// Values are JSON-like
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// Converter declares what properties it requires and produces
pub struct ConverterDecl {
    /// Pattern that input properties must match
    pub requires: PropertyPattern,

    /// Properties this converter changes/adds
    pub produces: PropertyPattern,

    /// Properties explicitly removed (rare)
    pub removes: Vec<String>,

    // Everything else is preserved
}

/// Pattern for matching properties
pub enum PropertyPattern {
    /// Exact value: {format: "png"}
    Exact(HashMap<String, Value>),

    /// Predicate: {width: Gt(2048)}
    Predicate(HashMap<String, Predicate>),

    /// Any value present: {format: Any}
    Exists(Vec<String>),
}
```

**Search algorithm:** State-space planning

```rust
/// Find sequence of converters from current to goal properties
pub fn plan(
    current: &Properties,
    goal: &Properties,
    converters: &[ConverterDecl],
) -> Option<Vec<ConverterId>> {
    // A* search where:
    // - State = current properties
    // - Actions = applicable converters
    // - Goal test = current ⊇ goal (superset match)
    // - Heuristic = |properties differing from goal|
}
```

**Superset matching:** Goal `{format: webp, width: 1024}` is satisfied by
`{format: webp, width: 1024, colorspace: srgb}` - extra properties are fine.

**Conventions (not enforced, for interop):**

```
image.*     - image properties (format, width, height, colorspace, ...)
video.*     - video properties (container, codec, pixfmt, framerate, ...)
audio.*     - audio properties (format, samplerate, channels, bitrate, ...)
document.*  - document properties (format, pages, ...)
archive.*   - archive properties (format, compression, ...)

Or flat: format, width, height, ... (simpler, but collision risk)
```

**Schemas as plugin:**

```rust
// Optional: paraphase-schemas plugin
pub struct Schema {
    pub domain: String,
    pub properties: Vec<PropertyDef>,
}

pub struct PropertyDef {
    pub name: String,
    pub typ: PropertyType,  // Int { min, max }, Enum { choices }, etc.
    pub required: bool,
}

// Plugin validates properties against schemas
impl Plugin for SchemaValidator {
    fn validate(&self, props: &Properties) -> Result<(), SchemaError>;
}
```

Multiple plugins can extend schemas. On conflict (incompatible constraints), error at plugin load time.

**Consequences:**

- (+) Core is maximally general and domain-agnostic
- (+) Same model handles format change, resize, transcode, etc.
- (+) Plugins can add any properties without core changes
- (+) Schemas are opt-in, not required
- (-) No built-in validation without schema plugin
- (-) Conventions need documentation and discipline
- (-) Search space potentially large (mitigated by heuristics)

**Examples:**

```rust
// PNG to WebP
requires: {format: Exact("png")}
produces: {format: "webp"}

// Resize (any image)
requires: {width: Exists, height: Exists}
produces: {width: <from_options>, height: <from_options>}

// yuv411 to yuv420p (specific pixel format)
requires: {pixfmt: Exact("yuv411")}
produces: {pixfmt: "yuv420p"}

// PDF to PNG (cross-domain)
requires: {format: Exact("pdf")}
produces: {format: "png", width: <from_options>, height: <from_options>}
removes: [pages, ...]  // PDF-specific props don't apply to image
```

---

## ADR-0004: Named Ports with Per-Port Cardinality

**Status:** Accepted

**Context:**

Paraphrase needs to handle N→M conversions (1→1, 1→N, N→1, N→M). Early designs tried to encode cardinality in PropertyPattern itself (via `$each` syntax or separate Cardinality enum), which felt awkward and coupled concerns.

Prior art: [ComfyUI](https://github.com/comfyanonymous/ComfyUI) uses named input/output ports with explicit types, handling multiple outputs and list/batch processing cleanly.

**Decision:** Named ports with per-port cardinality.

```rust
struct ConverterDecl {
    inputs: HashMap<String, PortDecl>,
    outputs: HashMap<String, PortDecl>,
    costs: Properties,
}

struct PortDecl {
    pattern: PropertyPattern,
    list: bool,  // true = expects/produces list
}
```

**Rationale:**

1. **Separation of concerns** - Property patterns describe "what kind of data", cardinality describes "how many". Orthogonal.
2. **Explicit wiring** - Multi-output converters have named ports; workflows reference them unambiguously (`step.output_name`).
3. **Composable** - Multiple inputs, multiple outputs, lists - all combinations work uniformly.
4. **Planning stays simple** - Planning infers cardinality from source/target, tracks through graph using transformation rules.

**Examples:**

```rust
// 1→1 (most common)
inputs: { "in": PortDecl { pattern: {format: "png"}, list: false } }
outputs: { "out": PortDecl { pattern: {format: "webp"}, list: false } }

// N→1 aggregator (frames → video)
inputs: { "frames": PortDecl { pattern: {format: "png"}, list: true } }
outputs: { "video": PortDecl { pattern: {format: "mp4"}, list: false } }

// 1→N expander (video → frames)
inputs: { "video": PortDecl { pattern: {format: "mp4"}, list: false } }
outputs: { "frames": PortDecl { pattern: {format: "png"}, list: true } }

// Multiple outputs (image + sidecar)
inputs: { "in": PortDecl { pattern: {format: "png"}, list: false } }
outputs: {
    "image": PortDecl { pattern: {format: "webp"}, list: false },
    "sidecar": PortDecl { pattern: {format: "json"}, list: false }
}

// Multiple inputs (compositing)
inputs: {
    "base": PortDecl { pattern: {format: "png"}, list: false },
    "overlay": PortDecl { pattern: {format: "png"}, list: false }
}
outputs: { "out": PortDecl { pattern: {format: "png"}, list: false } }
```

**Cardinality transformation rules:**

| Input `list` | Output `list` | Behavior |
|--------------|---------------|----------|
| false | false | 1→1, auto-maps over batch |
| true | false | N→1, aggregation |
| false | true | 1→N, expansion |
| true | true | N→M, transform |

**Planning:**

Planning infers cardinality from the request:
- `bob_*.png` (glob) → N items
- `bob.gif` (single path) → 1 item

Planner searches for path where cardinality transforms correctly from source to target.

**Workflow wiring:**

```yaml
steps:
  - id: convert
    converter: with-sidecar

  - id: optimize
    converter: webp-optimize
    input: convert.image    # reference specific port

  - id: validate
    converter: json-schema
    input: convert.sidecar  # reference other port
```

**Consequences:**

- (+) Clean separation: patterns vs cardinality
- (+) Multi-output handled naturally via named ports
- (+) Planning stays property-based, cardinality is inferred
- (+) Uniform model for all N→M cases
- (-) Slightly more verbose converter declarations
- (-) Workflow wiring needs port references for multi-output

---

## ADR-0005: Conversion vs. Editing Scope Boundary

**Status:** Accepted

**Context:**

As Paraphrase adds more transformations (resize, crop, watermark), the question arises: when does a "conversion tool" become an "asset editor"? Without a clear boundary, scope creep leads to reimplementing Photoshop.

**Decision:** Paraphrase handles transformations expressible as **normalized options or property constraints**. Operations requiring **pixel-level precision or creative judgment** are out of scope.

**The test:** Can an agent express the operation without looking at the specific content?

**Rationale:**

From the philosophy doc: "Agent says 'I have X, I need Y' - paraphase finds the path." The agent shouldn't need to make creative decisions or specify exact coordinates.

| Operation | Agent expression | In scope? |
|-----------|------------------|-----------|
| Format change | `format=webp` | ✓ |
| Fit within bounds | `max_width=1024` | ✓ |
| Scale by factor | `scale=0.5` | ✓ |
| Quality preset | `quality=80` | ✓ |
| Crop to aspect | `aspect=16:9, gravity=center` | ✓ |
| Watermark corner | `watermark=logo.png, position=bottom-right, opacity=0.5` | ✓ |
| Crop to pixel region | `crop_x=100, crop_y=200, crop_w=500, crop_h=400` | ✗ |
| Watermark at coords | `watermark_x=347, watermark_y=892` | ✗ |
| Color adjustments | `saturation=+20, hue_shift=15` | ✗ |
| Filters/effects | `filter=sepia` | ✗ |

**Normalized options:**

Paraphrase's philosophy is one vocabulary, many backends:

```bash
# Same --max-width everywhere
paraphase convert image.png image.webp --max-width 1024
paraphase convert video.mp4 video.webp --max-width 1024
```

Options that can be normalized across domains belong in Paraphrase. Options that are tool-specific creative controls don't.

**Multi-input operations:**

Operations like watermarking require auxiliary inputs (the watermark image). These are in scope IF:
1. Auxiliary input is a resource path (like soundfont for MIDI→WAV)
2. Placement uses normalized presets (corners, center) not coordinates
3. Other parameters are normalized (opacity as 0-1, not tool-specific flags)

```yaml
# In scope: normalized watermark
source: { path: photo.jpg }
options:
  watermark: logo.png
  position: bottom-right  # preset, not coordinates
  opacity: 0.5
sink: { path: output.jpg }

# Out of scope: pixel-positioned watermark
options:
  watermark: logo.png
  x: 347
  y: 892
```

**Position presets:**

For operations requiring placement, Paraphrase provides semantic presets:

| Preset | Meaning |
|--------|---------|
| `top-left`, `top`, `top-right` | Corner/edge alignment |
| `left`, `center`, `right` | Middle row |
| `bottom-left`, `bottom`, `bottom-right` | Bottom row |

Plus margin/padding as percentage or normalized units.

**Gravity for cropping:**

Aspect-ratio cropping uses gravity to determine what to keep:

```bash
# Crop to 16:9, keeping center
paraphase convert photo.jpg photo.jpg --aspect 16:9 --gravity center

# Crop to 1:1, keeping top (for portraits/headshots)
paraphase convert photo.jpg photo.jpg --aspect 1:1 --gravity top
```

**What's explicitly out:**

- **Region selection** - "crop to the face", "select the background"
- **Content-aware operations** - seam carving, inpainting, upscaling
- **Color grading** - curves, levels, color balance
- **Compositing** - layers, blend modes, masks
- **Effects** - blur, sharpen, filters

These require either:
1. Creative judgment (what looks good?)
2. Content understanding (where is the subject?)
3. Tool-specific expertise (Photoshop vs GIMP vs ImageMagick)

For these, use the actual tool (ImageMagick, ffmpeg, etc.) directly, or a specialized asset pipeline.

**Consequences:**

- (+) Clear scope boundary prevents feature creep
- (+) Agent-friendly: all operations expressible as property constraints
- (+) Normalized options: one vocabulary across formats
- (+) Multi-input operations possible with auxiliary resources
- (-) Some "obvious" features excluded (arbitrary crop, filters)
- (-) Users wanting full editing must use external tools

**Future consideration:**

If a transformation becomes common enough that agents frequently need it AND it can be expressed with normalized options, it can be added. The bar is: "Would an agent reasonably request this as a target constraint?"

---

## ADR-0006: Executor Abstraction for Resource Management

**Status:** Accepted

**Context:**

Paraphrase's current architecture has three layers:
1. **Converters** - individual transformations (bytes → bytes)
2. **Planner** - finds conversion paths
3. **CLI** - orchestrates execution

The CLI currently has hardcoded sequential execution. As we add parallelism, streaming, and memory management, these concerns shouldn't pollute the core.

Problem cases:
- 100 large images × 8 threads = OOM (no memory budget)
- 1-hour audio file = 635 MB in memory (no streaming)
- Batch directory conversion = sequential (no parallelism)

**Decision:** Extract execution into a separate `Executor` trait. Core stays pure (planning, converters). Execution policy is pluggable.

**Architecture:**

```
┌─────────────────────────────────────────┐
│            Executor                     │  ← HOW to run (resources, parallelism)
├─────────────────────────────────────────┤
│            Planner                      │  ← WHAT path to take
├─────────────────────────────────────────┤
│       Registry + Converters             │  ← WHAT conversions exist
└─────────────────────────────────────────┘
```

**Executor trait:**

```rust
/// Execution context and resource constraints
pub struct ExecutionContext {
    pub registry: Arc<Registry>,
    pub memory_limit: Option<usize>,
    pub parallelism: Option<usize>,
}

/// Result of executing a plan
pub struct ExecutionResult {
    pub data: Vec<u8>,
    pub props: Properties,
    pub stats: ExecutionStats,
}

pub struct ExecutionStats {
    pub duration: Duration,
    pub peak_memory: usize,
    pub steps_executed: usize,
}

/// Executor determines HOW a plan runs
pub trait Executor: Send + Sync {
    /// Execute a single conversion plan
    fn execute(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: Vec<u8>,
        props: Properties,
    ) -> Result<ExecutionResult, ExecuteError>;

    /// Execute batch of independent conversions
    fn execute_batch(
        &self,
        ctx: &ExecutionContext,
        jobs: Vec<Job>,
    ) -> Vec<Result<ExecutionResult, ExecuteError>> {
        // Default: sequential
        jobs.into_iter()
            .map(|job| self.execute(ctx, &job.plan, job.input, job.props))
            .collect()
    }
}
```

**Executor implementations:**

| Executor | Behavior | Use case |
|----------|----------|----------|
| `SimpleExecutor` | Sequential, unbounded memory | CLI default, small files |
| `BoundedExecutor` | Sequential, memory tracking | Large files, fail-fast on OOM risk |
| `ParallelExecutor` | Rayon + memory semaphore | Batch processing |
| `StreamingExecutor` | Chunk-based I/O | Huge files (future) |

**Memory budget:**

```rust
pub struct MemoryBudget {
    limit: usize,
    used: AtomicUsize,
}

impl MemoryBudget {
    /// Try to reserve memory, returns None if would exceed limit
    pub fn try_reserve(&self, bytes: usize) -> Option<MemoryPermit>;

    /// Block until memory available (for async executor)
    pub async fn reserve(&self, bytes: usize) -> MemoryPermit;
}

pub struct MemoryPermit<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}

impl Drop for MemoryPermit<'_> {
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}
```

**Size estimation:**

Executors estimate memory needs before execution:

```rust
/// Estimate peak memory for a conversion
fn estimate_memory(input_size: usize, plan: &Plan) -> usize {
    let mut estimate = input_size;
    for step in &plan.steps {
        estimate = match step.converter_id.as_str() {
            // Audio: decode expands ~10x (MP3→PCM)
            s if s.starts_with("audio.") => estimate * 10,
            // Images: decode to RGBA, roughly width×height×4
            s if s.starts_with("image.") => estimate * 4,
            // Video: frame buffer, huge
            s if s.starts_with("video.") => estimate * 100,
            // Serde: roughly same size
            _ => estimate,
        };
    }
    estimate
}
```

This is a heuristic - converters could declare their expansion factor for better estimates.

**Parallel executor with backpressure:**

```rust
impl Executor for ParallelExecutor {
    fn execute_batch(
        &self,
        ctx: &ExecutionContext,
        jobs: Vec<Job>,
    ) -> Vec<Result<ExecutionResult, ExecuteError>> {
        let budget = MemoryBudget::new(ctx.memory_limit.unwrap_or(usize::MAX));

        jobs.into_par_iter()
            .map(|job| {
                let estimate = estimate_memory(job.input.len(), &job.plan);

                // Backpressure: wait for memory
                let _permit = budget.try_reserve(estimate)
                    .ok_or(ExecuteError::MemoryLimitExceeded)?;

                self.execute_single(ctx, &job.plan, job.input, job.props)
            })
            .collect()
    }
}
```

**CLI integration:**

```rust
fn main() {
    let registry = Registry::new();
    // ... register converters ...

    let executor: Box<dyn Executor> = if args.parallel {
        Box::new(ParallelExecutor::new(args.memory_limit))
    } else {
        Box::new(SimpleExecutor::new())
    };

    let ctx = ExecutionContext {
        registry: Arc::new(registry),
        memory_limit: args.memory_limit,
        parallelism: args.jobs,
    };

    match args.command {
        Command::Convert { input, output, .. } => {
            let plan = ctx.registry.plan(...)?;
            let result = executor.execute(&ctx, &plan, data, props)?;
            std::fs::write(output, result.data)?;
        }
        Command::Batch { inputs, .. } => {
            let jobs = inputs.iter().map(|i| make_job(i)).collect();
            let results = executor.execute_batch(&ctx, jobs);
            // ... handle results ...
        }
    }
}
```

**Streaming (future):**

For truly large files, streaming requires a different interface:

```rust
pub trait StreamingExecutor {
    fn execute_streaming(
        &self,
        ctx: &ExecutionContext,
        plan: &Plan,
        input: impl Read,
        output: impl Write,
        props: Properties,
    ) -> Result<Properties, ExecuteError>;
}
```

This only works if ALL converters in the plan support streaming. The planner would need to track this:

```rust
impl Plan {
    /// Can this plan be executed in streaming mode?
    pub fn supports_streaming(&self) -> bool {
        self.steps.iter().all(|s| s.supports_streaming)
    }
}
```

For now, we defer streaming to a future ADR. Memory-bounded parallel execution solves the immediate problem.

**Rationale:**

1. **Separation of concerns** - Core stays pure, resource management is policy
2. **Pluggable** - Different executors for different contexts (CLI vs server vs embedded)
3. **Incremental** - Start with SimpleExecutor, add ParallelExecutor, defer StreamingExecutor
4. **Testable** - Can test converters without executor, test executor with mock converters

**Consequences:**

- (+) Core unchanged - Converter trait stays simple
- (+) CLI chooses executor based on flags
- (+) Memory budget prevents OOM in batch processing
- (+) Path to streaming without redesigning converters
- (-) Extra abstraction layer
- (-) Size estimation is heuristic (could be wrong)
- (-) Streaming still requires converter changes (future work)

**Migration:**

1. Add `Executor` trait and `SimpleExecutor` to core
2. Refactor CLI to use executor
3. Add `BoundedExecutor` with memory tracking
4. Add `ParallelExecutor` with rayon + budget
5. (Future) Add streaming support per-converter
