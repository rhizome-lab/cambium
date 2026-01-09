# Cambium

Type-driven data transformation pipeline.

Part of the [Rhizome](https://rhizome-lab.github.io) ecosystem.

## Overview

Cambium is a route planner for data conversion. Given source and target properties, it finds a path through available converters automatically.

## Installation

```bash
cargo install cambium-cli
```

### Feature Flags

The CLI supports optional converter backends:

| Feature | Description |
|---------|-------------|
| `serde` | Serde formats with defaults (json, yaml, toml) |
| `serde-all` | All 18 serde formats |
| `image` | Image formats with defaults (png, jpeg, webp, gif) |
| `image-all` | All 14 image formats |
| `all` | Everything |

Default: `serde` + `image`

For minimal builds or specific formats:

```bash
# Only JSON and PNG
cargo install cambium-cli --no-default-features \
  --features cambium-serde/json,cambium-image/png

# Serde formats only (no image support)
cargo install cambium-cli --no-default-features --features serde-all

# Everything
cargo install cambium-cli --features all
```

## Usage

### Convert files

```bash
# Auto-detect formats from extensions
cambium convert input.json output.yaml
cambium convert photo.png photo.webp

# Explicit formats
cambium convert data.bin output.json --from msgpack --to json
```

### Plan conversions

```bash
# Show conversion steps without executing
cambium plan input.json output.toml
cambium plan photo.png photo.avif
```

### List converters

```bash
cambium list
```

### Workflows

Workflows define multi-step pipelines in YAML, TOML, or JSON:

```yaml
# workflow.yaml
source:
  path: input.json
sink:
  path: output.yaml
```

Run with auto-planning:
```bash
cambium run workflow.yaml
```

Or with explicit steps:
```yaml
source:
  path: input.json
steps:
  - converter: serde.json-to-yaml
sink:
  path: output.yaml
```

## Supported Formats

### Serde Formats (cambium-serde)

| Format | Feature | Extensions |
|--------|---------|------------|
| JSON | `json` | .json |
| YAML | `yaml` | .yaml, .yml |
| TOML | `toml` | .toml |
| RON | `ron` | .ron |
| JSON5 | `json5` | .json5 |
| XML | `xml` | .xml |
| S-expressions | `lexpr` | .lisp, .sexp |
| URL-encoded | `urlencoded` | - |
| Query strings | `qs` | - |
| MessagePack | `msgpack` | .msgpack, .mp |
| CBOR | `cbor` | .cbor |
| Bincode | `bincode` | .bincode, .bc |
| Postcard | `postcard` | .postcard, .pc |
| BSON | `bson` | .bson |
| FlexBuffers | `flexbuffers` | .flexbuf |
| Bencode | `bencode` | .bencode, .torrent |
| Pickle | `pickle` | .pickle, .pkl |
| Property List | `plist` | .plist |

### Image Formats (cambium-image)

| Format | Feature | Extensions |
|--------|---------|------------|
| PNG | `png` | .png |
| JPEG | `jpeg` | .jpg, .jpeg |
| WebP | `webp` | .webp |
| GIF | `gif` | .gif |
| BMP | `bmp` | .bmp |
| ICO | `ico` | .ico |
| TIFF | `tiff` | .tif, .tiff |
| TGA | `tga` | .tga |
| PNM | `pnm` | .pnm, .pbm, .pgm, .ppm |
| Farbfeld | `farbfeld` | .ff |
| QOI | `qoi` | .qoi |
| AVIF | `avif` | .avif |
| OpenEXR | `openexr` | .exr |
| Radiance HDR | `hdr` | .hdr |

## Library Usage

```rust
use cambium::{Registry, Planner, Properties, PropertyPattern, Cardinality};

// Create registry and register converters
let mut registry = Registry::new();
cambium_serde::register_all(&mut registry);
cambium_image::register_all(&mut registry);

// Plan a conversion
let planner = Planner::new(&registry);
let source = Properties::new().with("format", "json");
let target = PropertyPattern::new().eq("format", "yaml");

if let Some(plan) = planner.plan(&source, &target, Cardinality::One, Cardinality::One) {
    for step in &plan.steps {
        println!("  {}", step.converter_id);
    }
}
```

## License

MIT
