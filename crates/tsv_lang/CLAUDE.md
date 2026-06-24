# tsv_lang

> Language-agnostic foundation crate for `tsv`

All language crates (tsv_ts, tsv_css, tsv_svelte) depend on tsv_lang. It provides the shared primitives for parsing, formatting, and AST manipulation. Zero external dependencies (only std).

## Modules

Each module's visibility (in parens) reflects `pub use`-only modules (private) vs directly-imported modules (`pub mod`, used as `tsv_lang::doc::{...}` etc.).

- `span` (`span.rs`, private) — `Span { start: u32, end: u32 }` — compact source positions
- `location` (`location.rs`, private) — `LocationTracker` — lazy line/column via O(log n) binary search
- `error` (`error.rs`, private) — `ParseError` with context extraction and caret formatting
- `config` (`config.rs`, private) — `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts + `EmbedContext` / `LayoutMode` (no runtime config)
- `doc` (`doc/*.rs`, pub) — Document builder — arena-based Prettier-compatible IR
- `comment` (`comment.rs`, private) — Comment type, classification, and O(log n) range lookup
- `printing` (`printing.rs`, pub) — String literal formatting, same-line detection, visual width
- `source_scan` (`source_scan.rs`, pub) — Trivia-aware source scanning: the `skip_trivia` cursor plus the `find_char` / `find_keyword` / `rfind_keyword` delimiter/keyword finders (skipping JS/CSS comments + strings) and the `is_regex_start` / `skip_regex_literal` regex helpers (the one piece of `/`-disambiguation `skip_trivia` deliberately leaves out, since it needs backward token lookback). The single chokepoint for re-scanning source between AST nodes — used by AST conversion, all three printers, the Svelte parser (binding/declaration scans + the `{…}` brace matcher), and the TS parser's arrow-vs-paren / type-args lookahead
- `interner` (`interner.rs`, private) — String interning traits (`SymbolResolver`, `InfallibleResolve`); implements `doc::TextResolver`
- `escapes` (`escapes.rs`, private) — Escape sequence handling (quote swapping) — used internally by `printing`
- `json` (`json.rs`, private) — `estimated_json_capacity` — pre-size heuristic for public-AST JSON serialization buffers
- `output` (`output.rs`, private) — `OutputBuffer` — string building with column tracking
- `parser` (`parser.rs`, private) — `PeekData<K>` — shared lookahead token cache

## Doc Builder

The doc builder is the core of the formatting architecture. Language printers build declarative doc trees; the shared renderer decides layout based on print width.

### Key Types

- **`DocArena`** — Contiguous storage for all doc nodes. Heuristic capacity: ~4 nodes per source byte.
- **`DocId`** (`u32`) — Lightweight, `Copy` handle into the arena. No cloning, no recursive Drop.
- **`DocBuf`** (`SmallVec<[DocId; 8]>`) — Shared stack buffer for assembling a node's doc parts before `concat()` / `fill()`. Most nodes have only a handful of parts, so the common case stays off the heap; larger nodes spill. Used by all language printers (the TS chain / binary-operator printers, the Svelte template printer) as the single canonical doc-parts buffer type.
- **`DocNode`** — Node variants: `Text`, `Line`, `Indent`, `Dedent`, `Group`, `IfBreak`, `Concat`, `Fill`, etc.
- **`DocText`** — Three variants: `Static(&'static str)` (punctuation/keywords), `Owned(String)` (dynamic), `Symbol(u32)` (deferred resolution via interner).
- **`LineKind`** — `Normal` (space in flat, newline in break), `Soft` (nothing in flat), `Hard` (always newline), `Literal` (newline without indent).

### Builder API Categories

All methods take `&self` (interior mutability via `RefCell`):

- Text — `text()`, `text_owned()`, `empty()`, `symbol()`
- Lines — `line()`, `softline()`, `hardline()`, `literalline()`
- Structure — `group()`, `group_break()`, `indent()`, `dedent()`, `align()`
- Conditionals — `if_break()`, `indent_if_break()`, `conditional_group()`
- Sequences — `concat()`, `fill()`, `join()`, `join_doc()`
- Context — `with_context()`
- Line suffix — `line_suffix()`, `line_suffix_boundary()`, `break_parent()`
- Convenience — `wrap()`, `parens()`, `brackets()`, `braces()`
- Inspection — `will_break()`, `has_forced_break()`
- Diagnostics — `line_comment_text_owned()` (tags `//` text for the swallow check)

The `doc::swallow` module is a render-time guard against the
line-comment-swallow bug class (a `//` emitted inline runs to EOL and consumes
the following token). It lives behind the **`swallow_check` cargo feature** (off
by default, like tsv_ts's `convert`), so production builds compile it out
entirely — no `DocArena` side-set, no render hook; `line_comment_text_owned`
collapses to `text_owned`. With the feature, `set_swallow_check(true)` arms it
and the renderer (via `SwallowTracker`) records every swallow into a thread-local
sink drained by `take_swallow_reports()`. Output-neutral. `tsv_debug` enables the
feature to drive `tsv_debug swallow_audit`.

### Rendering Pipeline

```
Language Printer builds DocId tree
        ↓
arena_fits_with_lookahead()  — check if group fits in remaining width
        ↓
arena_print_doc*()           — render doc tree to formatted string
```

**Rendering variants** (6 total):

- `arena_print_doc()` — standard (column 0, no resolver)
- `arena_print_doc_flat_resolved()` — render in flat mode (no group breaking)
- `arena_print_doc_at_column()` — mid-line start (for Svelte template expressions)
- `arena_print_doc_with_indent()` — explicit indent level
- `arena_print_doc_with_indent_resolved()` — full control
- `arena_print_doc_with_indent_resolved_preserve_whitespace()` — for HTML pre/textarea

## How Language Crates Use tsv_lang

### Parsing

```rust
// Language parsers use:
use tsv_lang::{ParseError, Result, Span};
use tsv_lang::parser::PeekData;           // Lookahead caching
use tsv_lang::location::LocationTracker;  // For convert_ast()
use tsv_lang::comment::Comment;           // Collected during parsing

// Errors enriched with context:
parser.parse().map_err(|e| e.with_context(source))
```

### Formatting

```rust
// Create arena, build doc tree, render:
let arena = DocArena::for_source(source); // sized for source.len()
let mut printer = Printer::new(&arena, interner, source, &comments, &line_breaks, config);
printer.print_program(&program);
let output = printer.into_string();
```

### AST Conversion

```rust
// Internal AST → Public JSON AST:
let tracker = LocationTracker::new(source);
let public = convert_program(&program, source, &tracker, Schema::Acorn);
// Use Schema::SvelteScript when converting a Svelte non-lang="ts" <script>
// (Svelte's parser omits importKind/exportKind=value and always emits
// `attributes` on import/export declarations).
```

`Schema` is defined in `tsv_ts::ast::convert::Schema`, not in tsv_lang — see [../tsv_ts/CLAUDE.md §Distinctives](../tsv_ts/CLAUDE.md#distinctives).

## Comment Utilities

See [../../CLAUDE.md §Comment Handling](../../CLAUDE.md#comment-handling-detached-model) for the detached model rationale and the `Comment` struct.

### Lookup Functions

- `comments_in_range()` — Find comments between two positions (O(log n))
- `comments_after()` — Iterate comments at or after a position (O(log n))
- `find_first_comment_from()` — Binary-search index of first comment with `span.start >= pos`
- `classify_comment()` — Classify as Trailing, LeadingOwnLine, or LeadingInline
- `classify_comment_fast()` — Same but using precomputed line breaks (faster)
- `ClassifiedComments::from_range()` — Batch classify all 4 categories in one pass (with precomputed line breaks)
- `has_comments_in_range()` — Quick existence check
- `has_line_comments_in_range()` — Existence check restricted to line comments
- `has_multiline_block_comments_in_range()` — Existence check for multi-line block comments (force expansion)

### Directive Recognition

`is_format_ignore_directive()` / `is_format_ignore_range_start()` / `is_format_ignore_range_end()` are the single source of truth for the format-suppression directive set — the tsv-native `format-ignore` family plus prettier's `prettier-ignore` family (drop-in compat). Each operates on trimmed comment text and is called by all three language printers (`tsv_ts`, `tsv_css`, `tsv_svelte`), since the comment types differ across crates. See [docs/directives.md](../../docs/directives.md) and [docs/conformance_prettier.md §Format-ignore directive](../../docs/conformance_prettier.md#format-ignore-directive).

## Interner Traits

String interning deduplicates identifiers across all languages in a file. Symbols flow from parser through doc builder to renderer:

- `TextResolver` — `resolve(id: u32) -> &str` — resolve symbol during rendering
- `SymbolResolver` — `resolve_symbol()`, `with_resolved_symbol()` — zero-allocation hot path
- `InfallibleResolve` — `resolve_infallible()` — panic-free resolution
- `SymbolToU32` — Convert `DefaultSymbol` to `u32` for doc builder `Symbol` variant
- `SharedInterner` — Type alias `Rc<RefCell<DefaultStringInterner>>` — shared interner handle

**Pattern**: Parser interns identifiers → AST stores `DefaultSymbol` → printer calls `arena.symbol(sym.to_u32())` → renderer resolves via `TextResolver` at print time.

## Config Types

`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts, `EmbedContext`, and `LayoutMode` are covered in [../../CLAUDE.md §Internal Configuration](../../CLAUDE.md#internal-configuration-rust-library-only). tsv has no runtime configuration.

**Embedding knobs**: `base_indent_offset` and `first_line_offset` are how tsv_svelte tells tsv_ts/tsv_css to format at the right indentation level within a Svelte component. `LayoutMode::Embedded` selects ContinuationIndent style for binary expressions (matches Prettier's `JsExpressionRoot` parent → `shouldNotIndent = true` semantics).
