# Paraphase TODO

## Format Conversions (queued simplest → most complex)

### Trivial (pure Rust, minimal code)

- [x] **Base64/Hex encoding** - `base64`, `hex` crates; encode/decode bytes
- [x] **NDJSON/JSON Lines** - split lines + existing serde_json; streaming-friendly

### Simple (pure Rust, self-contained)

- [x] **Compression** - gzip (`flate2`), zstd, brotli; wrap/unwrap bytes
- [x] **INI config** - `rust-ini`; simple key-value config files
- [x] **Character encoding** - `encoding_rs`; UTF-16, Latin-1, Shift-JIS, etc.

### Medium (pure Rust, more logic)

- [x] **Markdown → HTML** - `pulldown-cmark`; CommonMark compliant
- [x] **HTML → text** - `html2text`; strip tags, preserve structure
- [x] **Archives** - `tar`, `zip` crates; extract/create, maps to Multi output

### Complex (schema-based or native deps)

- [x] **Spreadsheets** - `calamine` for XLSX/ODS/XLS reading (read-only)
- [x] **Avro** - `apache-avro`; schema embedded in container files (self-describing)
- [x] **Parquet** - `parquet`; columnar format via Arrow (self-describing)

### Schema-required (need external definition)

These formats require schema files to decode - not "point and shoot":

- [ ] **Protobuf** - `prost`; requires .proto schema files
- [ ] **Cap'n Proto** - `capnp`; zero-copy, requires .capnp schema files

---

## Document Conversion (paraphase-document)

Thin integration with a document IR library (separate project).

See `docs/document-ir-spec.md` for comprehensive spec of the document IR:
- Analysis of Pandoc's strengths/weaknesses
- Property-bag based architecture (aligns with Cambium philosophy)
- Layered representation (semantic, style, layout)
- Fidelity tracking for lossy conversions
- Embedded resource handling

**The document IR is out of Cambium's scope** - it's a standalone library project.

paraphase-document will:
- [ ] Integrate with document IR library (once it exists)
- [ ] Register format converters with Cambium registry
- [ ] Route document conversions through Cambium's executor

## Audio Encoders (paraphase-audio)

Currently only WAV encoding is supported. Adding encoders for other formats:

- [ ] **FLAC encoder** - pure Rust via `flacenc` crate (if stable)
- [ ] **MP3 encoder** - requires `lame` (native dependency)
- [ ] **OGG Vorbis encoder** - requires `libvorbis` (native dependency)
- [ ] **AAC encoder** - requires FFmpeg or native lib
- [ ] **Opus encoder** - consider as modern alternative to OGG

## Video (paraphase-video)

- [ ] Complete frame encoding pipeline (currently scaffold)
- [ ] Audio track passthrough/transcoding
- [ ] Subtitle extraction

## Architecture

See ADR-0006 for the Executor abstraction.

Implemented:
- [x] **SimpleExecutor** - sequential, unbounded memory
- [x] **BoundedExecutor** - sequential with memory limit checking (fail-fast)
- [x] **ParallelExecutor** - rayon + memory budget for batch (requires `parallel` feature)
- [x] **MemoryBudget** - semaphore-like reservation with RAII permits

Future work:
- [ ] **StreamingExecutor** - chunk-based I/O for huge files (requires converter interface changes)

## CLI Usability

Implemented:
- [x] **Shell completions** - `paraphase completions bash/zsh/fish`
- [x] **Man pages** - `paraphase manpage > paraphase.1`
- [x] **Verbose/quiet modes** - `-v` for debug info, `-q` for silent
- [x] **Better format detection** - magic bytes before extension fallback
- [x] **Stdin/stdout piping** - `cat file.mp3 | paraphase convert - -o - --from mp3 --to wav`
- [x] **Batch processing** - `paraphase convert *.mp3 --output-dir out/ --to wav`
- [x] **Progress reporting** - progress bars for batch conversions

Implemented:
- [x] **Presets** - `--preset web` for common conversion profiles
- [x] **Config file** - `~/.config/paraphase/config.toml` for defaults
- [x] **Dynamic presets** - Dew expressions in preset values (requires `dew` feature)

Implemented:
- [x] **Path optimization** - `--optimize quality|speed|size` for multi-path selection
- [x] **Better error messages** - actionable suggestions, format hints, typo detection

## Dynamic Presets (Dew Integration)

With the `dew` feature enabled, preset numeric values can be expressions:

```toml
# ~/.config/paraphase/config.toml
[preset.smart-web]
max_width = "min(width, 1920)"
max_height = "min(height, 1080)"
quality = "if file_size > 5000000 then 70 else 85"

[preset.proportional]
max_width = "width * 0.5"
max_height = "height * 0.5"
```

Available variables (from input file properties):
- `width`, `height` - image dimensions
- `file_size` - input file size in bytes
- Any other numeric property from the input

Expressions use [Dew](https://github.com/rhi-zone/dew) syntax with standard math functions:
- Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Math: `min`, `max`, `clamp`, `abs`, `sqrt`, `pow`
- Conditionals: `if ... then ... else ...`

Build with expressions: `cargo build -p paraphase-cli --features dew`

## Testing & Quality

Implemented:
- [x] **Integration tests** - 18 end-to-end CLI tests covering:
  - Multi-hop chains (JSON → YAML → TOML, roundtrips)
  - Batch processing with multiple files
  - Progress bar and quiet mode
  - Presets and config
  - Optimize flag variations
- [x] **Unit tests** - Archive roundtrips (tar, zip), format converters
- [x] **CI/CD** - GitHub Actions for check/test/fmt/clippy/doc/build

Implemented:
- [x] **Expansion executor** - `execute_expanding()` properly fans out 1→N through pipeline
- [x] **Aggregation executor** - `execute_aggregating()` for N→1 conversions (files → archive)
- [x] **Compound archives** - `tar.gz`, `tar.zst`, `tgz` with post-aggregation compression
- [x] **Glob support** - `paraphase convert "*.json" --to yaml`
- [x] **Directory recursion** - `-r/--recursive` for tree traversal
- [x] **Batch modes** - `--batch-mode all|per-dir` for different grouping strategies

Known limitations (documented, not bugs):
- Output filenames may collide when processing trees (flat output dir)

Future work:
- [ ] **Benchmarks** - criterion benchmarks for regression tracking
- [ ] **Preserve directory structure** - mirror input tree to output tree

## Complexity Hotspots (threshold >21)
- [ ] `crates/paraphase-cli/src/main.rs:detect_format` (44)
- [ ] `crates/paraphase-audio/src/lib.rs:convert_to_i16` (40)
- [ ] `crates/paraphase-cli/src/main.rs:convert_single_file` (38)
- [ ] `crates/paraphase-image/src/lib.rs:compute_resize_dimensions` (30)
- [ ] `crates/paraphase-cli/src/main.rs:mime_to_format` (29)
- [ ] `crates/paraphase-serde/src/lib.rs:avro_impl.avro_value_to_json` (28)
- [ ] `crates/paraphase-image/src/lib.rs:composite_with_opacity` (27)
- [ ] `crates/paraphase-cli/src/main.rs:cmd_plan_workflow` (21)
- [ ] `crates/paraphase-serde/src/lib.rs:deserialize` (21)
- [ ] `crates/paraphase-serde/src/lib.rs:serialize` (21)

## Distribution

Implemented:
- [x] **Man pages** - via `paraphase manpage` command

Deferred (needs ecosystem consensus):
- [ ] **Packaging** - cargo-dist, Homebrew formula, AUR package
- [ ] **Release binaries** - pre-built for Linux/macOS/Windows
