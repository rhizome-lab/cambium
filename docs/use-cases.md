# Use Cases

Concrete scenarios to drive type system design. Grouped by domain and complexity.

## Config / Structured Data

### UC-01: JSON ↔ YAML ↔ TOML

**Scenario:** Convert between config formats losslessly.

```bash
paraphase convert config.json config.yaml
paraphase convert config.yaml config.toml
```

**Type complexity:** Trivial - flat types, no parameters.
- `json`, `yaml`, `toml` are distinct types
- All share an IR (in-memory tree)
- Lossless round-trip (modulo comments, formatting)

### UC-02: JSON with schema validation

**Scenario:** Convert JSON, but only if it matches a schema.

```bash
paraphase convert --validate schema.json input.json output.yaml
```

**Type complexity:** Type + constraint?
- `json` vs `json[schema=foo.json]`?
- Or is validation a separate step, not a type?

### UC-03: Minify / pretty-print

**Scenario:** Same format in, same format out, different representation.

```bash
paraphase convert input.json --to json --minify
paraphase convert input.json --to json --pretty
```

**Type complexity:** Same type, different output options.
- Is `json:minified` a different type than `json:pretty`?
- Or just converter options?

---

## Images

### UC-10: PNG → WebP

**Scenario:** Simple format conversion.

```bash
paraphase convert photo.png photo.webp
```

**Type complexity:** Trivial - flat types.

### UC-11: Batch resize + convert

**Scenario:** Convert all PNGs to WebP at max 1024px width.

```bash
paraphase convert "*.png" --to webp --max-width 1024
```

**Type complexity:** Parameters on output.
- `webp[width<=1024]` as target type?
- Or just converter options?

### UC-12: Only convert large images

**Scenario:** Convert images, but only if they exceed a size threshold.

```bash
paraphase convert "*.png" --to webp --if "width > 2048"
```

**Type complexity:** Predicate on input.
- Input type: `png[width>2048]`
- Needs content inspection to evaluate predicate

### UC-13: Lossy vs lossless WebP

**Scenario:** Explicitly choose lossy or lossless encoding.

```bash
paraphase convert photo.png photo.webp --lossy --quality 80
paraphase convert diagram.png diagram.webp --lossless
```

**Type complexity:** Same output format, different encoding.
- `webp:lossy` vs `webp:lossless`?
- Or converter option?

---

## Video

### UC-20: Container conversion

**Scenario:** Change container without re-encoding.

```bash
paraphase convert video.mkv video.mp4 --copy-streams
```

**Type complexity:** Container vs codec distinction.
- Type is container: `mkv`, `mp4`
- Codec is... parameter? Separate type?

### UC-21: Pixel format conversion

**Scenario:** Convert YUV411 to YUV420p for compatibility.

```bash
paraphase convert input.mp4 output.mp4 --pixfmt yuv420p
# or: only convert if source is yuv411
paraphase convert input.mp4 output.mp4 --from-pixfmt yuv411 --to-pixfmt yuv420p
```

**Type complexity:** This is the motivating example.
- `video[pixfmt=yuv411]` → `video[pixfmt=yuv420p]`
- Requires inspecting input to know its pixfmt
- Routing depends on parameter values

### UC-22: Resolution targeting

**Scenario:** Downscale 4K to 1080p, leave 1080p alone.

```bash
paraphase convert "*.mp4" --to mp4 --max-height 1080
```

**Type complexity:** Conditional transformation.
- Only applies if `height > 1080`
- Output guarantees `height <= 1080`

### UC-23: Codec selection

**Scenario:** Re-encode H.264 to H.265/HEVC.

```bash
paraphase convert input.mp4 output.mp4 --codec hevc
```

**Type complexity:** Codec as type or parameter?
- `video/mp4:h264` → `video/mp4:hevc`?
- Or `mp4` → `mp4` with codec option?

---

## Audio

### UC-30: WAV → MP3

**Scenario:** Lossy compression.

```bash
paraphase convert audio.wav audio.mp3 --bitrate 320k
```

**Type complexity:** Trivial + quality parameter.

### UC-31: Sample rate conversion

**Scenario:** Resample 48kHz to 44.1kHz for CD.

```bash
paraphase convert audio.wav output.wav --samplerate 44100
```

**Type complexity:** Same format, different sample rate.
- `wav[samplerate=48000]` → `wav[samplerate=44100]`?

### UC-32: Channel downmix

**Scenario:** 5.1 surround to stereo.

```bash
paraphase convert surround.wav stereo.wav --channels 2
```

**Type complexity:** Channel layout as type parameter?

---

## Documents

### UC-40: Markdown → HTML

**Scenario:** Basic document conversion.

```bash
paraphase convert readme.md readme.html
```

**Type complexity:** Trivial.

### UC-41: Markdown → PDF (multi-hop)

**Scenario:** No direct converter, needs intermediate.

```bash
paraphase convert readme.md readme.pdf
# Internally: md → html → pdf, or md → latex → pdf
```

**Type complexity:** Graph traversal, multiple paths possible.
- Which path is better? User preference? Plugin availability?

### UC-42: Markdown with frontmatter

**Scenario:** YAML frontmatter should be preserved/extracted.

```bash
paraphase convert post.md post.html --preserve-frontmatter
paraphase convert post.md --extract-frontmatter > metadata.yaml
```

**Type complexity:** Substructure within format.
- `markdown` vs `markdown+frontmatter`?

### UC-43: Pandoc-style format variants

**Scenario:** GitHub-flavored markdown vs CommonMark vs MultiMarkdown.

```bash
paraphase convert --from gfm input.md output.html
paraphase convert --from commonmark input.md output.html
```

**Type complexity:** Variants of same base format.
- `markdown:gfm`, `markdown:commonmark`?
- Or separate types `gfm`, `commonmark`?

---

## 3D / Mesh

### UC-50: OBJ → glTF

**Scenario:** Convert mesh formats.

```bash
paraphase convert model.obj model.gltf
```

**Type complexity:** Trivial if just format.

### UC-51: Mesh optimization

**Scenario:** Decimate mesh, merge vertices.

```bash
paraphase convert model.obj model.gltf --decimate 0.5 --merge-threshold 0.001
```

**Type complexity:** Transformation during conversion.
- Are these converter options or type parameters?

### UC-52: Extract textures

**Scenario:** glTF with embedded textures → glTF with external textures.

```bash
paraphase convert model.glb model.gltf --extract-textures ./textures/
```

**Type complexity:** Same format, different embedding mode.
- `gltf:binary` vs `gltf:separate`?

---

## Archives / Containers

### UC-55: Basic archive conversion

**Scenario:** Convert between archive formats.

```bash
paraphase convert archive.zip archive.tar.gz
paraphase convert archive.rar archive.7z
```

**Type complexity:** Straightforward format conversion.

### UC-56: Nested/compound formats

**Scenario:** `.tar.gz` is gzip(tar), not a single format.

```bash
paraphase convert archive.tar.gz archive.tar.xz  # recompress only
paraphase convert archive.tar.gz archive.zip     # unpack tar, repack as zip
```

**Type complexity:** Layered types.
- `tar.gz` = `gzip[inner=tar]`?
- `tar.xz` = `xz[inner=tar]`?
- Converter needs to know whether to unwrap fully or just re-compress outer layer

### UC-57: "Secretly archives" formats

**Scenario:** JAR, DOCX, APK, Unity packages, Godot PCK, some EXEs are actually ZIPs.

```bash
paraphase convert app.jar contents/         # extract
paraphase convert contents/ app.jar         # repack
paraphase convert game.pck assets/          # godot package
paraphase convert document.docx document/   # office xml
```

**Type complexity:** Same underlying format, different semantics.
- `jar` ≈ `zip[convention=java]`
- `docx` ≈ `zip[convention=ooxml]`
- `apk` ≈ `zip[convention=android]`
- Should paraphase know these equivalences?

### UC-58: Extract + convert contents

**Scenario:** Extract archive, convert contents, repack.

```bash
paraphase convert textures.zip textures-optimized.zip \
  --convert-contents "*.png -> webp" --quality 80
```

**Type complexity:** Recursive conversion.
- Pipeline into archive contents
- Preserve structure

### UC-59: Self-extracting archives

**Scenario:** EXE that's secretly a ZIP with a stub.

```bash
paraphase convert installer.exe contents/    # extract
paraphase convert contents/ installer.exe    # create sfx
```

**Type complexity:** Format detection vs extension.
- `.exe` could be PE binary or SFX archive
- Content sniffing required

---

## Code / AST

### UC-60: Syntax highlighting

**Scenario:** Source code → HTML with highlighting.

```bash
paraphase convert main.rs main.html --highlight
```

**Type complexity:** Source language detection.
- Type is `rust`, `python`, etc.? Or generic `source`?

### UC-61: AST extraction

**Scenario:** Parse code, output AST as JSON.

```bash
paraphase convert main.rs ast.json --format tree-sitter
```

**Type complexity:** Multiple output formats for AST.
- `rust` → `ast:tree-sitter` vs `ast:syn`?

---

## Cross-Domain

### UC-70: Pipeline composition

**Scenario:** Chain multiple conversions explicitly.

```bash
paraphase pipe input.md \
  | markdown-to-html \
  | minify-html \
  | gzip \
  > output.html.gz
```

**Type complexity:** Explicit pipeline, no routing.

### UC-71: Automatic multi-hop

**Scenario:** System finds path through graph.

```bash
paraphase convert input.md output.pdf
# System determines: md → html → pdf
```

**Type complexity:** Graph traversal.

### UC-72: Constrained path

**Scenario:** User constrains intermediate formats.

```bash
paraphase convert input.md output.pdf --via latex
# Forces: md → latex → pdf
```

**Type complexity:** Path constraints.

---

## Extended Chains (Unconventional)

### UC-80: Manufacturing - CAD to GCODE

**Scenario:** 3D model to printable instructions.

```bash
paraphase convert model.step model.gcode --printer ender3
# Internally: STEP → STL → sliced GCODE
```

**Complexity:** Multi-hop through different domains (CAD → mesh → toolpath).
External config needed (printer profile).

### UC-81: PCB pipeline

**Scenario:** PCB design to fabrication files.

```bash
paraphase convert board.kicad_pcb --to gerber-zip
# Outputs: multiple gerber layers + drill files + preview PNG
```

**Complexity:** One input → multiple outputs. Domain-specific toolchain.

### UC-82: Font pipeline

**Scenario:** Source font to web-ready formats.

```bash
paraphase convert font.glyphs font.woff2
# Internally: Glyphs → UFO → OTF → WOFF2
```

**Complexity:** Domain-specific chain. May need subsetting, hinting.

### UC-83: Icon font generation

**Scenario:** SVG icons → icon font.

```bash
paraphase convert icons/*.svg icons.woff2 --as icon-font
```

**Complexity:** Multiple inputs → single output. Semantic: these SVGs are glyphs.

### UC-84: E-book chain

**Scenario:** Markdown to Kindle format.

```bash
paraphase convert book.md book.azw3
# Internally: Markdown → EPUB → MOBI → AZW3
```

**Complexity:** Long chain through intermediate formats.

### UC-85: Diagram rendering (text → image)

**Scenario:** Text DSL to image.

```bash
paraphase convert diagram.mmd diagram.png   # mermaid
paraphase convert graph.dot graph.svg        # graphviz
paraphase convert sequence.puml sequence.png # plantuml
```

**Complexity:** "Format" is text, but semantic interpretation produces graphics.
Converters need domain interpreters (mermaid-js, graphviz, plantuml).

### UC-86: Data visualization

**Scenario:** Data → chart → image.

```bash
paraphase convert data.csv chart.png --spec vega-lite.json
# Or: inlined
paraphase convert data.csv chart.png --chart bar --x date --y value
```

**Complexity:** Conversion requires specification/template. Data + spec → output.

### UC-87: Localization export

**Scenario:** Translation spreadsheet → multiple platform formats.

```bash
paraphase convert translations.xlsx locales/ --formats android,ios,json
# Outputs: locales/en.xml, locales/en.strings, locales/en.json, ...
```

**Complexity:** One input → multiple outputs × multiple formats. Tree output.

### UC-88: Compilation as conversion

**Scenario:** Source code → compiled output.

```bash
paraphase convert main.ts main.js          # typescript
paraphase convert paper.tex paper.pdf       # latex
paraphase convert notebook.ipynb report.html --execute  # jupyter
```

**Complexity:** Is this conversion or build? Requires toolchain (tsc, pdflatex, jupyter).
Execution may have side effects, network access, nondeterminism.

### UC-89: MIDI to audio

**Scenario:** MIDI → WAV requires external resource.

```bash
paraphase convert song.midi song.wav --soundfont piano.sf2
```

**Complexity:** Conversion requires auxiliary input (soundfont).
Not just file → file, but file + resource → file.

### UC-90: QR/Barcode generation

**Scenario:** Data → visual encoding.

```bash
paraphase convert --from text "https://example.com" qr.png
paraphase convert --from isbn "978-3-16-148410-0" barcode.svg
```

**Complexity:** Input is data/string, not file. Type is semantic (URL, ISBN), not format.

### UC-91: Asymmetric bidirectional

**Scenario:** Round-trip with quality loss.

```bash
paraphase convert doc.pdf doc.docx   # best-effort extraction
paraphase convert doc.docx doc.pdf   # faithful rendering
```

**Complexity:** Same types, but converters have different fidelity.
Graph edges have quality/lossiness weights.

### UC-92: Spreadsheet as database

**Scenario:** Excel → SQLite.

```bash
paraphase convert data.xlsx data.sqlite
paraphase convert data.sqlite data.xlsx
```

**Complexity:** Cross-domain (document ↔ database). Schema inference.

### UC-93: Video/GIF ↔ frames

**Scenario:** Explode video/gif to frames, or assemble frames to video/gif.

```bash
paraphase convert animation.gif frames/      # gif → directory of PNGs
paraphase convert video.mp4 frames/ --fps 30 # video → frames at specific rate
paraphase convert frames/*.png animation.gif --fps 15  # frames → gif
paraphase convert frames/*.png video.mp4 --fps 60      # frames → video
```

**Complexity:**
- 1 → N (explode) and N → 1 (assemble)
- Timing/fps metadata
- Frame ordering (filename sort? explicit?)

### UC-95: Batch import heterogeneous assets (game engine)

**Scenario:** Import a directory of mixed assets for a game engine, generating manifests.

```bash
paraphase import assets/ --target godot --output imported/
# Input: assets/
#   ├── player.png
#   ├── enemy.blend
#   ├── music.wav
#   └── config.json
#
# Output: imported/
#   ├── player.webp
#   ├── player.webp.import    # godot manifest
#   ├── enemy.glb
#   ├── enemy.glb.import
#   ├── music.ogg
#   ├── music.ogg.import
#   └── config.tres           # godot resource
```

**Complexity:**
- Heterogeneous input types - each file needs different conversion
- Target-specific output formats (godot wants webp, ogg, glb)
- Manifest generation per asset (.import files)
- Preserves directory structure
- May need target-specific metadata (import hints, flags)

### UC-94: Spritesheet ↔ individual sprites

**Scenario:** Pack sprites into sheet, or extract from sheet.

```bash
paraphase convert sprites/*.png spritesheet.png --pack --metadata sprites.json
paraphase convert spritesheet.png sprites/ --unpack --metadata sprites.json
```

**Complexity:**
- N → 1 with layout algorithm (packing)
- Metadata sidecar (positions, names)
- Round-trip depends on metadata preservation

---

## Observations

Patterns emerging from use cases:

### Pattern: Routing complexity

| Pattern | Example | Core needs |
|---------|---------|------------|
| Direct | json → yaml | Lookup |
| Multi-hop | markdown → pdf | Graph search |
| Conditional | only if width > 2048 | Predicate matching |
| Parameterized | yuv411 → yuv420p | Property-aware routing |

### Pattern: Input/Output cardinality

| Pattern | Example | Notes |
|---------|---------|-------|
| 1 → 1 | png → webp | Standard |
| 1 → N | kicad → gerber layers | Multi-output |
| N → 1 | svgs → icon font | Aggregation |
| N → N | tree conversion | Recursive |

### Pattern: External dependencies

| Pattern | Example | Notes |
|---------|---------|-------|
| Self-contained | json → yaml | Just data |
| Toolchain | latex → pdf | Needs pdflatex |
| Resource | midi → wav | Needs soundfont |
| Spec/template | csv → chart | Needs chart spec |

### Pattern: Conversion vs computation

| Pattern | Example | Notes |
|---------|---------|-------|
| Pure transform | png → jpg | Deterministic |
| Compilation | ts → js | Requires compiler |
| Execution | notebook → html | Runs code, side effects |
| Generation | text → qr code | Creates from data |

### Pattern: Fidelity

| Pattern | Example | Notes |
|---------|---------|-------|
| Lossless | png → png (resize) | Recoverable |
| Lossy | png → jpg | Information lost |
| Asymmetric | pdf ↔ docx | One direction faithful, other best-effort |

### Open questions surfaced

1. ~~**Is compilation in scope?**~~ **Yes.** TypeScript → JS, LaTeX → PDF, Sass → CSS are all in scope.
   - These require toolchains with their own dependencies
   - Goal: minimize mental overhead, unify interface regardless of whether it's "conversion" or "compilation"

2. **Multi-input conversions?** MIDI + soundfont → WAV, data + template → chart
   - Current model assumes single input
   - How to express auxiliary inputs?

3. **Multi-output conversions?** KiCad → gerber layers, i18n → locale files, video → frames
   - Single file → directory of files
   - How to express output structure?

4. **Is generation "conversion"?** Text → QR code, data → chart
   - Input isn't a "file format"
   - Type is semantic (URL, ISBN) not structural

5. **How to express fidelity?** PDF → DOCX is lossy, DOCX → PDF is not
   - Directed graph with edge weights?
   - User preference: `--prefer lossless`?

6. **Where's the line with bundling?**
   - Spritesheet packing: conversion? bundling?
   - Archive creation: conversion? bundling?
   - Webpack/esbuild territory: definitely out of scope?
   - Need clearer boundary
