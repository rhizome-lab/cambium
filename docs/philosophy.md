# Philosophy

Core design principles for Cambium.

## Cambium is a Pipeline Orchestrator

**Founding use case:** Game asset conversion - textures, meshes, audio, configs all need processing through diverse tools with inconsistent interfaces.

**Core job:** Unification. One vocabulary, one interface, many backends.

```bash
# Without cambium: learn each tool's flags
ffmpeg -i video.mp4 -crf 23 video.webm
cwebp -q 80 image.png -o image.webp
gltf-pipeline -i model.glb -o model.glb --draco.compressionLevel 7

# With cambium: one interface
cambium convert video.mp4 video.webm --quality 80
cambium convert image.png image.webp --quality 80
cambium convert model.glb model.glb --compress draco
```

Cambium normalizes options (`--quality` maps to `-crf`, `-q`, `--draco.compressionLevel` etc.) so users learn one vocabulary.

## Type-Driven, Not Command-Driven

The fundamental insight: declare **what you have** and **what you want**, not **how to get there**.

```
# Task runner (make/just) - you specify the command
ffmpeg -i video.mp4 -c:v libx264 video.mkv

# Cambium - you specify the types
cambium convert video.mp4 --to mkv
# OR: inferred from extension
cambium convert video.mp4 video.mkv
```

The system maintains a graph of registered converters and finds the path.

## Converter Graph

Formats are nodes. Converters are edges. Cambium finds shortest paths.

```
         ┌─────┐
         │ PNG │
         └──┬──┘
            │
┌─────┐  ┌──▼──┐  ┌─────┐
│ SVG ├──► RGB ◄──┤ JPG │
└─────┘  └──┬──┘  └─────┘
            │
         ┌──▼──┐
         │ WebP│
         └─────┘
```

If you have PNG and want WebP, Cambium finds: `PNG → RGB → WebP`

Converters can be:
- Built-in (common formats)
- Plugins (user-registered)
- Shelling out (wrapping existing tools)

## Types, Not Extensions

File extensions are hints, not truth. Cambium uses content-aware type detection:

```
# These are equivalent:
cambium convert data.json --to yaml
cambium convert --from json data --to yaml

# Type detection for ambiguous files:
cambium convert config --to toml  # sniffs content to determine source type
```

Types form a hierarchy:
```
Data
├── Structured
│   ├── JSON
│   ├── YAML
│   └── TOML
├── Document
│   ├── Markdown
│   ├── HTML
│   └── PDF
└── Media
    ├── Image
    │   ├── PNG
    │   └── JPG
    └── Audio
        ├── WAV
        └── MP3
```

## Intermediate Representations

Some conversions go through a canonical IR:

| Domain | IR | Why |
|--------|-----|-----|
| Config | In-memory tree (serde_value?) | Lossless between JSON/YAML/TOML |
| Document | AST (markdown-like) | Semantic structure preserved |
| Image | Raw pixels / GPU texture | Universal bitmap interchange |
| Mesh | Half-edge or indexed | Topology-aware transforms |

Direct converters can bypass IR when lossless (e.g., JSON ↔ YAML).

## Pipelines

Chain transforms explicitly when needed:

```
cambium pipe input.md \
  | markdown-to-html \
  | minify-html \
  | gzip \
  > output.html.gz
```

Or declaratively:
```yaml
# cambium.yaml
pipelines:
  docs:
    input: "docs/*.md"
    steps:
      - markdown-to-html
      - minify-html
    output: "dist/{name}.html"
```

## Incremental by Default

Track file mtimes and content hashes. Only reconvert when inputs change.

```
cambium convert *.md --to html  # first run: converts all
cambium convert *.md --to html  # second run: "nothing to do"
# edit one file
cambium convert *.md --to html  # third run: converts only changed file
```

## Ecosystem, Not Monolith

Unlike pandoc/ffmpeg (which bundle everything), Cambium is:
- **Core**: graph traversal, type detection, CLI
- **Plugins**: actual converters, registered at runtime

```
cambium plugin add cambium-images   # adds PNG, JPG, WebP, etc.
cambium plugin add cambium-docs     # adds Markdown, HTML, PDF
cambium plugin add my-custom-format # user-defined
```

Plugins declare:
- Types they handle (input/output)
- Converter functions
- Optional: quality/speed tradeoffs, lossy vs lossless
