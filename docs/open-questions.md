# Open Questions

Unresolved design decisions for Cambium.

## Core Model

### How do converters specify cost/quality?

When multiple paths exist (e.g., `PNG → JPG` direct vs `PNG → RGB → JPG`), how to choose?

Options:
1. **Shortest path** - fewest hops
2. **Weighted edges** - converters declare cost (speed? quality loss?)
3. **User hint** - `--prefer lossless` or `--prefer fast`

### Are types flat or hierarchical?

Flat: `png`, `jpg`, `webp` are all distinct
Hierarchical: `image/png`, `image/jpg` share parent `image`

Hierarchy enables:
- Wildcards: "convert all images to webp"
- Fallback converters: "anything → image/png"

But adds complexity.

### How to handle parameterized types?

`video/mp4` at 1080p vs 4K - same type or different?
`yuv411` vs `yuv420p` - same "video" type with different pixel format?

This quickly becomes expressive:
```
video[pixfmt=yuv411] → video[pixfmt=yuv420p]
image[width>4096] → image[width<=4096]
audio[samplerate=48000] → audio[samplerate=44100]
```

Options spectrum:

1. **Flat types, parameters as options** (simplest)
   - Types: `mp4`, `mkv`, `png`
   - Parameters passed to converter: `convert(&input, json!({"pixfmt": "yuv420p"}))`
   - Graph is simple, converter handles params
   - Con: can't express "only convert yuv411 sources"

2. **Type + variant** (middle ground)
   - Types: `video/mp4`, `video/mp4:yuv420p`
   - Explicit variants for common cases
   - Con: combinatorial explosion (resolution × pixfmt × codec × ...)

3. **Type + constraints** (most expressive)
   - Types: `video[container=mp4, pixfmt=yuv411, width=1920]`
   - Converters declare: `from: video[pixfmt=yuv411], to: video[pixfmt=yuv420p]`
   - Graph traversal becomes constraint matching
   - Con: complexity, need constraint solver

4. **Type + traits** (capability-based)
   - Types have capabilities: `{id: "mp4", traits: ["video", "seekable", "lossy"]}`
   - Converters require/provide traits
   - More about "what can I do with this" than exact format
   - Con: doesn't handle numeric constraints (resolution, bitrate)

**Needs further design work.** Questions to resolve:
- What parameters actually matter for routing vs. just converter options?
- Is constraint solving worth the complexity?
- Can we start simple (option 1) and evolve, or does that paint us into a corner?
- What does prior art do? (ffmpeg filters, imagemagick delegates, nix derivations)

## Plugin System

*Plugin format decided: C ABI dynamic libraries. See architecture-decisions.md #001.*

### Plugin versioning

How to handle ABI compatibility?
- Strict version matching (plugin must match exact cambium version)?
- Semver ranges?
- API version number in plugin (current approach in ADR)?

### Plugin dependencies

Can plugins depend on other plugins?
- Plugin A provides `foo → bar`, Plugin B provides `bar → baz`
- What if Plugin B is missing? Graceful degradation or error?

## Incremental Builds

### What's the caching granularity?

Options:
1. **File-level** - mtime/hash per file
2. **Content-addressed** - hash outputs, reuse across projects
3. **Fine-grained** - track dependencies within files

### Where does cache live?

- `.cambium/cache/` in project?
- Global `~/.cache/cambium/`?
- Both with hierarchy?

## CLI Design

### Primary interface

```bash
# Option A: subcommands
cambium convert input.md output.html
cambium pipe input.md | step1 | step2 > output.html
cambium watch src/ --to dist/

# Option B: implicit
cambium input.md output.html  # infers "convert"
cambium input.md --to html    # output to stdout or inferred name

# Option C: make-like
cambium build  # reads cambium.toml, builds all targets
```

### How explicit should type annotation be?

```bash
# Fully inferred
cambium convert data output.yaml

# Explicit source type
cambium convert --from json data output.yaml

# Explicit both
cambium convert --from json --to yaml data output
```

## Integration with Resin/Rhizome

*Library-first decided. See architecture-decisions.md #002.*

### Shared types with Resin?

Do Cambium's `Image`, `Mesh`, etc. share definitions with Resin?
Or is Cambium format-agnostic and Resin provides domain IRs?

Options:
1. **Cambium is format-only** - knows `png`, `obj`, not `Image`, `Mesh`
2. **Shared IR crate** - `rhizome-types` used by both
3. **Cambium defines IRs** - Resin depends on cambium's `Image` type
