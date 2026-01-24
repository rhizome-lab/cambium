# Paraphrase

Type-driven data transformation pipeline.

Part of the [rhi](https://rhi.zone) ecosystem.

## Overview

Paraphrase is a route planner for data conversion. Given source and target properties, it finds a path through available converters automatically.

## Installation

```bash
cargo install rhi-paraphase-cli
```

### Feature Flags

The CLI supports optional converter backends:

| Feature | Description |
|---------|-------------|
| `serde` | Serde formats with defaults (json, yaml, toml) |
| `serde-all` | All 18 serde formats |
| `image` | Image formats with defaults (png, jpeg, webp, gif) |
| `image-all` | All 14 image formats |
| `video` | Video formats with defaults (mp4, webm, gif) - requires FFmpeg |
| `video-all` | All video formats |
| `audio` | Audio formats with defaults (wav, flac, mp3, ogg) - pure Rust |
| `audio-all` | All audio formats |
| `all` | Everything |

Default: `serde` + `image` (video/audio excluded)

For minimal builds or specific formats:

```bash
# Only JSON and PNG
cargo install rhi-paraphase-cli --no-default-features \
  --features rhi-paraphase-serde/json,rhi-paraphase-image/png

# Serde formats only (no image support)
cargo install rhi-paraphase-cli --no-default-features --features serde-all

# Everything
cargo install rhi-paraphase-cli --features all
```

## Usage

### Convert files

```bash
# Auto-detect formats from extensions
paraphase convert input.json output.yaml
paraphase convert photo.png photo.webp

# Explicit formats
paraphase convert data.bin output.json --from msgpack --to json
```

### Image transforms

```bash
# Resize to fit within max width (preserves aspect ratio)
paraphase convert photo.png thumb.png --max-width 200

# Scale by factor
paraphase convert photo.png half.png --scale 0.5

# Crop to aspect ratio
paraphase convert photo.png banner.png --aspect 16:9

# Crop with gravity (anchor point)
paraphase convert photo.png portrait.png --aspect 3:4 --gravity top

# Combine transforms with format conversion
paraphase convert photo.png avatar.webp --aspect 1:1 --max-width 150

# Add watermark
paraphase convert photo.png branded.png --watermark logo.png --watermark-position bottom-right

# Watermark with opacity and margin
paraphase convert photo.png branded.png --watermark logo.png \
  --watermark-position bottom-right --watermark-opacity 0.5 --watermark-margin 20
```

### Video conversion (requires FFmpeg)

```bash
# Convert between video formats
paraphase convert video.mp4 video.webm

# Resize video
paraphase convert video.mp4 small.mp4 --max-width 720

# GIF to video
paraphase convert animation.gif video.mp4
```

### Audio conversion (pure Rust)

```bash
# Convert MP3 to WAV
paraphase convert song.mp3 song.wav

# Convert FLAC to WAV
paraphase convert album.flac album.wav
```

### Plan conversions

```bash
# Show conversion steps without executing
paraphase plan input.json output.toml
paraphase plan photo.png photo.avif
```

### List converters

```bash
paraphase list
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
paraphase run workflow.yaml
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

### Serde Formats (rhi-paraphase-serde)

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

### Image Formats (rhi-paraphase-image)

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

### Video Formats (rhi-paraphase-video)

| Format | Feature | Extensions |
|--------|---------|------------|
| MP4 | `mp4` | .mp4, .m4v |
| WebM | `webm` | .webm |
| MKV | `mkv` | .mkv |
| AVI | `avi` | .avi |
| MOV | `mov` | .mov, .qt |
| GIF | `gif` | .gif |

### Audio Formats (rhi-paraphase-audio)

| Format | Feature | Decode | Encode |
|--------|---------|--------|--------|
| WAV | `wav` | ✓ | ✓ |
| FLAC | `flac` | ✓ | - |
| MP3 | `mp3` | ✓ | - |
| OGG | `ogg` | ✓ | - |
| AAC | `aac` | ✓ | - |

## Library Usage

```rust
use rhi_paraphase_core::{Registry, Planner, Properties, PropertyPattern, Cardinality};

// Create registry and register converters
let mut registry = Registry::new();
rhi_paraphase_serde::register_all(&mut registry);
rhi_paraphase_image::register_all(&mut registry);

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
