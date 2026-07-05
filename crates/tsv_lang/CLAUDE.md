# tsv_lang

> Language-agnostic foundation crate for `tsv`

All language crates (tsv_ts, tsv_css, tsv_svelte) depend on tsv_lang. It provides the shared primitives for parsing, formatting, and AST manipulation. Zero external dependencies (only std).

## Modules

Each module's visibility (in parens) reflects `pub use`-only modules (private) vs directly-imported modules (`pub mod`, used as `tsv_lang::doc::{...}` etc.).

- `span` (`span.rs`, private) — `Span { start: u32, end: u32 }` — compact source positions
- `location` (`location.rs`, private) — `LocationTracker` (line/column via binary search on line starts, fronted by a 1-entry line-range cache that turns the sequential-emission common case into an O(1) range check), `ByteToCharMap` (byte → UTF-16 code-unit offsets; `identity()` for byte-space passthrough), and `LocationMapper` (tracker + map bundle the AST-conversion layers thread — with a real map it emits final char-space positions during conversion, fusing out the post-conversion translation walk; with the identity map it is exact byte-space passthrough)
- `error` (`error.rs`, private) — `ParseError` with context extraction and caret formatting
- `config` (`config.rs`, private) — `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts + `EmbedContext` / `LayoutMode` (no runtime config)
- `doc` (`doc/*.rs`, pub) — Document builder — arena-based Prettier-compatible IR
- `comment` (`comment.rs`, private) — Comment type, classification, and O(log n) range lookup
- `printing` (`printing.rs`, pub) — String literal formatting, same-line detection, visual width
- `source_scan` (`source_scan.rs`, pub) — Trivia-aware source scanning: the `skip_trivia` cursor plus the `find_char` / `find_keyword` / `rfind_keyword` delimiter/keyword finders (skipping JS/CSS comments + strings), the `is_regex_start` / `skip_regex_literal` regex helpers (the one piece of `/`-disambiguation `skip_trivia` deliberately leaves out, since it needs backward token lookback), and the balanced-brace pair `scan_to_matching_brace` (the expression-context `{…}` matcher — trivia + regex + template aware) / `skip_template_literal` (interpolation-aware template skip, since `skip_trivia`'s opaque quote-to-quote scan mis-pairs backticks across a nested template like `` `${`x`}` ``). The single chokepoint for re-scanning source between AST nodes — used by AST conversion, all three printers, the Svelte parser (which wraps `scan_to_matching_brace` for its `{…}` tags and shares `skip_template_literal` in its regex-unaware binding-pattern scan), and the TS parser's arrow-vs-paren / type-args lookahead
- `interner` (`interner.rs`, private) — String interning traits (`SymbolResolver`, `InfallibleResolve`); implements `doc::TextResolver`
- `escapes` (`escapes.rs`, private) — Escape sequence handling (quote swapping) — used internally by `printing`
- `sizing` (`sizing.rs`, private) — `estimated_json_capacity` / `estimated_ast_arena_capacity` — pre-size heuristics for the wire-JSON output buffer and the parse-time bump arena
- `output` (`output.rs`, private) — `OutputBuffer` — string building with column tracking

Each language parser keeps its own single-token lookahead as `peek: Option<Token>` (the lexer's own token POD), with any decoded escape value parked out-of-band — there is no shared lookahead type.

## Doc Builder

The doc builder is the core of the formatting architecture. Language printers build declarative doc trees; the shared renderer decides layout based on print width.

### Key Types

- **`DocArena`** — Contiguous storage for all doc nodes, plus the text pool (the `String` backing `Pooled`/`MultilineText` bodies) and an inline direct-mapped static cache whose slots carry two halves: the amortized-eager widths behind `text()` statics, and the per-document **interned node** — repeated `text(",")` calls within one format return one shared `DocId` instead of allocating per call (`empty()` interns through a dedicated cell; sound because statics are position-free at render, nodes are append-only, and no consumer compares `DocId` identity). The stateless singleton nodes intern the same way through dedicated generation-gated cells with no hash probe: the four `Line` kinds (direct-indexed by `LineKind` discriminant), `LineSuffixBoundary`, and `BreakParent` — a `Line` node carries no mode or indent (both supplied per visit by the enclosing render command), so every `line()`/`softline()`/`hardline()`/`literalline()` within one document returns one shared node. `Symbol` nodes intern per document too, through a generation-gated table direct-indexed by the interner id (ids are small dense per-document integers — no hash probe), so repeated `symbol(id)` calls for the same element/attribute name share one node. The arena also parks a per-render output scratch buffer (`take_render_scratch()`/`park_render_scratch()` — the render analog of `pool_writer()`'s parked scratch): the hot per-piece render-and-write seams (TS whole-program/per-expression, CSS per declaration, Svelte per root node) render through the `*_into` entry points into it, one warm buffer per file instead of an alloc/free per call, with a fresh-fallback empty default so nested renders stay correct. The render loop's work buffers pool the same way — each top-level render borrows the arena's command stack + line-suffix buffer (`RefCell`-backed, cleared at borrow; sub-renders keep their own inline `SmallVec` locals) — and the per-file line-break table parks via `take_line_breaks_scratch()`/`park_line_breaks_scratch()` (filled by `printing::build_line_breaks_into` in each `format_in`). The doc-build side pools too: the wide-list builders assemble their parts into a `DocBuf` drawn from a recursion-safe free-list (`acquire_docbuf`/`release_docbuf`, or the `PooledDocBuf` RAII guard from `pooled_docbuf()`) — a builder pops a cleared buffer (retaining a prior spill's heap capacity) and returns it on scope exit, so the many transient `SmallVec` spills across a document collapse into a handful of long-lived reused buffers (the pool self-sizes to the max concurrent-live builders and is retained across `reset()`); byte-identical — allocation only, never output. Heuristic capacity: ~2 nodes per source byte (kept above the post-interning ~0.26/byte density because `estimated_children = nodes/2` must still clear the un-shrunk children demand); the text pool pre-sizes at source/8 (measured per-file demand p50 ≈ 0.17× source). `reset()` clears the node/child/text-pool/memo stores while retaining capacity — O(1) on the node store, since `DocNode` carries no drop glue — so a multi-file driver reuses one arena across files (the doc-IR analogue of the binding crates' `Bump::reset()` reuse); the static cache's width halves deliberately survive `reset()` (they key on `'static` string addresses — warming once per arena lifetime) while the interned node halves are invalidated in O(1) by the reset's `format_gen` bump; the printers borrow `&DocArena` and the caller owns the reusable one (`format_in` on each language crate is the borrowed-arena entry point).
- **`DocId`** (`u32`) — Lightweight, `Copy` handle into the arena. No cloning, no recursive Drop.
- **`DocBuf`** (`SmallVec<[DocId; 8]>`) — Shared stack buffer for assembling a node's doc parts before `concat()` / `fill()`. Most nodes have only a handful of parts, so the common case stays off the heap; larger nodes spill. Used by all language printers (the TS chain / binary-operator printers, the Svelte template printer) as the single canonical doc-parts buffer type. Wide-list builders (statement / object / array / parameter / specifier lists) draw a reusable buffer from the arena's `DocBuf` free-list (`pooled_docbuf()`) rather than allocating a fresh `SmallVec` per call, amortizing the per-spill malloc/free churn (see `DocArena` below).
- **`DocNode`** — Node variants: `Text`, `MultilineText` (a `\n`-separated body rendered with per-line context indent — one pool-stored body for an indentable multi-line block comment), `Line`, `Indent`, `Dedent`, `Group`, `IfBreak`, `Concat`, `Fill`, etc. `DocNode` carries no drop glue (`const`-asserted via `needs_drop`): dynamic text lives in the arena text pool, so `reset()`/drop never walk the node store running destructors. Its size is also pinned by a companion `const` assert — **32 B on 64-bit** (the native flagship), **16 B on wasm32** (the shipped WASM bundles); the size is pointer-width dependent (`Align`'s `usize`, `DocText::Static`'s fat pointer), so the pin is `cfg`-gated per target. The node store is walked linearly at render, so the AoS layout's cache locality is the point (shrinking the node has been refuted repeatedly on this traversal-bound engine); a variant that bloats it is a deliberate decision, not an accident.
- **`DocText`** — Four variants: `Static(&'static str)` (punctuation/keywords), `Pooled(PoolSpan)` (dynamic text, stored in the arena text pool), `SourceSpan(Span)` (verbatim source slice — resolved against `source` at print time, like `Symbol` but keyed on a span; zero allocation for unmodified text such as identifier names (via `source_span_ident`), comments, template chunks, already-canonical literals (TS numbers/strings, CSS dimensions), and Svelte markup text, with no `DocArena` lifetime), `Symbol(u32)` (deferred resolution via interner). Width policy: `Pooled`, `SourceSpan`, and `Static` always precompute their visual width at build (a real width or the newline sentinel — fits never borrows the pool, render skips its column byte-scan; `Static`'s precompute is amortized through the arena's static cache, measured once per unique string per arena rather than per node — the same slots that intern `Static` nodes per document), and the exceptions are identifier names (`source_span_ident`) and `Symbol`, which measure on demand (high-frequency, rarely fits-measured — the opposite tradeoff, measured both ways).
- **`LineKind`** — `Normal` (space in flat, newline in break), `Soft` (nothing in flat), `Hard` (always newline), `Literal` (newline without indent).

### Builder API Categories

All methods take `&self` (interior mutability via `RefCell`):

- Text — `text()`, `text_pooled(&str)` (dynamic text, copied into the pool), `multiline_text(&str)`, `pool_writer()` (streaming pooled-text assembly: a `PoolTextWriter` owning an arena-parked scratch buffer — no transient `String`, no pool borrow held open, so interleaved arena calls stay correct; consume-on-finish `finish_text()` / `finish_multiline_text()`; implements `fmt::Write`), `source_span()` / `source_span_ident()` (newline-free, width-deferred — identifier names) / `line_comment_source_span()` (verbatim source slice, no allocation), `empty()`, `symbol()`
- Lines — `line()`, `softline()`, `hardline()`, `literalline()`
- Structure — `group()`, `group_break()`, `indent()`, `dedent()`, `align()`
- Conditionals — `if_break()`, `indent_if_break()`, `conditional_group()`
- Sequences — `concat()`, `fill()`, `join()`, `join_doc()`
- Buffer pooling — `pooled_docbuf()` (RAII `PooledDocBuf`, releases on drop) / `acquire_docbuf()` / `release_docbuf()` — reusable `DocBuf` assembly buffers for wide-list builders
- Context — `with_context()`
- Line suffix — `line_suffix()`, `line_suffix_boundary()`, `break_parent()`
- Convenience — `wrap()`, `parens()`, `brackets()`, `braces()`
- Inspection — `will_break()`, `has_forced_break()`
- Diagnostics — `line_comment_text_pooled()` (tags `//` text for the swallow check)

The `doc::swallow` module is a render-time guard against the
line-comment-swallow bug class (a `//` emitted inline runs to EOL and consumes
the following token). It lives behind the **`swallow_check` cargo feature** (off
by default, like tsv_ts's `convert`), so production builds compile it out
entirely — no `DocArena` side-set, no render hook; `line_comment_text_pooled`
collapses to `text_pooled`. With the feature, `set_swallow_check(true)` arms it
and the renderer (via `SwallowTracker`) records every swallow into a thread-local
sink drained by `take_swallow_reports()`. Output-neutral. `tsv_debug` forwards
the feature as its own opt-in `swallow_check` feature (off by default so its
profiles measure production-shaped render code) and gates the `swallow_audit`
command behind it — build with `--features swallow_check` to drive
`tsv_debug swallow_audit`.

### Rendering Pipeline

```
Language Printer builds DocId tree
        ↓
arena_fits_with_lookahead()  — check if group fits in remaining width
        ↓
arena_print_doc*()           — render doc tree to formatted string
```

**Rendering variants** (6, plus `_into` forms):

- `arena_print_doc()` — standard (column 0, no resolver)
- `arena_print_doc_flat_resolved()` — render in flat mode (no group breaking)
- `arena_print_doc_at_column()` — mid-line start (for Svelte template expressions)
- `arena_print_doc_with_indent()` — explicit indent level
- `arena_print_doc_with_indent_resolved()` — full control
- `arena_print_doc_with_indent_resolved_preserve_whitespace()` — for HTML pre/textarea

The two `resolved` variants also have `*_into(…, output: &mut String)` forms that render into a caller-provided buffer (reserving `estimated_output_capacity` themselves) — the seam behind the arena-parked render scratch the per-piece writers use; the `String`-returning forms are thin wrappers over them.

## How Language Crates Use tsv_lang

### Parsing

```rust
// Language parsers use:
use tsv_lang::{ParseError, Result, Span};
use tsv_lang::location::LocationTracker;  // For wire-JSON emission
use tsv_lang::comment::Comment;           // Collected during parsing
// Lookahead is each parser's own `peek: Option<Token>` over its lexer's token POD.

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
// Internal AST → wire JSON, emitted directly by the writer in one walk:
let (tracker, map) = LocationTracker::new_ecmascript_with_map(source);
let bytes = write_program_json(&program, source, LocationMapper { tracker: &tracker, map: &map }, Schema::Acorn);
// The LocationMapper carries the ByteToCharMap so the writer emits final UTF-16
// positions directly (identity/byte-space passthrough on ASCII). Pass
// Schema::SvelteScript when writing a Svelte non-lang="ts" <script>
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

The interner is per-document, shared across all languages in a file — its tenants are Svelte element/attribute names and escaped identifiers (identifier names are span-identity; see the Pattern below). Symbols flow from parser through doc builder to renderer:

- `TextResolver` — `resolve(id: u32) -> &str` — resolve symbol during rendering. Also `resolve_source_span(span) -> &str` (defaulted to panic) for `DocText::SourceSpan` nodes; the default-impl interner carries no source, so a printer emitting `SourceSpan` wraps its interner in `doc::SourceTextResolver { inner, source }` and passes that to the resolved render entry points (this is how `source` reaches render without a `DocArena` lifetime). A printer with **no interner** — the CSS printer, which emits source slices directly and never `DocText::Symbol` — instead supplies a bare source-only `TextResolver` (its `resolve` is unreachable, only `resolve_source_span` does work), the same source-awareness without a symbol table.
- `SymbolResolver` — `resolve_symbol()`, `with_resolved_symbol()` — zero-allocation hot path
- `InfallibleResolve` — `resolve_infallible()` — panic-free resolution
- `SymbolToU32` — Convert `DefaultSymbol` to `u32` for doc builder `Symbol` variant
- `SharedInterner` — Type alias `Rc<RefCell<DefaultStringInterner>>` — shared interner handle

**Pattern**: identifier names are span-identity — the AST stores a name channel (tsv_ts's `IdentName`: raw token length + an `Option<DefaultSymbol>` escape hatch) and printers emit `DocText::SourceSpan` name slices, so the common path never interns. The interner's remaining tenants are tsv_svelte's element/attribute names (parser interns → AST stores `DefaultSymbol` → printer calls `arena.symbol(sym.to_u32())`, interned per document so repeated ids share one node → renderer resolves via `TextResolver` at print time) and the rare unicode-escaped identifier, whose decoded name rides the same deferred-`Symbol` path.

## Config Types

`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts, `EmbedContext`, and `LayoutMode` are covered in [../../CLAUDE.md §Internal Configuration](../../CLAUDE.md#internal-configuration-rust-library-only). tsv has no runtime configuration.

**Embedding knobs**: `base_indent_offset` and `first_line_offset` are how tsv_svelte tells tsv_ts/tsv_css to format at the right indentation level within a Svelte component. `LayoutMode::Embedded` selects ContinuationIndent style for binary expressions (matches Prettier's `JsExpressionRoot` parent → `shouldNotIndent = true` semantics).
