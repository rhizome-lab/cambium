# Use Cases

Concrete scenarios to drive type system design. Grouped by domain and complexity.

## Config / Structured Data

### UC-01: JSON ↔ YAML ↔ TOML

**Scenario:** Convert between config formats losslessly.

```bash
cambium convert config.json config.yaml
cambium convert config.yaml config.toml
```

**Type complexity:** Trivial - flat types, no parameters.
- `json`, `yaml`, `toml` are distinct types
- All share an IR (in-memory tree)
- Lossless round-trip (modulo comments, formatting)

### UC-02: JSON with schema validation

**Scenario:** Convert JSON, but only if it matches a schema.

```bash
cambium convert --validate schema.json input.json output.yaml
```

**Type complexity:** Type + constraint?
- `json` vs `json[schema=foo.json]`?
- Or is validation a separate step, not a type?

### UC-03: Minify / pretty-print

**Scenario:** Same format in, same format out, different representation.

```bash
cambium convert input.json --to json --minify
cambium convert input.json --to json --pretty
```

**Type complexity:** Same type, different output options.
- Is `json:minified` a different type than `json:pretty`?
- Or just converter options?

---

## Images

### UC-10: PNG → WebP

**Scenario:** Simple format conversion.

```bash
cambium convert photo.png photo.webp
```

**Type complexity:** Trivial - flat types.

### UC-11: Batch resize + convert

**Scenario:** Convert all PNGs to WebP at max 1024px width.

```bash
cambium convert "*.png" --to webp --max-width 1024
```

**Type complexity:** Parameters on output.
- `webp[width<=1024]` as target type?
- Or just converter options?

### UC-12: Only convert large images

**Scenario:** Convert images, but only if they exceed a size threshold.

```bash
cambium convert "*.png" --to webp --if "width > 2048"
```

**Type complexity:** Predicate on input.
- Input type: `png[width>2048]`
- Needs content inspection to evaluate predicate

### UC-13: Lossy vs lossless WebP

**Scenario:** Explicitly choose lossy or lossless encoding.

```bash
cambium convert photo.png photo.webp --lossy --quality 80
cambium convert diagram.png diagram.webp --lossless
```

**Type complexity:** Same output format, different encoding.
- `webp:lossy` vs `webp:lossless`?
- Or converter option?

---

## Video

### UC-20: Container conversion

**Scenario:** Change container without re-encoding.

```bash
cambium convert video.mkv video.mp4 --copy-streams
```

**Type complexity:** Container vs codec distinction.
- Type is container: `mkv`, `mp4`
- Codec is... parameter? Separate type?

### UC-21: Pixel format conversion

**Scenario:** Convert YUV411 to YUV420p for compatibility.

```bash
cambium convert input.mp4 output.mp4 --pixfmt yuv420p
# or: only convert if source is yuv411
cambium convert input.mp4 output.mp4 --from-pixfmt yuv411 --to-pixfmt yuv420p
```

**Type complexity:** This is the motivating example.
- `video[pixfmt=yuv411]` → `video[pixfmt=yuv420p]`
- Requires inspecting input to know its pixfmt
- Routing depends on parameter values

### UC-22: Resolution targeting

**Scenario:** Downscale 4K to 1080p, leave 1080p alone.

```bash
cambium convert "*.mp4" --to mp4 --max-height 1080
```

**Type complexity:** Conditional transformation.
- Only applies if `height > 1080`
- Output guarantees `height <= 1080`

### UC-23: Codec selection

**Scenario:** Re-encode H.264 to H.265/HEVC.

```bash
cambium convert input.mp4 output.mp4 --codec hevc
```

**Type complexity:** Codec as type or parameter?
- `video/mp4:h264` → `video/mp4:hevc`?
- Or `mp4` → `mp4` with codec option?

---

## Audio

### UC-30: WAV → MP3

**Scenario:** Lossy compression.

```bash
cambium convert audio.wav audio.mp3 --bitrate 320k
```

**Type complexity:** Trivial + quality parameter.

### UC-31: Sample rate conversion

**Scenario:** Resample 48kHz to 44.1kHz for CD.

```bash
cambium convert audio.wav output.wav --samplerate 44100
```

**Type complexity:** Same format, different sample rate.
- `wav[samplerate=48000]` → `wav[samplerate=44100]`?

### UC-32: Channel downmix

**Scenario:** 5.1 surround to stereo.

```bash
cambium convert surround.wav stereo.wav --channels 2
```

**Type complexity:** Channel layout as type parameter?

---

## Documents

### UC-40: Markdown → HTML

**Scenario:** Basic document conversion.

```bash
cambium convert readme.md readme.html
```

**Type complexity:** Trivial.

### UC-41: Markdown → PDF (multi-hop)

**Scenario:** No direct converter, needs intermediate.

```bash
cambium convert readme.md readme.pdf
# Internally: md → html → pdf, or md → latex → pdf
```

**Type complexity:** Graph traversal, multiple paths possible.
- Which path is better? User preference? Plugin availability?

### UC-42: Markdown with frontmatter

**Scenario:** YAML frontmatter should be preserved/extracted.

```bash
cambium convert post.md post.html --preserve-frontmatter
cambium convert post.md --extract-frontmatter > metadata.yaml
```

**Type complexity:** Substructure within format.
- `markdown` vs `markdown+frontmatter`?

### UC-43: Pandoc-style format variants

**Scenario:** GitHub-flavored markdown vs CommonMark vs MultiMarkdown.

```bash
cambium convert --from gfm input.md output.html
cambium convert --from commonmark input.md output.html
```

**Type complexity:** Variants of same base format.
- `markdown:gfm`, `markdown:commonmark`?
- Or separate types `gfm`, `commonmark`?

---

## 3D / Mesh

### UC-50: OBJ → glTF

**Scenario:** Convert mesh formats.

```bash
cambium convert model.obj model.gltf
```

**Type complexity:** Trivial if just format.

### UC-51: Mesh optimization

**Scenario:** Decimate mesh, merge vertices.

```bash
cambium convert model.obj model.gltf --decimate 0.5 --merge-threshold 0.001
```

**Type complexity:** Transformation during conversion.
- Are these converter options or type parameters?

### UC-52: Extract textures

**Scenario:** glTF with embedded textures → glTF with external textures.

```bash
cambium convert model.glb model.gltf --extract-textures ./textures/
```

**Type complexity:** Same format, different embedding mode.
- `gltf:binary` vs `gltf:separate`?

---

## Archives / Containers

### UC-55: Basic archive conversion

**Scenario:** Convert between archive formats.

```bash
cambium convert archive.zip archive.tar.gz
cambium convert archive.rar archive.7z
```

**Type complexity:** Straightforward format conversion.

### UC-56: Nested/compound formats

**Scenario:** `.tar.gz` is gzip(tar), not a single format.

```bash
cambium convert archive.tar.gz archive.tar.xz  # recompress only
cambium convert archive.tar.gz archive.zip     # unpack tar, repack as zip
```

**Type complexity:** Layered types.
- `tar.gz` = `gzip[inner=tar]`?
- `tar.xz` = `xz[inner=tar]`?
- Converter needs to know whether to unwrap fully or just re-compress outer layer

### UC-57: "Secretly archives" formats

**Scenario:** JAR, DOCX, APK, Unity packages, Godot PCK, some EXEs are actually ZIPs.

```bash
cambium convert app.jar contents/         # extract
cambium convert contents/ app.jar         # repack
cambium convert game.pck assets/          # godot package
cambium convert document.docx document/   # office xml
```

**Type complexity:** Same underlying format, different semantics.
- `jar` ≈ `zip[convention=java]`
- `docx` ≈ `zip[convention=ooxml]`
- `apk` ≈ `zip[convention=android]`
- Should cambium know these equivalences?

### UC-58: Extract + convert contents

**Scenario:** Extract archive, convert contents, repack.

```bash
cambium convert textures.zip textures-optimized.zip \
  --convert-contents "*.png -> webp" --quality 80
```

**Type complexity:** Recursive conversion.
- Pipeline into archive contents
- Preserve structure

### UC-59: Self-extracting archives

**Scenario:** EXE that's secretly a ZIP with a stub.

```bash
cambium convert installer.exe contents/    # extract
cambium convert contents/ installer.exe    # create sfx
```

**Type complexity:** Format detection vs extension.
- `.exe` could be PE binary or SFX archive
- Content sniffing required

---

## Code / AST

### UC-60: Syntax highlighting

**Scenario:** Source code → HTML with highlighting.

```bash
cambium convert main.rs main.html --highlight
```

**Type complexity:** Source language detection.
- Type is `rust`, `python`, etc.? Or generic `source`?

### UC-61: AST extraction

**Scenario:** Parse code, output AST as JSON.

```bash
cambium convert main.rs ast.json --format tree-sitter
```

**Type complexity:** Multiple output formats for AST.
- `rust` → `ast:tree-sitter` vs `ast:syn`?

---

## Cross-Domain

### UC-70: Pipeline composition

**Scenario:** Chain multiple conversions explicitly.

```bash
cambium pipe input.md \
  | markdown-to-html \
  | minify-html \
  | gzip \
  > output.html.gz
```

**Type complexity:** Explicit pipeline, no routing.

### UC-71: Automatic multi-hop

**Scenario:** System finds path through graph.

```bash
cambium convert input.md output.pdf
# System determines: md → html → pdf
```

**Type complexity:** Graph traversal.

### UC-72: Constrained path

**Scenario:** User constrains intermediate formats.

```bash
cambium convert input.md output.pdf --via latex
# Forces: md → latex → pdf
```

**Type complexity:** Path constraints.

---

## Observations

*To be filled in after reviewing use cases.*

Questions each use case raises:
- Is this a type distinction or a converter option?
- Does routing need to know this, or just the converter?
- Can the system inspect input to determine parameters?

Patterns emerging:
- ???
