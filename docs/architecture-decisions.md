# Architecture Decisions

Record of key technical choices for Cambium.

## 001: Plugin Format - C ABI Dynamic Libraries

**Status:** Accepted

**Context:**

Cambium needs a plugin system for converters. Options considered:

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
// cambium_plugin.h

#include <stdint.h>
#include <stddef.h>

#define CAMBIUM_PLUGIN_API_VERSION 1

// Converter metadata
typedef struct {
    const char* id;           // unique identifier, e.g. "serde.json_to_yaml"
    const char* from_type;    // e.g. "json"
    const char* to_type;      // e.g. "yaml"
    uint32_t flags;           // CAMBIUM_FLAG_* bitmask
} CambiumConverter;

// Flags
#define CAMBIUM_FLAG_LOSSLESS   (1 << 0)
#define CAMBIUM_FLAG_STREAMING  (1 << 1)

// Plugin exports these symbols:

// Called once on load, returns API version for compatibility check
uint32_t cambium_plugin_version(void);

// List available converters (caller does NOT free)
const CambiumConverter* cambium_list_converters(size_t* count);

// Perform conversion
// Returns 0 on success, non-zero error code on failure
// On success, *output and *output_len are set (caller must free with cambium_free)
// options_json may be NULL
int cambium_convert(
    const char* converter_id,
    const uint8_t* input, size_t input_len,
    uint8_t** output, size_t* output_len,
    const char* options_json
);

// Free memory allocated by cambium_convert
void cambium_free(void* ptr);

// Optional: get error message for last failure (may return NULL)
const char* cambium_last_error(void);
```

**Rust Plugin Authoring:**

`cambium-plugin` crate provides ergonomic wrapper:

```rust
use cambium_plugin::prelude::*;

#[cambium_converter(from = "json", to = "yaml", lossless)]
fn json_to_yaml(input: &[u8], _opts: &Options) -> Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(input)?;
    Ok(serde_yaml::to_vec(&value)?)
}

// Macro generates:
// - #[no_mangle] extern "C" fn cambium_plugin_version() -> u32
// - #[no_mangle] extern "C" fn cambium_list_converters(*mut usize) -> *const CambiumConverter
// - #[no_mangle] extern "C" fn cambium_convert(...) -> i32
// - #[no_mangle] extern "C" fn cambium_free(*mut c_void)
// - #[no_mangle] extern "C" fn cambium_last_error() -> *const c_char

cambium_plugin::export![json_to_yaml];
```

**Plugin Discovery:**

Plugins are discovered from (in order):
1. Built-in converters (compiled into cambium binary)
2. `$CAMBIUM_PLUGIN_PATH` (colon-separated)
3. `~/.cambium/plugins/*.{so,dylib,dll}`
4. Project-local `./cambium-plugins/*.{so,dylib,dll}`

Later sources can override earlier ones (project-local wins).

**Loading:**

```rust
// Pseudocode
fn load_plugin(path: &Path) -> Result<Plugin> {
    let lib = libloading::Library::new(path)?;

    let version: Symbol<fn() -> u32> = lib.get(b"cambium_plugin_version")?;
    if version() != CAMBIUM_PLUGIN_API_VERSION {
        return Err(IncompatibleVersion);
    }

    let list: Symbol<fn(*mut usize) -> *const CambiumConverter> =
        lib.get(b"cambium_list_converters")?;
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

## 002: Library-First Design

**Status:** Accepted

**Context:**

Cambium can be designed as either:
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
// cambium/src/lib.rs

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

/// Perform conversion
pub fn convert(
    registry: &Registry,
    from: &str,
    to: &str,
    input: &[u8],
    options: Option<&str>,  // JSON options
) -> Result<Vec<u8>>;

/// Streaming conversion for large files
pub fn convert_stream(
    registry: &Registry,
    from: &str,
    to: &str,
    input: impl Read,
    output: impl Write,
    options: Option<&str>,
) -> Result<()>;

/// Converter trait for built-in converters
pub trait Converter: Send + Sync {
    fn info(&self) -> ConverterInfo;
    fn convert(&self, input: &[u8], options: Option<&str>) -> Result<Vec<u8>>;
}
```

**CLI as wrapper:**

```rust
// cambium-cli/src/main.rs

use cambium::Registry;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let registry = Registry::with_default_plugins()?;

    match args.command {
        Command::Convert { input, output, from, to } => {
            let from = from.or_else(|| detect_type(&input));
            let to = to.or_else(|| detect_type(&output));

            let data = std::fs::read(&input)?;
            let result = cambium::convert(&registry, &from, &to, &data, None)?;
            std::fs::write(&output, result)?;
        }
        Command::List => {
            for c in registry.converters() {
                println!("{} -> {}", c.from_type, c.to_type);
            }
        }
        Command::Path { from, to } => {
            match registry.find_path(&from, &to) {
                Some(path) => println!("{}", path.iter().map(|c| &c.id).join(" -> ")),
                None => eprintln!("No conversion path found"),
            }
        }
    }
    Ok(())
}
```

**Consequences:**

- (+) Resin can use cambium with zero overhead
- (+) In-memory conversions without temp files
- (+) Testable without spawning processes
- (+) Can introspect and optimize conversion paths
- (-) Rust-only for direct usage (others use CLI)
- (-) Must maintain semver stability for library API

**Crate structure:**

```
crates/
  cambium/           # library (pub API)
  cambium-cli/       # binary (thin wrapper)
  cambium-plugin/    # plugin authoring helpers
```
