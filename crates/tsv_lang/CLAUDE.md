# tsv_lang

> Language-agnostic foundation crate for `tsv`

All language crates (tsv_ts, tsv_css, tsv_svelte) depend on tsv_lang. It provides the shared primitives for parsing, formatting, and AST manipulation. Zero external dependencies (only std).

## Modules

Each module's visibility (in parens) reflects `pub use`-only modules (private) vs directly-imported modules (`pub mod`, used as `tsv_lang::doc::{...}` etc.).

- `span` (`span.rs`, private) — `Span { start: u32, end: u32 }` — compact source positions
- `location` (`location.rs`, private) — `LocationTracker` (line/column via binary search on line starts, fronted by a 1-entry line-range cache that turns the sequential-emission common case into an O(1) range check), `ByteToCharMap` (byte → UTF-16 code-unit offsets; `identity()` for byte-space passthrough), and `LocationMapper` (tracker + map bundle the AST-conversion layers thread — with a real map it emits final char-space positions during conversion, fusing out the post-conversion translation walk; with the identity map it is exact byte-space passthrough). The `no-locations` emission path skips the line-start scan entirely — it builds a line-data-free tracker via `LocationTracker::new_map_only` (stub `line_starts`, byte→UTF-16 map only) and emits `start`/`end` offsets with no line/column
- `error` (`error.rs`, private) — `ParseError` with context extraction and caret formatting
- `config` (`config.rs`, private) — `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts + `EmbedContext` / `LayoutMode` (no runtime config)
- `doc` (`doc/*.rs`, pub) — Document builder — arena-based Prettier-compatible IR
- `comment` (`comment.rs`, private) — Comment type, classification, and O(log n) range lookup
- `comment_ledger` (`comment_ledger.rs`, pub, **`comment_check` feature**) — the print-once comment ledger (diagnostic)
- `printing` (`printing.rs`, pub) — String literal formatting, same-line detection, visual width
- `source_scan` (`source_scan.rs`, pub) — Trivia-aware source scanning: the `skip_trivia` cursor plus the `find_char` / `find_keyword` / `rfind_keyword` delimiter/keyword finders (skipping JS/CSS comments + strings), the `is_regex_start` / `skip_regex_literal` regex helpers (the one piece of `/`-disambiguation `skip_trivia` deliberately leaves out, since it needs backward token lookback), and the balanced-brace pair `scan_to_matching_brace` (the expression-context `{…}` matcher — trivia + regex + template aware) / `skip_template_literal` (interpolation-aware template skip, since `skip_trivia`'s opaque quote-to-quote scan mis-pairs backticks across a nested template like `` `${`x`}` ``). The single chokepoint for re-scanning source between AST nodes — used by AST conversion, all three printers, the Svelte parser (which wraps `scan_to_matching_brace` for its `{…}` tags and shares `skip_template_literal` in its regex-unaware binding-pattern scan), and the TS parser's arrow-vs-paren / type-args lookahead
- `escapes` (`escapes.rs`, private) — Escape sequence handling (quote swapping) — used internally by `printing`
- `sizing` (`sizing.rs`, private) — `estimated_json_capacity` / `estimated_ast_arena_capacity` — pre-size heuristics for the wire-JSON output buffer and the parse-time bump arena
- `output` (`output.rs`, private) — `OutputBuffer` — string building with column tracking

Each language parser keeps its own single-token lookahead as `peek: Option<Token>` (the lexer's own token POD), with any decoded escape value parked out-of-band — there is no shared lookahead type.

## Doc Builder

The doc builder is the core of the formatting architecture. Language printers build declarative doc trees; the shared renderer decides layout based on print width.

### Key Types

- **`DocArena`** — Contiguous storage for all doc nodes, plus the text pool (the `String` backing `Pooled`/`MultilineText` bodies) and an inline direct-mapped static cache whose slots carry two halves: the amortized-eager widths behind `text()` statics, and the per-document **interned node** — repeated `text(",")` calls within one format return one shared `DocId` instead of allocating per call (`empty()` interns through a dedicated cell; sound because statics are position-free at render, nodes are append-only, and no consumer compares `DocId` identity). The stateless singleton nodes intern the same way through dedicated generation-gated cells with no hash probe: the four `Line` kinds (direct-indexed by `LineKind` discriminant), `LineSuffixBoundary`, and `BreakParent` — a `Line` node carries no mode or indent (both supplied per visit by the enclosing render command), so every `line()`/`softline()`/`hardline()`/`literalline()` within one document returns one shared node. The arena also parks a per-render output scratch buffer (`take_render_scratch()`/`park_render_scratch()` — the render analog of `pool_writer()`'s parked scratch): the hot per-piece render-and-write seams (TS whole-program/per-expression, CSS per declaration, Svelte per root node) render through the `*_into` entry points into it, one warm buffer per file instead of an alloc/free per call, with a fresh-fallback empty default so nested renders stay correct. The render loop's work buffers pool the same way — each top-level render borrows the arena's command stack + line-suffix buffer (`RefCell`-backed, cleared at borrow; sub-renders keep their own inline `SmallVec` locals) — and the per-file line-break table parks via `take_line_breaks_scratch()`/`park_line_breaks_scratch()` (filled by `printing::build_line_breaks_into` in each `format_in`), and the multi-line block-comment builders borrow a parked line-offset scratch (`borrow_line_spans_scratch()` — one `split('\n')` pass per comment fills each body line's `(start, end)` range, so the classifier and builders iterate slice-cheap with no per-comment line buffer). The doc-build side pools too: the wide-list builders assemble their parts into a `DocBuf` drawn from a recursion-safe free-list (`acquire_docbuf`/`release_docbuf`, or the `PooledDocBuf` RAII guard from `pooled_docbuf()`) — a builder pops a cleared buffer (retaining a prior spill's heap capacity) and returns it on scope exit, so the many transient `SmallVec` spills across a document collapse into a handful of long-lived reused buffers; the free-list keeps **only spilled buffers** (a release drops a never-spilled one — nothing to retain, free to re-construct), so every pooled entry carries real heap capacity and a big-need builder can't pop a virgin buffer while capacity sits deeper in the LIFO; retained across `reset()`; byte-identical — allocation only, never output. A parked node-keyed doc-share map (`share_map_scratch()`, an AST-node pointer → built `DocId` table) backs the TS printer's member-chain argument sharing the same way — the consumer clears it at share-scope entry/exit, so only its table capacity persists instead of a fresh `HashMap` resize chain per printer/file. Heuristic capacity: ~2 nodes per source byte (kept above the post-interning ~0.26/byte density because `estimated_children = nodes/2` must still clear the un-shrunk children demand); the text pool pre-sizes at source/8 (measured per-file demand p50 ≈ 0.17× source). `reset()` clears the node/child/text-pool/memo stores while retaining capacity — O(1) on the node store, since `DocNode` carries no drop glue — so a multi-file driver reuses one arena across files (the doc-IR analogue of the binding crates' `Bump::reset()` reuse); the static cache's width halves deliberately survive `reset()` (they key on `'static` string addresses — warming once per arena lifetime) while the interned node halves are invalidated in O(1) by the reset's `format_gen` bump; the printers borrow `&DocArena` and the caller owns the reusable one (`format_in` on each language crate is the borrowed-arena entry point).
- **`DocId`** (`u32`) — Lightweight, `Copy` handle into the arena. No cloning, no recursive Drop.
- **`DocBuf`** (`SmallVec<[DocId; 8]>`) — Shared stack buffer for assembling a node's doc parts before `concat()` / `fill()`. Most nodes have only a handful of parts, so the common case stays off the heap; larger nodes spill. Used by all language printers (the TS chain / binary-operator printers, the Svelte template printer) as the single canonical doc-parts buffer type. Wide-list builders (statement / object / array / parameter / specifier lists) draw a reusable buffer from the arena's `DocBuf` free-list (`pooled_docbuf()`) rather than allocating a fresh `SmallVec` per call, amortizing the per-spill malloc/free churn (see `DocArena` below).
- **`DocNode`** — Node variants: `Text`, `MultilineText` (a `\n`-separated body rendered with per-line context indent — one pool-stored body for an indentable multi-line block comment), `Line`, `Indent`, `Dedent`, `Group`, `IfBreak`, `Concat`, `Fill`, etc. `DocNode` carries no drop glue (`const`-asserted via `needs_drop`): dynamic text lives in the arena text pool, so `reset()`/drop never walk the node store running destructors. Its size is also pinned by a companion `const` assert — **32 B on 64-bit** (the native flagship), **16 B on wasm32** (the shipped WASM bundles); the size is pointer-width dependent (`AlignRoot`'s `usize`, `DocText::Static`'s fat pointer), so the pin is `cfg`-gated per target. The node store is walked linearly at render, so the AoS layout's cache locality is the point (shrinking the node has been refuted repeatedly on this traversal-bound engine); a variant that bloats it is a deliberate decision, not an accident.
- **`DocText`** — Four variants: `Static(&'static str)` (punctuation/keywords), `Pooled(PoolSpan)` (dynamic text, stored in the arena text pool), `SourceSpan(Span)` (verbatim source slice — resolved against `source` at print time; zero allocation for unmodified text such as identifier and element/attribute names (via `source_span_ident`), comments, template chunks, already-canonical literals (TS numbers/strings, CSS dimensions), and Svelte markup text, with no `DocArena` lifetime), and `VerbatimSpan(Span)` (`SourceSpan` for a **format-ignored frozen slice** — identical in measurement and render, but **opaque to `will_break`**: a frozen slice's embedded newlines are source layout, not a break the enclosing group must honor, matching prettier's `printIgnored` plain-string docs; built only via `verbatim_source_span`, only by the tsv_ts/tsv_svelte ignore emitters). Width policy: `Pooled`, `SourceSpan`/`VerbatimSpan`, and `Static` always precompute their visual width at build (a real width or the newline sentinel — fits never borrows the pool, render skips its column byte-scan; `Static`'s precompute is amortized through the arena's static cache, measured once per unique string per arena rather than per node — the same slots that intern `Static` nodes per document), and the exception is the name slices (`source_span_ident`), which measure on demand (high-frequency, rarely fits-measured — the opposite tradeoff, measured both ways).
- **`LineKind`** — `Normal` (space in flat, newline in break), `Soft` (nothing in flat), `Hard` (always newline), `Literal` (newline without indent).

### Text width: the corpus cannot grade it — the equivalence test can

`pooled_text_width` (the eager precompute above) answers three questions of a text node in **one** byte pass: is there a newline, is the slice ASCII, how many tabs. Past `FUSED_WIDTH_SCAN_MAX` it flips to a searcher-based shape instead — `contains('\n')` and `is_ascii` are SIMD and the tab count auto-vectorizes, so on a long slice three vector passes beat one scalar walk, while on the short slice that normally arrives their setup (paid regardless of length) is the entire cost. The gate is not decoration: an ungated fused walk is a measurable **regression** on the TS corpus, whose text nodes run longer, while CSS never notices the gate at all.

**What a contributor must know before touching any of this: no corpus can tell you that you got the arithmetic wrong.** A width only changes the output once it crosses the print width, so an error on a rare byte leaves every formatted file byte-identical. Corrupting the tab arm by a single column was verified to pass the **fixture suite**, an **11,696-file format diff**, and an **11,696-file wire diff** — every external gate in the repo — and to be caught **only** by the exhaustive equivalence test beside the function, which grades it against the searcher shape on every string of length 0–3 over an alphabet covering each arm (including the control chars and the boundary-crossing grapheme clusters). Keep that test green; it is the only thing that can fail.

Two traps it exists to catch. The fused walk mirrors `printing::visual_width`'s **ASCII fast path**, where a control character is **one** column — deliberately *not* `printing::ascii_char_width`, which counts it as **zero** and which only the grapheme-walking path uses. And a non-ASCII byte hands the **whole** slice to the searcher arm, never the scanned remainder, because a grapheme cluster can begin on the ASCII byte *before* it.

### Builder API Categories

All methods take `&self` (interior mutability via `RefCell`):

- Text — `text()`, `text_pooled(&str)` (dynamic text, copied into the pool), `multiline_text(&str)`, `pool_writer()` (streaming pooled-text assembly: a `PoolTextWriter` owning an arena-parked scratch buffer — no transient `String`, no pool borrow held open, so interleaved arena calls stay correct; consume-on-finish `finish_text()` / `finish_multiline_text()`; implements `fmt::Write`), `source_span()` / `source_span_ident()` (newline-free, width-deferred — identifier / element / attribute names) / `line_comment_source_span()` (verbatim source slice, no allocation) / `verbatim_source_span()` (format-ignored frozen slice — `will_break`-opaque), `empty()`
- Lines — `line()`, `softline()`, `hardline()`, `literalline()`
- Structure — `group()`, `group_break()`, `indent()`, `dedent()`, `align_root()` (absolute tab level — template-literal root reset), `align()` (sub-tab `align(n)` — literal spaces under useTabs, tab-width-independent alignment)
- Conditionals — `if_break()`, `indent_if_break()`, `conditional_group()`
- Sequences — `concat()`, `fill()`, `join()`, `join_doc()`
- Buffer pooling — `pooled_docbuf()` (RAII `PooledDocBuf`, releases on drop) / `acquire_docbuf()` / `release_docbuf()` — reusable `DocBuf` assembly buffers for wide-list builders
- Context — `with_context()`
- Line suffix — `line_suffix()`, `line_suffix_boundary()`, `break_parent()`
- Convenience — `wrap()`, `parens()`, `brackets()`, `braces()`
- Inspection — `will_break()`, `has_forced_break()`
- Transforms — `remove_lines()` / `atomize()` — rebuild a subtree with its lines statically flattened (old nodes stay in the arena, unused). **Two operations, not one function with a strength dial**, so pick by which prettier behavior you want: `remove_lines` is prettier's `removeLines` (breakable lines only; hard lines and `MultilineText` survive — it cannot promise one line), while `atomize` emulates a re-render at `printWidth: Infinity` (hard lines deleted, `conditional_group` collapsed to its least-expanded state). Atomizing is only sound where the caller has proved no newline is required — deleting a hard line fuses the content around it. The atomize contract is asserted directly by a width-invariance test: its result must render identically at every width
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

**Rendering variants** (the `_resolved_*_into` forms are the production seam):

- `arena_print_doc()` — standard (column 0, no source: for docs with no `SourceSpan`)
- `arena_print_doc_with_indent_resolved_into()` — source-resolved render into a caller-provided buffer (full control)
- `arena_print_doc_with_indent_resolved_preserve_whitespace_into()` — same, preserving last-line whitespace (HTML pre/textarea)

The `_resolved_*_into` forms thread the document `source` (so `DocText::SourceSpan` leaves resolve to their verbatim slice) and render into a caller-provided buffer, reserving `estimated_output_capacity` themselves — the seam behind the arena-parked render scratch the per-piece writers use. `arena_print_doc` passes no source, since its docs contain no `SourceSpan`. (`arena_measure_doc_flat_resolved` renders flat for *measuring* only — never written to output.)

**Below those entry points, the render path threads one `&RenderCtx`.** The mutually-recursive
internals (`render_doc_iterative` → `render_doc_core` → `render_single_doc` /
`render_fill_iterative`, plus the line-suffix flush) each need the same four invariants — the
arena, the `RenderConfig`, the `EmbedContext`, and the document `source` (`Option<&str>`, for
resolving `DocText::SourceSpan` leaves) — so those are bundled into `RenderCtx` and every entry
point constructs one. Each internal function destructures it back
into locals at entry, so the render logic reads unchanged.

⚠️ `RenderCtx` holds **only shared references, deliberately**. The mutable render state —
`output`, `pos`, `should_remeasure`, and the command / line-suffix work buffers — stays as
separate `&mut` parameters, which is why three functions still carry a
`clippy::too_many_arguments` allow. Bundling those behind a struct pointer takes their address
and sinks them out of registers in the hot loop; the allow is the cheaper price. Don't "finish
the job" by folding them in without an instruction-count gate.

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
let mut printer = Printer::new(&arena, source, &comments, &line_breaks, config);
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

See [../../docs/comments.md](../../docs/comments.md) for the detached model rationale, the `Comment` struct, the ownership doctrine, and the leading-comment emitter rules; the always-loaded core is [../../CLAUDE.md §Comment Handling](../../CLAUDE.md#comment-handling-detached-model).

### Lookup Functions — three questions, three names

`Comment::owned_by_node` takes a comment out of the *positional* model: the node its token
begins prints it. **Ownership is a fact about who PRINTS a comment, never about whether it
EXISTS** — so the API asks the caller to name which of the three questions it is asking, and
every name states its axis. A miswire then reads as a category error at the call site rather
than as plausible code. See [../../CLAUDE.md §Comment Handling](../../CLAUDE.md#comment-handling-detached-model).

**to emit** — "which comments must *I* print here?" — **skips** owned:

- `comments_to_emit_in_range()` / `has_comments_to_emit_in_range()` / `comments_to_emit_after()`

**on page** — "does any comment occupy the page here?" — **counts** owned. Every layout gate
(break / expand / hug / paren / fast-path / force-multiline):

- `has_comments_on_page_in_range()` / `has_multiline_block_comments_on_page_in_range()`

**in source** — "what comment bytes are physically here?" — **counts** owned. Every cursor
(blank-line scan, offset, `prev_end`):

- `comments_in_source_range()` / `comments_in_source_after()`

Axis-free (provably): `has_line_comments_in_range()` — ownership only ever binds a **block**
comment, so skip ≡ count. If a line comment ever becomes ownable, it must grow an axis.

Shared:

- `find_first_comment_from()` — Binary-search index of first comment with `span.start >= pos`
- `classify_comment()` — Classify as Trailing, LeadingOwnLine, or LeadingInline
- `classify_comment_fast()` — Same but using precomputed line breaks (faster)
- `ClassifiedComments::from_range()` — Batch classify all 4 categories in one pass (emit-keyed)

### Print-Once Ledger (`comment_check` feature)

Nothing in the detached model forces a parsed comment to be *printed* — a gap emitter that
never runs, an owned comment whose node reassembles off the ownership seam, a builder
handed `&[]` for its comment slice each silently lose one. `comment_ledger` is the
structural guard (tsv's `ensureAllCommentsPrinted`): each format entry point registers the
comment list it is about to print (`register_parsed`), each emission records one
(`record_emitted`), each raw source slice that carries comments out verbatim records its
range (`record_verbatim_range`), and `take_comment_ledger` reports every comment whose
emit count isn't exactly one — DROPPED or DOUBLE-PRINTED.

The **doc-based** printers (`tsv_ts`, `tsv_svelte`) don't record at build: they tag the
comment's doc node (`DocArena::tag_comment_doc`) and the *renderer* records the emit when
it reaches that node. A builder may assemble one subtree into two `conditional_group`
candidates of which only one renders, so build-time counting reads as a double-print — and
a comment built only into a *losing* candidate would read as printed while being lost.
`tsv_css`, whose printer writes comments straight to its output buffer, records at the
write itself.

Off by default (like `swallow_check`), so production builds — and default `tsv_debug`
builds, whose profiles must measure production-shaped code — compile out the registration,
the `DocArena` side-set, and the render hook. Output is byte-identical either way.
`tsv_debug` forwards the feature and gates `comment_audit` behind it; `deno task
comments:audit` drives it over `tests/fixtures` and is gated in `deno task check`.

### Directive Recognition

`is_format_ignore_directive()` / `is_format_ignore_range_start()` / `is_format_ignore_range_end()` are the single source of truth for the format-suppression directive set — the tsv-native `format-ignore` family plus prettier's `prettier-ignore` family (drop-in compat). Each operates on trimmed comment text and is called by all three language printers (`tsv_ts`, `tsv_css`, `tsv_svelte`), since the comment types differ across crates. See [docs/directives.md](../../docs/directives.md) and [docs/conformance_prettier.md §Format-ignore directive](../../docs/conformance_prettier.md#format-ignore-directive).

## Names are span-identity — no interner

There is **no string interner**. Every name a printer emits — TS/JS identifier
names, Svelte element and attribute names — is recovered from the source slice it
occupies (`source[span]`), never from a symbol table:

- **TS identifier names** (`tsv_ts`'s `IdentName`): the name is the leading
  `raw_len` bytes of the node span, re-sliced at every consumer. The rare
  `\u`-escaped or `u16`-oversized name that can't be recovered from source carries
  its decoded form as an `Option<&'arena str>` escape hatch (the parser's already
  arena-allocated `current_decoded`) — read directly, no round-trip.
- **Svelte element/attribute names** (`tsv_svelte`): `Element::name(source)` =
  `source[name_span]` (tag names are verbatim source runs); `Attribute::name(source)`
  = `source[name_span].trim()` (a no-op except for a padded `{ shorthand }`, whose
  `name_span` covers the untrimmed braces interior to match Svelte's `name_loc`).
  No stored name field at all.

The one render-time resolution the doc builder needs is [`DocText::SourceSpan`] →
verbatim source slice (`span.extract(source)`). A printer emitting `SourceSpan` passes
its `&str` source to the resolved render entry points (the `_resolved_*_into` forms),
which thread it through the render path as `Option<&str>` — this is how `source` reaches
render without putting a lifetime on `DocArena` (the span lives in the lifetime-less
arena; the source is supplied transiently at render). There is no resolver trait, no
symbol variant, no deferred symbol resolution, and no `string_interner` dependency.

## Config Types

`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts, `EmbedContext`, and `LayoutMode` are covered in [../../CLAUDE.md §Internal Configuration](../../CLAUDE.md#internal-configuration-rust-library-only). tsv has no runtime configuration.

**Embedding knobs**: `base_indent_offset` and `first_line_offset` are how tsv_svelte tells tsv_ts/tsv_css to format at the right indentation level within a Svelte component. `LayoutMode::Embedded` selects ContinuationIndent style for binary expressions (matches Prettier's `JsExpressionRoot` parent → `shouldNotIndent = true` semantics).
