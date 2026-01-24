# Document IR Specification

A specification for a universal document intermediate representation, designed to improve upon existing solutions like Pandoc.

## Problem Statement

Document conversion is a common need: Markdown to PDF, DOCX to HTML, etc. Current solutions make tradeoffs that lose information or limit interoperability.

**Use cases:**
- Normalize diverse documents to a single format (e.g., PDF)
- Convert between markup formats (Markdown, HTML, LaTeX)
- Extract content from complex formats (DOCX, PDF)
- Build documentation pipelines
- Power static site generators

## Prior Art Analysis

### Pandoc

Pandoc is the de facto standard for document conversion, supporting 40+ formats.

**Strengths:**
- Extensive format support
- Battle-tested, widely used
- Good CLI and library interfaces
- Active development
- Lua filter system for customization

**Weaknesses:**

| Issue | Description |
|-------|-------------|
| **Lossy by design** | AST is "least common denominator" - format-specific features lost on parse |
| **No layout layer** | AST is purely semantic - no page breaks, columns, positioning |
| **Poor roundtrip fidelity** | A→B→A loses information (DOCX styles, HTML classes, LaTeX macros) |
| **Simplified tables** | Complex tables (merged cells, nested content) get flattened |
| **References-only media** | Images are paths, not embedded - no unified resource handling |
| **Fixed schema** | Adding element types requires changing Pandoc itself |
| **No fidelity tracking** | No way to know what was lost in conversion |
| **Haskell-only core** | Bindings exist but core development requires Haskell |

**Pandoc's AST (simplified):**
```haskell
data Block
  = Para [Inline]
  | Header Int Attr [Inline]
  | CodeBlock Attr Text
  | BlockQuote [Block]
  | BulletList [[Block]]
  | OrderedList ListAttributes [[Block]]
  | Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot
  | Div Attr [Block]
  | ...

data Inline
  = Str Text
  | Emph [Inline]
  | Strong [Inline]
  | Link Attr [Inline] Target
  | Image Attr [Inline] Target
  | Code Attr Text
  | Span Attr [Inline]
  | ...
```

The fixed enum variants mean format-specific features either:
1. Get mapped to generic containers (Div/Span with attributes)
2. Get lost entirely

### unified.js Ecosystem

JavaScript ecosystem with separate ASTs for different domains.

**Components:**
- **mdast** - Markdown AST
- **hast** - Hypertext (HTML) AST
- **nlcst** - Natural language CST
- **xast** - XML AST

**Strengths:**
- Domain-specific ASTs preserve format semantics
- Clear transformation pipelines
- Plugin architecture
- Well-specified (documented specs)
- Large ecosystem of plugins

**Weaknesses:**
- JavaScript-only
- Multiple ASTs means multiple representations to handle
- Transforms between ASTs can still lose information
- No unified resource handling

### Other Solutions

| Tool | Approach | Limitation |
|------|----------|------------|
| LibreOffice | Native format handling | Heavy runtime, focused on Office formats |
| wkhtmltopdf | WebKit rendering | HTML input only |
| WeasyPrint | CSS layout engine | HTML/CSS input only |
| Typst | Own markup + PDF output | Single output format, own input format |
| rst2pdf | reStructuredText focus | Single input format |

## Proposed Architecture

### Core Principles

1. **Property bags over fixed schemas** - Elements carry extensible properties, not fixed fields
2. **Layered representation** - Separate semantic, style, and layout concerns
3. **Fidelity tracking** - Know what was lost, warn about it
4. **Embedded resources** - First-class handling of images, fonts, data
5. **Roundtrip-friendly** - Preserve source format info for better reconstruction

### Document Model

```rust
/// A document with content and embedded resources.
pub struct Document {
    /// Root content node.
    pub content: Node,
    /// Embedded resources (images, fonts, etc.).
    pub resources: ResourceMap,
    /// Document-level metadata.
    pub metadata: Properties,
    /// Source format information (for roundtrip fidelity).
    pub source: Option<SourceInfo>,
}

/// A content node in the document tree.
pub struct Node {
    /// Node type (e.g., "paragraph", "heading", "table").
    pub kind: NodeKind,
    /// Extensible properties for this node.
    pub props: Properties,
    /// Child nodes.
    pub children: Vec<Node>,
    /// Source location for error reporting.
    pub span: Option<Span>,
}

/// Node kind - open enum for extensibility.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeKind(pub String);

impl NodeKind {
    // Standard block kinds
    pub const DOCUMENT: &'static str = "document";
    pub const PARAGRAPH: &'static str = "paragraph";
    pub const HEADING: &'static str = "heading";
    pub const CODE_BLOCK: &'static str = "code_block";
    pub const BLOCKQUOTE: &'static str = "blockquote";
    pub const LIST: &'static str = "list";
    pub const LIST_ITEM: &'static str = "list_item";
    pub const TABLE: &'static str = "table";
    pub const TABLE_ROW: &'static str = "table_row";
    pub const TABLE_CELL: &'static str = "table_cell";
    pub const FIGURE: &'static str = "figure";
    pub const HORIZONTAL_RULE: &'static str = "horizontal_rule";

    // Standard inline kinds
    pub const TEXT: &'static str = "text";
    pub const EMPHASIS: &'static str = "emphasis";
    pub const STRONG: &'static str = "strong";
    pub const CODE: &'static str = "code";
    pub const LINK: &'static str = "link";
    pub const IMAGE: &'static str = "image";
    pub const LINE_BREAK: &'static str = "line_break";

    // Format-specific kinds (examples)
    pub const LATEX_MATH: &'static str = "latex:math";
    pub const HTML_DIV: &'static str = "html:div";
    pub const DOCX_COMMENT: &'static str = "docx:comment";
}
```

### Property System

Properties are key-value pairs that carry semantic, style, and format-specific data.

```rust
/// Property namespaces for organization.
pub mod prop {
    // Semantic properties (format-agnostic)
    pub const LEVEL: &str = "level";           // heading level
    pub const ORDERED: &str = "ordered";       // list ordering
    pub const LANGUAGE: &str = "language";     // code language
    pub const URL: &str = "url";               // link target
    pub const TITLE: &str = "title";           // link/image title
    pub const ALT: &str = "alt";               // image alt text
    pub const RESOURCE_ID: &str = "resource";  // embedded resource reference

    // Style properties (presentational)
    pub const STYLE_FONT: &str = "style:font";
    pub const STYLE_SIZE: &str = "style:size";
    pub const STYLE_COLOR: &str = "style:color";
    pub const STYLE_ALIGN: &str = "style:align";

    // Layout properties (positioning)
    pub const LAYOUT_PAGE_BREAK: &str = "layout:page_break";
    pub const LAYOUT_COLUMN: &str = "layout:column";
    pub const LAYOUT_FLOAT: &str = "layout:float";

    // Format-specific prefixes
    pub const HTML_PREFIX: &str = "html:";     // html:class, html:id
    pub const LATEX_PREFIX: &str = "latex:";   // latex:env, latex:macro
    pub const DOCX_PREFIX: &str = "docx:";     // docx:style, docx:revision
}
```

### Resource Management

Embedded resources (images, fonts, data files) are stored separately and referenced by ID.

```rust
pub type ResourceId = String;
pub type ResourceMap = HashMap<ResourceId, Resource>;

pub struct Resource {
    /// Original filename or identifier.
    pub name: Option<String>,
    /// MIME type.
    pub mime_type: String,
    /// Raw data.
    pub data: Vec<u8>,
    /// Resource metadata.
    pub metadata: Properties,
}

impl Document {
    /// Embed a resource and return its ID.
    pub fn embed(&mut self, resource: Resource) -> ResourceId;

    /// Get a resource by ID.
    pub fn resource(&self, id: &ResourceId) -> Option<&Resource>;

    /// Iterate over all resources.
    pub fn resources(&self) -> impl Iterator<Item = (&ResourceId, &Resource)>;
}
```

### Fidelity Tracking

Track what information was lost or transformed during conversion.

```rust
pub struct ConversionResult {
    pub document: Document,
    pub warnings: Vec<FidelityWarning>,
}

pub struct FidelityWarning {
    pub severity: Severity,
    pub kind: WarningKind,
    pub message: String,
    pub span: Option<Span>,
}

pub enum Severity {
    /// Information only, no data lost.
    Info,
    /// Minor formatting may differ.
    Minor,
    /// Significant information lost.
    Major,
    /// Conversion may be incorrect.
    Error,
}

pub enum WarningKind {
    /// Property not supported by target format.
    UnsupportedProperty(String),
    /// Node kind not supported, using fallback.
    UnsupportedNode(String),
    /// Complex structure simplified.
    Simplified(String),
    /// Resource could not be embedded.
    ResourceFailed(ResourceId),
    /// Format-specific feature lost.
    FeatureLost(String),
}
```

### Parser/Emitter Traits

```rust
/// Parse a format into the document IR.
pub trait Parser: Send + Sync {
    /// Formats this parser can handle.
    fn formats(&self) -> &[&str];

    /// Parse bytes into a document.
    fn parse(&self, input: &[u8], options: &ParseOptions) -> Result<ConversionResult, ParseError>;
}

/// Emit the document IR to a format.
pub trait Emitter: Send + Sync {
    /// Formats this emitter can produce.
    fn formats(&self) -> &[&str];

    /// Emit a document to bytes.
    fn emit(&self, doc: &Document, options: &EmitOptions) -> Result<EmitResult, EmitError>;
}

pub struct EmitResult {
    pub data: Vec<u8>,
    pub warnings: Vec<FidelityWarning>,
}

/// Transform a document (same IR, modified content).
pub trait Transformer: Send + Sync {
    fn name(&self) -> &str;
    fn transform(&self, doc: Document) -> Result<Document, TransformError>;
}
```

### Standard Transformers

```rust
// Examples of useful transformers

/// Resolve relative URLs to absolute.
pub struct ResolveUrls { base: Url }

/// Embed external images into the document.
pub struct EmbedImages { fetcher: Box<dyn Fetcher> }

/// Strip style/layout properties (semantic only).
pub struct StripFormatting;

/// Apply a style sheet.
pub struct ApplyStyles { stylesheet: Stylesheet }

/// Extract plain text content.
pub struct ExtractText;

/// Validate document structure.
pub struct Validate { schema: Schema }
```

## Format Support

### Tier 1: Pure Rust (no external deps)

| Format | Parse | Emit | Notes |
|--------|-------|------|-------|
| Markdown (CommonMark) | Yes | Yes | via `pulldown-cmark` or custom |
| Markdown (GFM) | Yes | Yes | via `comrak` |
| HTML | Yes | Yes | via `html5ever` / `markup5ever` |
| Plain text | Yes | Yes | trivial |
| JSON (serialized IR) | Yes | Yes | for tooling interop |

### Tier 2: Pure Rust (complex)

| Format | Parse | Emit | Notes |
|--------|-------|------|-------|
| PDF | Partial | Yes | Parse: `pdf-extract`. Emit: `typst` or `printpdf` |
| EPUB | Yes | Yes | ZIP + HTML, can reuse HTML parser |
| LaTeX | Partial | Yes | Parse subset, emit full |
| reStructuredText | Yes | Yes | Custom parser needed |

### Tier 3: External tools

| Format | Tool | Notes |
|--------|------|-------|
| DOCX, XLSX, PPTX | LibreOffice | Full fidelity Office support |
| ODT, ODS, ODP | LibreOffice | Native LibreOffice formats |
| RTF | LibreOffice | Legacy format |
| PDF (complex) | LibreOffice | Better than pure Rust for complex PDFs |

## API Examples

### Basic Conversion

```rust
use docir::{Document, Parser, Emitter};
use docir_markdown::MarkdownParser;
use docir_html::HtmlEmitter;

let parser = MarkdownParser::new();
let emitter = HtmlEmitter::new();

let result = parser.parse(markdown_bytes, &ParseOptions::default())?;

for warning in &result.warnings {
    eprintln!("Warning: {}", warning.message);
}

let output = emitter.emit(&result.document, &EmitOptions::default())?;
```

### With Transformations

```rust
use docir::{Document, Pipeline};
use docir_markdown::MarkdownParser;
use docir_pdf::PdfEmitter;
use docir_transforms::{EmbedImages, ApplyStyles};

let pipeline = Pipeline::new()
    .parser(MarkdownParser::new())
    .transform(EmbedImages::new(http_fetcher))
    .transform(ApplyStyles::new(stylesheet))
    .emitter(PdfEmitter::new());

let output = pipeline.run(input_bytes)?;
```

### Inspecting Documents

```rust
use docir::{Document, Node, NodeKind};

fn count_headings(doc: &Document) -> usize {
    count_nodes(&doc.content, |n| n.kind.0 == NodeKind::HEADING)
}

fn extract_links(doc: &Document) -> Vec<String> {
    collect_props(&doc.content, NodeKind::LINK, "url")
}

fn word_count(doc: &Document) -> usize {
    extract_text(&doc.content)
        .split_whitespace()
        .count()
}
```

### Building Documents Programmatically

```rust
use docir::{Document, Node, NodeKind, Properties};
use docir::props;

let doc = Document::new()
    .with_metadata(props! {
        "title" => "My Document",
        "author" => "Alice",
    })
    .with_content(Node::new(NodeKind::DOCUMENT).children(vec![
        Node::new(NodeKind::HEADING)
            .prop("level", 1)
            .child(Node::text("Introduction")),
        Node::new(NodeKind::PARAGRAPH)
            .child(Node::text("This is the first paragraph.")),
        Node::new(NodeKind::PARAGRAPH).children(vec![
            Node::text("Visit "),
            Node::new(NodeKind::LINK)
                .prop("url", "https://example.com")
                .child(Node::text("our website")),
            Node::text(" for more."),
        ]),
    ]));
```

## Comparison with Pandoc

| Aspect | Pandoc | This Design |
|--------|--------|-------------|
| Schema | Fixed Haskell ADT | Open `NodeKind` + Properties |
| Format-specific data | Div/Span attrs, mostly lost | Namespaced properties preserved |
| Style information | Not represented | `style:*` properties |
| Layout information | Not represented | `layout:*` properties |
| Embedded resources | External references | First-class `ResourceMap` |
| Fidelity tracking | None | `ConversionResult.warnings` |
| Roundtrip support | Poor | Source format info preserved |
| Extensibility | Requires Haskell changes | Add properties/node kinds freely |
| Language | Haskell | Rust (with C API possible) |

## Project Structure

```
docir/
├── docir/              # Core IR types and traits
├── docir-markdown/     # Markdown parser/emitter
├── docir-html/         # HTML parser/emitter
├── docir-pdf/          # PDF emitter (via typst/printpdf)
├── docir-latex/        # LaTeX parser/emitter
├── docir-docx/         # DOCX via LibreOffice bridge
├── docir-transforms/   # Standard transformers
├── docir-cli/          # Command-line tool
└── docir-wasm/         # WebAssembly bindings
```

## Integration with Paraphrase

Paraphrase would use this IR through a thin integration crate:

```rust
// paraphase-document/src/lib.rs

use paraphase::{Converter, Registry};
use docir::{Parser, Emitter};

/// Register document converters with Paraphrase.
pub fn register_all(registry: &mut Registry) {
    // For each parser/emitter combination, register a converter
    // that goes: input format → Document IR → output format
}
```

Paraphrase handles:
- Routing (which converter to use)
- Execution (memory management, parallelism)
- Property matching

The document IR handles:
- Parsing formats to IR
- Transforming IR
- Emitting IR to formats
- Fidelity tracking

## Open Questions

1. **Name**: `docir`? `unidoc`? `markup`? Something else?

2. **Inline vs Block**: Should we distinguish inline/block at the type level, or just by convention via `NodeKind`?

3. **Text handling**: Should text be a special `Node` kind, or a separate `Content` enum variant?

4. **Streaming**: Should parsing/emitting support streaming for large documents?

5. **Schema validation**: Should there be optional schemas that validate node/property combinations?

6. **Binary formats**: How to handle truly binary formats (images in documents) - always extract, or preserve structure?

## Next Steps

1. **Prototype core types** - `Document`, `Node`, `Properties`, `Resource`
2. **Implement Markdown parser** - Using `pulldown-cmark`, map to IR
3. **Implement HTML emitter** - Straightforward from IR
4. **Test roundtrip** - Markdown → IR → HTML → IR → Markdown
5. **Add fidelity tracking** - Warnings when information lost
6. **PDF output** - Via Typst or printpdf
7. **Evaluate and iterate** - Does the design hold up?
