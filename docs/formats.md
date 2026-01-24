# Formats Reference

Complete list of formats supported by Paraphrase converters.

## Serde Formats (paraphase-serde)

All serde formats use `serde_json::Value` as an intermediate representation, enabling conversion between any pair of enabled formats.

### Text Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| JSON | `json` | .json | Default enabled |
| YAML | `yaml` | .yaml, .yml | Default enabled |
| TOML | `toml` | .toml | Default enabled |
| RON | `ron` | .ron | Rust Object Notation |
| JSON5 | `json5` | .json5 | JSON with comments, trailing commas |
| XML | `xml` | .xml | Via quick-xml |
| S-expressions | `lexpr` | .lisp, .sexp | Lisp-style |
| URL-encoded | `urlencoded` | - | Form data |
| Query strings | `qs` | - | Nested query params |

### Binary Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| MessagePack | `msgpack` | .msgpack, .mp | Compact binary JSON-like |
| CBOR | `cbor` | .cbor | Concise Binary Object Representation |
| Bincode | `bincode` | .bincode, .bc | Rust-native binary |
| Postcard | `postcard` | .postcard, .pc | Embedded-friendly |
| BSON | `bson` | .bson | MongoDB binary format |
| FlexBuffers | `flexbuffers` | .flexbuf | Schema-less FlatBuffers |
| Bencode | `bencode` | .bencode, .torrent | BitTorrent format |
| Pickle | `pickle` | .pickle, .pkl | Python serialization |
| Property List | `plist` | .plist | Apple binary plist |

### Feature Groups

```toml
# Cargo.toml for paraphase-serde
[features]
default = ["json", "yaml", "toml"]
all = ["json", "yaml", "toml", "ron", "json5", "xml", "lexpr",
       "urlencoded", "qs", "msgpack", "cbor", "bincode", "postcard",
       "bson", "flexbuffers", "bencode", "pickle", "plist"]
```

## Image Formats (paraphase-image)

All image formats use `image::DynamicImage` as an intermediate representation.

### Lossless Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| PNG | `png` | .png | Default enabled |
| GIF | `gif` | .gif | Default enabled, animated support |
| BMP | `bmp` | .bmp | Windows bitmap |
| ICO | `ico` | .ico | Windows icon |
| TIFF | `tiff` | .tif, .tiff | Tagged image |
| TGA | `tga` | .tga | Truevision |
| PNM | `pnm` | .pnm, .pbm, .pgm, .ppm, .pam | Portable anymap family |
| Farbfeld | `farbfeld` | .ff | Simple lossless |
| QOI | `qoi` | .qoi | Quite OK Image |

### Lossy Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| JPEG | `jpeg` | .jpg, .jpeg | Default enabled |
| WebP | `webp` | .webp | Default enabled |
| AVIF | `avif` | .avif | AV1-based |

### HDR Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| OpenEXR | `openexr` | .exr | High dynamic range |
| Radiance HDR | `hdr` | .hdr | RGBE format |

### Feature Groups

```toml
# Cargo.toml for paraphase-image
[features]
default = ["png", "jpeg", "webp", "gif"]
all = ["png", "jpeg", "webp", "gif", "bmp", "ico", "tiff", "tga",
       "pnm", "farbfeld", "qoi", "avif", "openexr", "hdr"]
```

### Image Transforms

Beyond format conversion, paraphase-image provides transform operations:

| Converter | Description | Options |
|-----------|-------------|---------|
| `image.resize` | Resize image | `max_width`, `max_height`, `scale`, `target_width`, `target_height` |
| `image.crop-aspect` | Crop to aspect ratio | `aspect` (e.g., "16:9"), `gravity` |
| `image.watermark` | Overlay watermark | `position`, `opacity`, `margin` (multi-input) |

**Resize options:**

- `max_width` / `max_height`: Fit within bounds, preserving aspect ratio (no upscaling)
- `scale`: Scale factor (e.g., 0.5 for half size)
- `target_width` / `target_height`: Exact dimensions (may change aspect ratio)

**Gravity presets** (for crop anchor point):

| Preset | Aliases |
|--------|---------|
| `top-left` | `nw`, `northwest` |
| `top` | `n`, `north` |
| `top-right` | `ne`, `northeast` |
| `left` | `w`, `west` |
| `center` | `c`, `middle` (default) |
| `right` | `e`, `east` |
| `bottom-left` | `sw`, `southwest` |
| `bottom` | `s`, `south` |
| `bottom-right` | `se`, `southeast` |

**CLI usage:**

```bash
# Resize to fit within 1024px width
paraphase convert photo.png photo.webp --max-width 1024

# Scale to 50%
paraphase convert photo.png thumb.png --scale 0.5

# Crop to 16:9, keeping top of image
paraphase convert photo.png banner.png --aspect 16:9 --gravity top

# Combine: crop to square, resize, convert format
paraphase convert photo.png avatar.webp --aspect 1:1 --max-width 200

# Add watermark
paraphase convert photo.png branded.png --watermark logo.png

# Watermark with options
paraphase convert photo.png branded.png --watermark logo.png \
  --watermark-position bottom-right --watermark-opacity 0.5 --watermark-margin 20
```

**Watermark options:**

- `position`: Where to place the watermark (uses gravity presets above)
- `opacity`: Watermark transparency (0.0-1.0, default 1.0)
- `margin`: Pixels from edge (default 0)

## Audio Formats (paraphase-audio)

Pure Rust audio processing via Symphonia (decode) and Hound (WAV encode).

### Supported Formats

| Format | Decode | Encode | Feature |
|--------|--------|--------|---------|
| WAV | ✓ | ✓ | `wav` |
| FLAC | ✓ | - | `flac` |
| MP3 | ✓ | - | `mp3` |
| OGG Vorbis | ✓ | - | `ogg` |
| AAC | ✓ | - | `aac` |

**Note:** Currently all formats decode to WAV. Encoders for other formats are planned.

### Feature Groups

```toml
# Cargo.toml for paraphase-audio
[features]
default = ["wav", "flac", "mp3", "ogg"]
all = ["wav", "flac", "mp3", "ogg", "aac"]
```

**CLI usage:**

```bash
# Convert MP3 to WAV
paraphase convert song.mp3 song.wav

# Convert FLAC to WAV
paraphase convert album.flac album.wav

# Convert OGG to WAV
paraphase convert audio.ogg audio.wav
```

## Video Formats (paraphase-video)

All video formats use FFmpeg as the transcoding backend. **Requires FFmpeg installed at runtime.**

### Container Formats

| Format | Feature | Extensions | Default Codecs |
|--------|---------|------------|----------------|
| MP4 | `mp4` | .mp4, .m4v | H.264 + AAC |
| WebM | `webm` | .webm | VP9 + Opus |
| MKV | `mkv` | .mkv | H.264 + AAC |
| AVI | `avi` | .avi | MPEG-4 + MP3 |
| MOV | `mov` | .mov, .qt | H.264 + AAC |
| GIF | `gif` | .gif | GIF (animated) |

### Feature Groups

```toml
# Cargo.toml for paraphase-video
[features]
default = ["mp4", "webm", "gif"]
all = ["mp4", "webm", "mkv", "avi", "mov", "gif", "audio"]
```

### Video Transforms

| Converter | Description | Options |
|-----------|-------------|---------|
| `video.resize` | Resize video | `max_width`, `max_height`, `scale` |

### Quality Presets

| Preset | CRF | Use Case |
|--------|-----|----------|
| `low` | 28 | Smaller file size |
| `medium` | 23 | Balanced (default) |
| `high` | 18 | Higher quality |
| `lossless` | 0 | No quality loss |

**CLI usage:**

```bash
# Convert MP4 to WebM
paraphase convert video.mp4 video.webm

# Convert with quality preset
paraphase convert video.mp4 video.webm --quality high

# Resize video
paraphase convert video.mp4 small.mp4 --max-width 720

# GIF to video
paraphase convert animation.gif video.mp4
```

## CLI Feature Flags

The CLI combines all converter backends:

```toml
# Cargo.toml for paraphase-cli
[features]
default = ["serde", "image"]

# Include backends
serde = ["dep:paraphase-serde"]
image = ["dep:paraphase-image"]
video = ["dep:paraphase-video"]  # Requires FFmpeg
audio = ["dep:paraphase-audio"]

# Enable all formats per backend
serde-all = ["serde", "paraphase-serde/all"]
image-all = ["image", "paraphase-image/all"]
video-all = ["video", "paraphase-video/all"]
audio-all = ["audio", "paraphase-audio/all"]

# Everything (video excluded from default, requires FFmpeg)
all = ["serde-all", "image-all", "video-all", "audio-all"]
```

### Installation Examples

```bash
# Default: common serde + common image formats
cargo install paraphase-cli

# All formats
cargo install paraphase-cli --features all

# Only serde formats (no image support)
cargo install paraphase-cli --no-default-features --features serde-all

# Only image formats (no serde support)
cargo install paraphase-cli --no-default-features --features image-all

# Specific formats only
cargo install paraphase-cli --no-default-features \
  --features paraphase-serde/json,paraphase-serde/yaml,paraphase-image/png
```

## Converter Naming

Converters follow the pattern `{crate}.{from}-to-{to}`:

- `serde.json-to-yaml`
- `serde.toml-to-msgpack`
- `image.png-to-webp`
- `image.jpg-to-gif`

List all available converters:

```bash
paraphase list
```

## Adding Custom Converters

Implement the `Converter` trait:

```rust
use paraphase::{Converter, ConverterDecl, ConvertError, ConvertOutput, Properties, PropertyPattern};

pub struct MyConverter {
    decl: ConverterDecl,
}

impl MyConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::simple(
            "my.foo-to-bar",
            PropertyPattern::new().eq("format", "foo"),
            PropertyPattern::new().eq("format", "bar"),
        ).description("Convert foo to bar");

        Self { decl }
    }
}

impl Converter for MyConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Transform input bytes to output bytes
        let output = transform(input)?;

        let mut out_props = props.clone();
        out_props.insert("format".into(), "bar".into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}
```

Register with a registry:

```rust
let mut registry = Registry::new();
registry.register(MyConverter::new());
```
