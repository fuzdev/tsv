# Architecture

Design decisions and technical rationale for tsv.

## Core Philosophy

tsv is a **multi-tool foundation** for Svelte/TypeScript/CSS—formatter, parser, and future linter/LSP. JSON serialization for testing compatibility is secondary to efficient internal manipulation.

This inverts the typical approach where JSON compatibility drives AST design.

**Optimal artifacts (invariant).** Runtime speed _and_ compiled code size are first-class, non-negotiable goals for **every** shipped artifact. The format-only `@fuzdev/tsv_format_wasm` is the current yardstick—it's the most-developed and first-shipped artifact—but it holds no long-term primacy; `@fuzdev/tsv_parse_wasm`, the CLI, and the native N-API binding count just as much as they mature. The architecture serves this directly: concrete types end-to-end (no `dyn` dispatch), per-language crates that WASM tree-shakes independently, and unneeded layers excluded at the link level — the printers from parse-only builds, the convert layer from format-only builds (see §"Closed Scope, Open Convention"). Heavier infrastructure for future tools—incremental reparse, red-green/CST layers for LSP—must be added as later, feature-gated layers that don't regress this, not as weight in the initial artifacts (see §"Red-Green Trees (Deferred)").

**Safety constraint**: `unsafe_code = "forbid"` at the workspace level — no unsafe Rust in core crates. Only the two native binding crates relax it, each to the weakest level that compiles: `tsv_ffi` to `"allow"`, since the crate *is* the C ABI boundary and hand-writes the raw-pointer work; `tsv_napi` to `"deny"`, because it hand-writes no unsafe at all and `#[napi]`'s generated items carry their own `#[allow(unsafe_code)]` — enough to override `deny`, so every other site in that crate stays a compile error. (Neither can inherit `forbid`, which no inner `allow` may override; `tests/lint_parity.rs` pins both relaxations and guards the hand-mirrored tables against drift.) Combined with a deliberately small dependency set (authoritative list: `[workspace.dependencies]` in the root `Cargo.toml`, whose externals are the twelve library/binding crates — the `napi`/`napi-derive`/`napi-build` trio being the N-API carve-out — plus the CLI/debug-only `argh`/`tokio`/`futures-util`; the wasm32-only `talc` allocator lives in `tsv_wasm`'s own manifest; purpose table in [CLAUDE.md § Rust Crates](../CLAUDE.md#rust-crates-minimal-deps)), the attack surface and audit burden stay minimal.

## Two-AST Design

A single AST cannot optimize for both manipulation and serialization.

### Internal AST (what every tool reads)

- Fast traversal (tight loops, cache-friendly)
- Compact memory (u32 positions, span-identity identifier names)
- Zero serialization overhead
- Nested ownership (direct traversal, no index lookups)

### Public wire JSON (serialization boundary only)

- Exact JSON field ordering (matching the canonical parser)
- Plain JSON numbers (byte spans emitted as char / UTF-16 offsets)
- Source-faithful strings (raw slices reconstructed at the boundary)
- Emitted directly by the writer (`ast/convert/`), never an intermediate typed tree

### Solution

```
Parse → Internal AST → [Format, Lint, Analyze]
                          ↓ (only when serializing)
                       convert_ast_json_bytes() → wire JSON
```

Each language crate separates these cleanly:

- `ast/internal` — Optimized for manipulation (file or directory)
- `ast/convert` — Emits the public wire JSON directly from `internal`, in one
  walk (the writer), matching the canonical parser's JSON exactly (file or directory)

TypeScript uses directories (`internal/`, `convert/`) due to complexity. CSS and Svelte use a single `internal.rs` for AST types and a directory for conversion.

Worked example — the internal node is clean and semantic; the writer emits the wire JSON straight from it, applying the canonical quirks (here, `raw` reconstructed from source) with no intermediate typed tree:

```rust
// Internal - clean and semantic
struct Literal {
    value: LiteralValue,  // Fully decoded: "test\n" → "test<newline>"
    span: Span,
}

fn write_literal(w: &mut JsonWriter, lit: &Literal, ctx: &Ctx) {
    node_header(w, "Literal", lit.span, ctx);   // "type"/"start"/"end"/"loc"
    // … "value" emitted from lit.value …
    w.raw(",\"raw\":");
    w.string(lit.span.extract(ctx.source));      // reconstruct from source
    w.raw("}");
}
```

### Model Fidelity Principle

The internal AST is the **spec-faithful model** every tool reads — the formatter today, and the linter / LSP / compiler / type-checker to come. Svelte's parse quirks and prettier's formatting choices live **only at the boundaries**: Svelte-JSON quirks in `ast/convert`, prettier layout choices in the printer. They are never baked into the internal model.

The formatter can absorb looseness; a tool built on a loose model inherits it as wrong answers, and the cost compounds as more tools share the model. So when the spec, Svelte, and prettier disagree, the **model follows the spec**, and each consumer reproduces only the divergences it needs at its own boundary — the public AST matches Svelte's JSON, the printer matches prettier's layout. There is no "prefer prettier if it reads better" carve-out at the model layer; that judgment belongs to the printer, on its output, not to the data every tool shares.

(Worked example: the CSS at-rule prelude. The parser builds a normalized prelude string, but it is printer-facing only — the public `Atrule.prelude` is reproduced source-verbatim at the conversion boundary, so the model stays faithful while the formatter still matches prettier. See [conformance_svelte.md](./conformance_svelte.md).)

## Crate Structure

```
tsv/
├── tsv_lang     # Foundation (Span, Doc, errors, printing utilities)
├── tsv_html     # HTML classification (pure functions)
├── tsv_ignore   # gitignore-aware discovery matcher (hierarchical .gitignore + .formatignore/.prettierignore)
├── tsv_discover # file-discovery policy (build-output heuristic + safety-net pruning) over tsv_ignore
├── tsv_ts       # TypeScript parser/formatter (standalone)
├── tsv_css      # CSS parser/formatter (standalone)
├── tsv_svelte   # Svelte parser/formatter (uses tsv_ts + tsv_css)
├── tsv_svelte_compile # experimental Svelte→JS compiler + JS canonicalizer (uses tsv_svelte + tsv_ts + tsv_css + tsv_html; consumed only by tsv_debug)
├── tsv_check    # experimental TypeScript binder/checker, may never ship (uses tsv_ts + tsv_lang; consumed only by tsv_debug)
├── tsv_cli      # Production CLI binary (pure Rust)
├── tsv_debug    # Dev utilities (uses embedded Deno sidecar for JS tools)
├── tsv_arena    # Shared binding substrate: per-thread reusable AST/doc arenas + the goal axis
├── tsv_ffi      # C FFI bindings
├── tsv_napi     # N-API bindings (Node/Bun native path)
└── tsv_wasm     # WebAssembly bindings
```

`tsv_html` and `tsv_ignore` are independent zero-`tsv_*`-dep leaves (pure
functions). `tsv_discover` is a thin policy layer whose only `tsv_*` dep is
`tsv_ignore` — it owns the build-output heuristic + safety-net pruning *decision*
(the matcher stays a pure gitignore(5) matcher). Both are consumed by `tsv_cli`
directly and by `tsv_wasm` and `tsv_napi` under their `format` features (the
matcher exposed as the `IgnoreStack` class, the policy as that class's verdict
methods), so the CLI, the WASM CLI, the native npm package, and the VS Code
extension all share one discovery matcher *and* one prune decision. `tsv_discover` is file-*scope* policy — the one sanctioned config
carve-out — not a language abstraction (no `Language` trait, registry, or
dispatch), so it doesn't bear on the closed-scope/open-convention stance below.

### Dependency Graph

```
   tsv_lang (foundation)          tsv_html          tsv_ignore
        ↑                       (zero-dep leaf)    (zero-dep leaf)
   ┌────┴────┐                       │                  ↑
 tsv_ts   tsv_css                    │             tsv_discover
   │         │                       │             (policy layer)
   └────┬────┴───────────────────────┘
        ↓
   tsv_svelte   (depends on tsv_lang, tsv_ts, tsv_css, tsv_html)
        ↑
   ┌───────────┬─────────────┬──────────┬──────────────────┬─────────────────────┐
 tsv_cli     tsv_debug     tsv_ffi    tsv_napi           tsv_wasm
(production) (dev, Deno)   (C FFI)    (N-API, Node/Bun)  (browser/Node/Deno)
                ↑
            tsv_check  (experimental TypeScript binder/checker, may never ship —
                        depends on tsv_lang + tsv_ts; consumed ONLY by tsv_debug,
                        so no shipped format/parse artifact links it)
            tsv_svelte_compile  (experimental Svelte→JS compiler + JS canonicalizer —
                        depends on tsv_lang + tsv_html + tsv_ts + tsv_css + tsv_svelte;
                        consumed ONLY by tsv_debug, so no shipped artifact links it)

   tsv_cli, tsv_wasm, and tsv_napi also consume tsv_discover (→ tsv_ignore).
   tsv_ffi, tsv_napi, and tsv_wasm also consume tsv_arena — the shared
   binding substrate: per-thread reusable AST/doc arenas (→ bumpalo;
   → tsv_lang under `format`) plus the goal-axis macros their exports
   are generated over.
```

### Design Rationale

**Independent Consumption** — Use just `tsv_ts` without pulling in Svelte/CSS.

**Compile-Time Isolation** — Cargo prevents circular dependencies. CSS changes don't trigger TypeScript recompilation.

**Clean API Boundaries** — Each language exports `parse()`, `format()`, and `convert_ast_json_bytes()` / `convert_ast_json_string()` (with `convert_ast_json()` a thin `Value` wrapper over the bytes). tsv_ts and tsv_css also provide embedding APIs (`parse_embedded`, expression formatting, `build_*_doc`) used by tsv_svelte for nested language support.

**Scalability** — Easy to add new crates (`tsv_ffi`, `tsv_wasm`, `tsv_napi`, `tsv_arena`, `tsv_ignore` + `tsv_discover`, and the experimental `tsv_check` / `tsv_svelte_compile` — which may never ship — are all crate additions; `tsv_linter`/`tsv_lsp`/`tsv_md` planned).

### Closed Scope, Open Convention

tsv commits to a closed scope of languages (TypeScript, CSS, Svelte) but
its architecture is **open by convention at the Rust source/crate
level**. The shape of a "tsv language" is a social contract, not a Rust
trait:

```rust
pub fn parse<'a>(source: &str, arena: &'a Bump) -> Result<InternalAst<'a>, ParseError>;
pub fn format(ast: &InternalAst, source: &str) -> String;
pub fn convert_ast_json_bytes(ast: &InternalAst, source: &str) -> Vec<u8>;
pub fn convert_ast_json_string(ast: &InternalAst, source: &str) -> String;
pub fn convert_ast_json(ast: &InternalAst, source: &str) -> serde_json::Value;
```

`convert_ast_json_bytes` is the **sole emission path** — the hot path for
compact wire output (FFI, CLI non-pretty) and the source every other JSON
form derives from. In every language it is a **writer-mode conversion**
(`ast/convert/write*`) that emits the wire JSON directly during a single
walk of the *internal* AST — no typed public tree is ever materialized —
with byte→UTF-16 offset translation fused into the walk via `LocationMapper`
(final char-space positions emitted directly; ASCII sources are byte-space
passthrough). The output is valid UTF-8 by construction, and returning bytes
lets byte-oriented boundaries skip the O(output) UTF-8 validation a `String`
requires (the wire is ~20× the source); `convert_ast_json_string` is the
same bytes plus that one validation, for `&str` boundaries (the WASM
binding's `JSON.parse`, N-API strings), and `convert_ast_json` parses the
bytes back into a `serde_json::Value` for the `Value` consumers (the CLI's
`--pretty` tab-serialization, the fixture gate) — a thin wrapper, not an
independent conversion. Each of the three has a `_no_locations` sibling
(`convert_ast_json_bytes_no_locations` / `_string_no_locations`) emitting the
same wire minus every line/column object — the per-node `loc`, plus Svelte's
`name_loc` — so only `start`/`end` offsets remain. Line/column is a pure function
of an offset plus source, so the variant derives it lazily consumer-side rather
than emitting it (every package that parses ships that derivation as a pure-JS
`reconstruct_locations` helper); it's an opt-in span-only product mirroring
acorn's `locations: false`, not a second encoding of the drop-in wire, which
stays byte-identical. Each writer is a faithful emission of the acorn /
`parseCss` quirk catalog; the fixture suite gates its output against the
canonical parser's `expected.json` on every fixture (including the multibyte
and template-comment ones that exercise the fused offset translation and
island-scoped comment attach). tsv_svelte's template-expression comments
(outside `<script>`) fuse via an island-scoped attach that runs **online**,
off the writer's own node opens and closes (`tsv_ts`'s `CommentAttach`):
acorn assigns a node's leading comments at node entry and its trailing ones
after its children, which are exactly the two moments the writer already has,
and the wire emits both lists at the close — so there is no second pass, no
recorded tree, and no per-node map. `leadingComments` / `trailingComments`
serialize in place, and the walk's child-visit order is acorn's *by
construction*, since it IS the emitted field order. `<script>` content, block patterns,
`{@const}`/`{const}`/`{let}` declarations, and `<svelte:options>` fuse the
same way, and embedded `<style>` children fuse via `tsv_css`'s
`write_css_children`.

There is **no central `Language` trait, no plugin registry, no
language-set enum**. Each language crate (`tsv_ts`, `tsv_css`,
`tsv_svelte`) is self-contained and exports these free functions over
its own concrete types. Cross-crate dependencies exist only where
languages actually integrate — `tsv_svelte` depends on `tsv_ts` and
`tsv_css` because Svelte embeds them, not because of any central
abstraction.

This shape gives both:

- **Optimal artifacts** — concrete types end-to-end, no dyn dispatch,
  inlining works freely, WASM tree-shakes by language. A parse-only
  build (`@fuzdev/tsv_parse_wasm`) excludes printer code at the link level
  because nothing references it, and a format-only build
  (`@fuzdev/tsv_format_wasm`) compiles out the JSON-AST conversion layer via
  the lang crates' `convert` feature — build-time selection, not runtime
  feature flags.
- **Convention openness (Rust source level)** — anyone can write a
  `my_org/tsv_html_parse` crate following the same shape, and any
  downstream _Rust_ consumer can `use my_org_tsv_html_parse::parse`
  without central buy-in. The tsv crates are MIT-licensed and will
  eventually publish to crates.io, making this story concrete:
  third-party `tsv_*` crates can sit alongside the official ones in
  the Rust ecosystem.

  **Caveat**: this property holds at the Rust crate level, not the
  binary level. Users of the published `tsv` CLI or the npm packages
  (the WASM trio and the native `@fuzdev/tsv`) would need to compose
  their own dispatch to wire in a third-party language — the CLI
  matches on file extension over a fixed list, and the WASM
  `lang_bindings!` macro instantiates exports for a fixed set of
  language crates. Both are intentional: the binaries make scope
  commitments that the Rust libraries do not.

**Closing the platform at the Rust level** would mean adding any of:

- A `Language` trait with `dyn` dispatch — costs inlining, adds vtables.
- A central `tsv_ast` crate owning shared public/wire types — inverts
  per-language ownership; every language crate becomes a dependent of
  the central crate. (The wire shape is the hand-maintained
  `tsv_ast.d.ts`, not a Rust type layer; a future typed-as-reader crate
  would be the same inversion to avoid.)
- A `tsv_languages` enum in some core crate — forces editing a central
  place to add a language.

None of these are needed. The CLI dispatches by file extension with a
`match`; the WASM crate instantiates concrete per-language exports via
a macro. The set of supported languages is a _scope_ decision (lived
in those two dispatch sites), not a structural one — adding a
tsv-shaped crate to the workspace later requires no edits to existing
language crates.

The npm publish surface (`@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_parse_wasm`,
`@fuzdev/tsv_wasm`, and the native `@fuzdev/tsv` set) groups
artifacts for user ergonomics independent of the Rust workspace shape.

#### Cargo feature surface

`tsv_ts`, `tsv_css`, and `tsv_svelte` each expose a default-on `convert`
feature that gates `pub mod convert` (the writer) and the
`convert_ast_json_bytes` / `convert_ast_json_string` / `convert_ast_json`
free functions. The format-only WASM
build (`@fuzdev/tsv_format_wasm`) declares its language deps with
`default-features = false` so the convert layer is excluded at link
time; the parse-capable builds (`@fuzdev/tsv_parse_wasm` and the full
`@fuzdev/tsv_wasm`) opt in via the `tsv_wasm/parse` feature, which
forwards to each language crate's `convert`. The parse-only build
conversely omits the `tsv_wasm/format` feature, so the `format_*`
exports and the printers behind them drop at link time. `tsv_ffi`
carries the same `format`/`parse` feature pair (default both), so the
native C FFI binding tree-shakes identically — the benchmark builds
format-only and parse-only `libtsv_ffi` variants to size them
scope-matched against `oxfmt` and `oxc-parser`. Third-party
Rust consumers that only need parse/format can follow the same pattern:

```toml
# Minimal: parse + format only (source-level consumption — tsv is not on crates.io)
tsv_ts = { git = "https://github.com/fuzdev/tsv", default-features = false }

# Full: also build the wire-JSON parse output (`convert_ast_json*`)
tsv_ts = { git = "https://github.com/fuzdev/tsv", features = ["convert"] }
```

## Foundation Crate (tsv_lang)

Language-agnostic primitives shared across all implementations:

- `Span` — Source positions (u32 for memory efficiency)
- `LocationTracker` — Lazy line/column computation (O(log n) binary search)
- `ParseError` — Language-agnostic errors (String-based for flexibility)
- `doc` — **Document builder for Prettier-style formatting**
- `printing` — Shared formatting utilities (string literals, whitespace)
- `OutputBuffer` — Pre-allocated output string building with column tracking
- `config` — `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts, `EmbedContext`, `LayoutMode` (no runtime config)
- `comment` — Comment type and lookup utilities (see Comment Handling below)
- `escapes` — Escape sequence handling
- `source_scan` — Trivia-aware source scanning: the `skip_trivia` cursor + delimiter/keyword/regex finders (used by AST conversion, printers, and the parsers)

See [crates/tsv_lang/CLAUDE.md](../crates/tsv_lang/CLAUDE.md) for detailed module documentation.

### Shared Foundation Leverage

The doc builder is the formatting engine — the majority of tsv_lang by code volume. Language printers express layout as doc trees; the shared renderer handles width-aware breaking. This means the layout algorithm (group breaking, fill packing, look-ahead fitting) is written once and shared across all three languages.

Printers account for roughly half of language crate code. This is inherent to formatting — layout decisions (when to break, how to indent, where to attach comments, how to handle chains/assignment/ternaries) outnumber parsing decisions. It is not a sign of insufficient sharing; the shared doc builder already factors out the rendering algorithm.

Printer-private analysis functions (parenthesis requirements, expression complexity classification, byte-scanning utilities) were evaluated for extraction to tsv_lang and rejected — most encode layout decisions rather than general AST analysis; see [What Not to Extract](#what-not-to-extract).

Use `cargo run -p tsv_debug metrics` to measure the current shared vs language-specific code distribution.

### Sharing Analysis

What's shared through tsv_lang vs reimplemented per language, and why:

- Lexer (shared: No, should-be: No) — Different token sets, hot path — mode switching adds branches on every character
- Parser (shared: No, should-be: No) — Different grammars, precedence, context sensitivity
- AST types (shared: No, should-be: No) — Different semantics (TypeScript's expression grammar dwarfs CSS's node set)
- AST conversion (shared: No, should-be: No) — Language-specific JSON quirks (Svelte compatibility, etc.)
- Escape handling (shared: No, should-be: No) — JS has 7 escape formats, CSS has hex escapes with Svelte quirks
- Doc builder (shared: Yes, should-be: Yes) — Core formatting engine — the largest tsv_lang module, single renderer everywhere
- Comment model (shared: Yes, should-be: Yes) — Detached model with O(log n) lookup, classification, batch helpers
- Width / indent (shared: Yes, should-be: Yes) — Hardcoded as `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts in `tsv_lang::config`
- EmbedContext (shared: Yes, should-be: Yes) — Embedding state (base_indent_offset, first_line_offset, suffix_width, mode, jsdoc_cast_cannot_hang, root_sequence_indents)
- String formatting (shared: Yes, should-be: Yes) — Quote selection, escape swapping, visual width
- Error types (shared: Yes, should-be: Yes) — ParseError with context enrichment
- Position tracking (shared: Yes, should-be: Yes) — Span (u32), LocationTracker

**Code distribution** (from `cargo run -p tsv_debug metrics`):

```
foundation (tsv_lang + tsv_html): ~7% of codebase
languages (tsv_ts + tsv_css + tsv_svelte): ~49%
experimental compiler + checker (tsv_svelte_compile + tsv_check): ~20%
tooling (tsv_cli + tsv_debug + bindings): ~23%

printer % of language code: ~61%
```

The 7% foundation / 49% language split (the experimental compiler/checker and dev tooling counted separately) reflects genuine domain complexity, not missing extraction opportunities. The doc builder already factors out the rendering algorithm (the expensive shared part); what remains language-specific is the _formatting decisions_ themselves — when to break, how to indent, where to attach comments — which differ fundamentally between TypeScript, CSS, and Svelte.

### What Not to Extract

Patterns that _look_ duplicated but shouldn't be shared:

- **Lexer utilities** (peek/advance/skip_whitespace): Each lexer's hot loop is different. A shared trait would add vtable indirection on every character for no benefit.
- **Comment collection during parsing**: Each parser manually collects into `Vec<Comment>`. Simple enough that sharing would add abstraction without reducing code.
- **Printer analysis functions** (parenthesis requirements, expression complexity): These encode _layout decisions_ specific to each language. `needs_parens` in tsv_ts is the strongest extraction candidate (relevant to minifiers/transformers too) — but extraction should wait until a second consumer exists.

## Doc Builder System

The `doc` module implements a declarative document builder inspired by prettier's doc.js. Instead of imperatively deciding line breaks, formatters describe document structure and let the renderer decide layout based on print width.

### Core Types (Arena-Based)

Doc nodes are allocated in a contiguous `DocArena`. Each node is referenced by a `DocId` (a `u32` index), and child lists use `ChildRange` (start index + length). This eliminates per-node heap allocation and recursive `Drop` traversal. This fits the doc tree specifically: it's built once, rendered once, and dropped wholesale, so `DocId` indices are the natural access pattern. The AST is also bump-arena-allocated but uses **`&'arena` references, not `DocId` indices** — it's traversed repeatedly, so direct pointer access beats index lookups — see [Nested AST](#nested-ast-bump-arena-not-flatindexed).

```rust
pub enum DocNode {
    Text(DocText),                              // Static, pooled, source-span, or verbatim-span
    MultilineText { span, first_width },        // Pooled multi-line block-comment body
    Line(LineKind),                             // Normal, soft, hard, literal
    Indent(DocId),                              // Increase indent
    Dedent(DocId),                              // Decrease indent
    AlignRoot { n, contents },                  // Absolute tab level (template-literal root reset)
    Align { n, contents },                      // Sub-tab align(n): literal spaces under useTabs
    Group { contents, expanded_states, id, should_break },  // All-or-nothing breaking
    IfBreak { break_doc, flat_doc },            // Conditional on parent
    IndentIfBreak { contents, group_id },  // Conditional indent
    Concat(ChildRange),                         // Sequence
    Fill(ChildRange),                           // Greedy line packing
    WithContext { doc, context },                // Rendering hints
    LineSuffix(DocId),                          // End-of-line content
    LineSuffixBoundary,                         // Flush pending suffixes
    BreakParent,                                // Force parent group to break
}
```

### Key Algorithms

**Group Breaking** — Try flat mode first. If content exceeds print width, break all lines in the group (all-or-nothing).

**Fill Packing** — Pack items left-to-right, breaking only when next item doesn't fit. Used for CSS values, long attribute lists.

**Look-Ahead** — When checking if a group fits, examine what follows. `(longExpr)!.method()` needs to consider the suffix when deciding whether to break.

### DocText: Static, Pooled, SourceSpan, VerbatimSpan

```rust
pub enum DocText {
    Static(&'static str, u16),  // Punctuation, keywords — no allocation
    Pooled(PoolSpan, u16),      // Built text — copied once into the arena text pool at doc-build time
    SourceSpan(Span, u16),      // Verbatim source slice — resolved at print time, no allocation
    VerbatimSpan(Span, u16),    // SourceSpan for a format-ignored frozen slice — opaque to will_break
}
```

The `u16` is a cached visual width with two sentinel values (`TEXT_WIDTH_HAS_NEWLINE`, `TEXT_WIDTH_NOT_COMPUTED`). `Pooled`, `SourceSpan`/`VerbatimSpan`, and `Static` text always precompute at build — a real width or the newline sentinel — so `fits()` answers from the node alone (never borrowing the pool; for `Pooled` only the render loop reads the bytes, through one pool borrow hoisted per render) and `render_text`'s column advance skips its per-text byte scan. For `Static` the precompute is amortized through the arena's direct-mapped static width cache — measured once per *unique* string per arena (the address is a link-time constant, so the slot hash folds per `text()` call site), never per node (both the per-node eager measure and an inline-pointer→table-index narrowing were measured losses). The exception defers to on-demand measurement: the name slices (`source_span_ident` — high-frequency, newline-free, rarely fits-measured, so the build-time scan costs more than it saves; measured both ways).

The precompute itself (`pooled_text_width`) settles all three questions it needs — newline, ASCII, tab count — in **one** byte pass, because the text reaching it is overwhelmingly a short slice (a CSS property name, a value chunk) where three separate searchers cost more in *setup*, paid regardless of length, than one walk costs in total. Past a length threshold it flips to the searcher shape instead: there `contains('\n')` and `is_ascii` are SIMD and a tab count auto-vectorizes, so three vector passes beat one scalar walk. The gate is load-bearing rather than decorative — ungated, the fused walk is a measurable regression on TypeScript, whose text nodes run longer, while CSS never reaches it. **The correctness of this arithmetic is guarded by an exhaustive equivalence test beside the function and by nothing else**: a width only changes the output once it crosses the print width, so an error on a rare byte leaves every formatted file byte-identical and passes the fixture suite and any size of corpus diff. See [`crates/tsv_lang/CLAUDE.md`](../crates/tsv_lang/CLAUDE.md) and [`docs/performance.md`](performance.md#a-corpus-cannot-grade-arithmetic).

`Pooled` stores its bytes in the arena-owned text pool (a `String` on `DocArena`, indexed by `PoolSpan { start, len }`), and `MultilineText` bodies live there too — so `DocNode` carries **no drop glue** (`const`-asserted via `needs_drop`), and `DocArena::reset()`/drop free the node store without walking every node to run destructors on the <1% of nodes that would otherwise own `String`s. A printer with a ready-made slice passes it to `text_pooled(&str)`; one that must *assemble* the text streams it through `DocArena::pool_writer()` instead of building a transient `String` — the returned `PoolTextWriter` owns a scratch buffer parked on the arena (capacity retained across uses and `reset()`), holds no pool borrow while open (interleaved arena calls stay correct by construction), and its consume-on-finish `finish_text()`/`finish_multiline_text()` moves the bytes into the pool atomically. `SourceSpan` defers text resolution to print time, keyed on a source span (identifier / element / attribute names use its `source_span_ident` constructor, which also skips the width precompute — names are newline-free — so identifiers allocate and scan nothing during doc building): it stores a `Span` into the document `source` and resolves to the verbatim slice at print time (against the `source` threaded through the render entry points), so unmodified text — comments, template chunks, already-canonical literals (TS numbers/strings, CSS dimensions), Svelte markup text — emits with **no `String` allocation** and **no lifetime on `DocArena`** (the lifetime-free alternative to borrowing `&'src str` into the doc tree, which would forfeit the cross-file arena `reset()` reuse).

## Parser Architecture

All three parsers are **recursive descent** with **fail-fast error handling** (return `Result`, stop at the first error). Each parser owns a lexer and maintains a single-entry peek cache (`peek: Option<Token>`, the lexer's own token POD) to avoid re-lexing during lookahead. (Fail-fast is current, not final — spec-style error recovery is a tracked goal; see [Open Concerns](#open-concerns).)

### TypeScript (`tsv_ts/src/parser/`)

The TS parser is the most complex, using **Pratt parsing** for expressions with multi-phase infix handling:

```
expression.rs            — Pratt parser core (binding powers, prefix/infix/postfix dispatch, primary + paren)
expression_arrow.rs      — Arrow function predicates and builders
expression_assignable.rs — Cover-grammar conversion of an expression to an assignable pattern
expression_literals.rs   — Object and array literal parsing
expression_lookahead.rs  — Arrow/generic/type-assertion disambiguation (byte-scan)
expression_template.rs   — Template literal parsing
expression_type_args.rs  — Type-argument byte-scan lookahead (`<Type, …>` vs `<`)
scan.rs                  — Byte-level scanning utilities (fast lookahead without lexing)
parameters.rs            — Parameter and destructuring-pattern parsing
types.rs, type_members.rs — Type-syntax parsing (annotations, type expressions; interface/type-literal members)
statement/               — Statement parsing (variable, function, class, control flow, modules, types)
```

**Pratt binding powers** (higher = tighter):

```rust
BP_COMMA: 0          // Sequence (lowest)
BP_ASSIGNMENT: 1     // =, +=, ternary
BP_YIELD: 1          // yield — same as assignment (yield takes AssignmentExpression per spec)
BP_TS_TYPE_ASSERTION: 2  // as, satisfies
// ... binary operators 5-28 ...
BP_UNARY: 29         // -, !, typeof (highest)
```

The `parse_expression_bp(min_bp)` loop handles multiple phases in precedence order: binary operators, TypeScript type assertions (`as T`, `satisfies T`), assignment (right-associative), ternary, and comma.

**TypeScript ambiguity resolution** uses byte-level scanning (`scan.rs`) to disambiguate without full tokenization:

- **Arrow functions**: Scan for `identifier =>`, `(...) =>`, or `<T>(...) =>` patterns
- **Generics vs comparison**: Check for type parameter markers after `<`, scan to closing `>`
- **Type assertions**: `<T>expr` vs `a < b` — lookahead for type-like content between angles

Parser state flags manage context sensitivity: `allow_in` (disables `in` operator in for-loop headers), `top_level_as_is_assertion` (Svelte `#each` binding context, where a top-level `as` is the block's separator rather than TypeScript's operator), `grouping_depth` (parenthesis nesting), `in_ambient_context` (`declare` blocks).

### CSS (`tsv_css/src/parser/`)

Simpler recursive descent — no operator precedence needed:

```
mod.rs           — CssParser struct, top-level stylesheet loop
atrules.rs       — @media, @keyframes, @supports, etc.
selectors.rs     — Selector parsing
declarations.rs  — Rule bodies and property declarations
attributes.rs    — Attribute selectors
pseudo.rs        — Pseudo-class/pseudo-element selectors
value/           — Property value parsing (colors, dimensions, functions)
```

Uses `peek_past_whitespace()` with a temporary lexer to disambiguate declarations vs nested rules without consuming whitespace tokens.

### Svelte (`tsv_svelte/src/parser/`)

Template parser that **delegates** to tsv_ts and tsv_css for embedded content:

```
mod.rs             — Public entry points
parser_impl.rs     — SvelteParser struct, root parsing (script, style, markup ordering)
fragment.rs        — Fragment and text parsing
element.rs         — Element parsing
attribute.rs       — Attribute and directive parsing
block.rs           — Control flow blocks ({#if}, {#each}, {#await}, {#key})
expression_tag.rs  — {expr} → tsv_ts::parse_expression_with_comments()
script.rs          — <script> → tsv_ts::parse_embedded()
style.rs           — <style> → tsv_css::parse_embedded()
```

Script/style tag content is extracted by **raw byte scanning** for closing delimiters (`</script>`, `</style>`) — no tokenization inside tags.

### Multi-Language Embedding

Every embedded region shares the Svelte document's one bump arena (so the whole component is one bump-allocated graph); each embedded region gets a fresh parser instance. There is no shared symbol table — all names are span-identity, recovered from source.

Embedded parsers track `base_offset` so spans are absolute positions in the root source, not relative to tag content. Standalone parsing passes `base_offset = 0`.

Each language also has its own lexer — no mode switching, so the hot loops carry no per-character dispatch on language context. The cost is some structural duplication between the lexers, paid in source code rather than at runtime.

### Error Handling

All parsers are fail-fast. Error context (source line, column, caret) is **lazily computed** — the parser stores only the byte position, and `with_context(source)` extracts the surrounding line only when the error is displayed:

```rust
parser.parse().map_err(|e| e.with_context(source))
```

## Printer Architecture

Each language has a `printer/` module. Structure varies by language complexity:

**TypeScript** (`tsv_ts/src/printer/`):

```
mod.rs        # Printer struct, constructors, source/comment utilities
program.rs    # Program-level printing orchestration (statements, blank lines, comments)
decorators.rs # Decorator printing (class-level and class-member)
expressions/  # Expression formatting (literals, functions, patterns, blocks, objects, arrays, operators, assignment, conditionals, template literals)
statements/   # Statement formatting (classes, functions, modules, type declarations, variables; control_flow/ splits if/else, loops, switch, try/jump)
types/        # Type annotation formatting (composites, signatures, members, type params, unions)
calls/        # Call and `new` expression layout (argument wrapping, call-site comments, chained call args)
chain/        # Member expression chains (analysis, doc construction, rendering)
```

Cross-cutting concerns live in flat modules alongside these: parenthesis
requirements (`needs_parens.rs`), break-after-operator / fluid hanging-indent
primitives (`layout.rs`), comment printing helpers, and shared analysis
utilities.

**CSS** (`tsv_css/src/printer/`):

```
mod.rs                  # Printer struct, entry points
rules.rs                # Style rule formatting
selectors.rs            # Selector formatting
declarations.rs         # Property/value formatting
values.rs               # Value formatting
atrules.rs              # @-rule formatting
value_normalization.rs  # Semantic value normalization (numbers, colors, whitespace)
```

**Svelte** (`tsv_svelte/src/printer/`):

```
mod.rs              # Printer struct, entry points
attributes.rs       # Attribute formatting
text.rs             # Text node handling
script_style.rs     # <script>/<style> formatting
helpers.rs          # Shared utilities
nodes/              # Element and fragment printing
  element.rs        #   Element entry points (delegate to doc builders)
  element_doc.rs    #   Doc construction for HTML/component elements
  fragment_doc.rs   #   Doc construction for fragment content (text fill, node dispatch)
  blocks_doc.rs     #   Doc construction for control flow blocks ({#if}, {#each}, etc.)
  tags_doc.rs       #   Doc construction for template tags (@html, @const, {const}/{let}, @debug, @render)
  special_doc.rs    #   Doc construction for svelte:* special elements
  helpers.rs        #   Node-specific helpers
classification/     # HTML element classification (delegates to tsv_html)
  element.rs        #   Element type classification
```

### Hanging-Indent Layout (TypeScript)

The "break after an operator/keyword, then hang-indent the continuation" family
(`=`, `:`, `=>`, `as`, `satisfies`, `extends`, type-parameter `=`) is centralized
in `printer/layout.rs`, which exposes the two distinct shapes Prettier uses — and
they are **not** interchangeable:

- **`hang_after_operator`** — `group(indent([line, x]))`. The continuation `x` is
  inside the group, so a forced break inside `x` propagates and forces the break
  after the operator. Mirrors Prettier's `break-after-operator` (`printAssignment`)
  and `printUnionType` + `shouldIndentUnionType`.
- **`fluid_after_operator`** — `group(indent(line), {id})` + `lineSuffixBoundary` +
  `indentIfBreak(value, {id})`. The value sits outside the marker group, so an
  object-like type hugs `= {` / `extends {` and expands internally instead of
  dropping to the next line. Mirrors Prettier's `fluid` (`printAssignment`,
  `printTypeParameter`).

Intersection types use a related-but-distinct idiom (`group(indent(x))` with no
leading `line` — the first member stays on the operator line, continuations indent
with a trailing `&`), kept separate in `union_intersection.rs` /
`type_annotation.rs`. The continuation indent is owned by the caller — the
type-alias, annotation, and function-return callers wrap the result in `indent` —
except the generic `build_type_doc` path, where `build_intersection_type_doc`
self-owns it under `wrap_in_group` so nested positions (type arguments, tuple
elements, mapped-type values) indent their continuations correctly.

### Language Differences

| Feature          | TypeScript                     | CSS                     | Svelte                |
| ---------------- | ------------------------------ | ----------------------- | --------------------- |
| String Interning | Yes (identifiers)              | No                      | Yes (via tsv_ts)      |
| Escape Handling  | Dedicated module (7 formats)   | Dedicated module (hex)  | Delegates to TS/CSS   |
| Public API       | Core + broad embedding surface | Core + `parse_embedded` | Orchestrates TS + CSS |

### Line terminators: `parse` takes the author's bytes, `format` folds first

`parse` never rewrites its input — its offsets are a drop-in contract with acorn /
Svelte / `parseCss` over the bytes on disk. Every **parse-then-format** entry point, by
contrast, folds `<CR>` and `<CR><LF>` to `<LF>` *before* it parses
(`tsv_lang::printing::normalize_carriage_returns`, called from each language crate's
`format_str`, the CLI's `format_source`, each binding's format export, and
`tsv_svelte_compile`'s `canonicalize_js`). tsv's output is therefore
LF-only, including inside the regions it copies verbatim — a frozen embedded body, a
`format-ignore` region, a multi-line comment, a template literal.

Ahead of the parse is the only place the question can be answered once. The printers ask
*where are the lines?* in several places that split on `'\n'` alone — `Comment::multiline`
at parse, `is_indentable_block_comment` at doc-build, and the
per-line emitters under them — and a fold applied to the finished string leaves all of them
disagreeing with the output about where the lines are, so the same document formats two
ways on two passes. It is also where prettier folds (`normalizeEndOfLine`, in
`normalizeInputAndOptions`), and where the languages themselves do: HTML preprocesses its
input stream so that "there are never any U+000D CR characters in the input to the
tokenization stage", CSS Syntax §3.3 filters `<CR>` / `<FF>` / `<CR><LF>` to one `<LF>`,
and ECMAScript normalizes `<CR>` and `<CR><LF>` to `<LF>` in both TV and TRV. `<LS>` /
`<PS>` are deliberately untouched — ECMAScript keeps each as itself, and HTML and CSS read
them as ordinary characters.

### `loc` lines: two classes, one per acorn parse

The fold above is about the bytes tsv *writes*. The counting question is separate, and the
Svelte wire answers it two ways at once, because Svelte's parser does.

Svelte's own positions come from `locate-character`, which opens a line at `\n` and nothing
else — the template spine, `name_loc`, the CSS `loc`, a `<script>`'s `Program` `loc` (which
`read_script` re-stamps after the parse), and the identifiers `read_identifier` builds
(recognizable in the wire by the `character` field beside `line`/`column`). Everything acorn
parses carries acorn's `loc`, and acorn's is the ECMAScript class: `\n`, `\r`, `\r\n`, `<LS>`,
`<PS>`. Since `format` folds `<CR>` and `<LS>`/`<PS>` are vanishingly rare in real code, the two
agree on essentially every document — but where they disagree, a single table is a whole class
of silently-off positions.

**It is not "route acorn's islands to an ECMAScript table" either.** acorn seeds its line
counter *once per parse*, and Svelte hands it a differently prepared string at every island:

| island | source acorn receives | prefix counts as |
| --- | --- | --- |
| `<script>` (`read_script`) | prefix blanked with `replace(/[^\n]/g, ' ')` + content | LF only |
| `{expr}` / attribute values (`read_expression`) | the raw template | ECMAScript |
| `{const …}` / `{let …}` (`read_declaration` → `parse_statement_at`) | the raw template | ECMAScript |
| `{#snippet}` parameters | prefix `replace(/\S/g, ' ')` — whitespace survives | ECMAScript |
| a pattern binding — `{@const}`'s `id`, a destructured block binding (`read_pattern`) | blanked prefix + `(pattern = 1)` | LF only |
| a binding's trailing `: T` (`read_type_annotation`) | blanked prefix + `_ as ` + raw rest | LF only |

Note that `{@const}` is **one tag spanning both classes**: Svelte reads its `id` with
`read_pattern` and its `init` with `read_expression`, so the two halves of a single declarator
carry different prefix counts. (The unprefixed `{const …}` / `{let …}` tags are a different
construct on a different reader — one `parse_statement_at` over the raw template.)

And then, whatever the prefix, acorn *skips* `[lineStart, startPos)` outright — `lineStart` is
found with `lastIndexOf("\n", startPos - 1)`, so a non-LF terminator between that LF and the
island is counted by neither half.

`tsv_ts::AcornSeed` carries that per-parse difference as two constants over the
ECMAScript-rule tracker's answer (lines to subtract, columns to add on the region's first
line), and `tsv_svelte`'s parser records the parse start of every island in
`Root::acorn_regions` so the writer can rebuild the seed — including for the root `comments`
array, which is emitted outside the tree walk that would otherwise carry it. The second
tracker is built **only** when the two classes actually differ, which
`LocationTracker::new_with_map` reports out of the scan it already runs.

⚠️ **The route answers two questions, and each has its own exact condition.** They look
like one and are not:

1. **Which table** answers an acorn-owned position — acorn's ECMAScript one, or the Svelte
   LF one. Exactly "the two classes differ": a terminator *inside* an island moves every
   node after it whether or not any seed re-bases anything.
2. **Whether a seed** re-bases that answer. Exactly "some seed is not the identity".

Neither stands in for the other. Five of the six rows above seed from the class difference
alone, so a document without a lone `<CR>` / `<LS>` / `<PS>` leaves them inert — question 2
is `false` while question 1 may still be `true`. And the **annotation** row goes the other
way: acorn is entered five bytes behind the colon, on an `_ as ` that *overwrites* those
bytes, so a plain `\n` the author wrote between a block binding and its `: T` is erased
before acorn ever sees it and the annotation's nodes stay on the *binding's* line — question
2 is `true` on a pure-LF source where question 1 is `false`.

`tsv_svelte`'s writer therefore computes the seeds (one per region, once per document) and
activates the route when the classes differ **or** some seed is non-identity. Computing them
is itself gated by the cheap *necessary* condition — the classes differ, or some region has
`origin != lex_start`, which is exactly the parses that begin behind where they lex — so a
document with a glued `{#each xs as e: T}`, whose seed is the identity, pays a filter and
nothing more. When only question 2 holds the route answers through the LF table, which the
probe has certified is byte-identical to acorn's, so no second table is built. Pinned by
`tests/acorn_loc_line_terminators.rs`.

The region a position belongs to is the last one starting at or before it; regions **can**
nest (a block pattern's `: T` runs inside the pattern's own parse), where the later start is
the inner parse and so the right answer. Each region carries the extent of the slice its
sub-parse was handed, and the lookup asserts the resolved parse contains the position — the
one failure mode here is otherwise silent, since a position resolved to the previous parse
still emits a well-formed wire with only its lines moved.

**The same table answers a second question — the comment `value`.** acorn's `onComment`
dedents a multi-line block comment by the `[ \t]` run opening the comment's line *in the
string acorn was handed*, so the four manufactured rows above strip something the document
does not contain. `tsv_lang::AcornPrefix` models them (`Comment::wire_value`), resolved from
the same `Root::acorn_regions` by the same "last region starting at or before" rule — per
COMMENT rather than per island, since a block binding spans two of the rows
(`AcornPrefixes::at`, which both the root `comments` array and the attached
`leadingComments`/`trailingComments` copies read, so the two lists cannot disagree about one
comment).

The two questions are answered off **one** field, `AcornRegion::prefix` (an
`AcornPrefixText`), because they are one fact seen from two sides: a prefix blanked with
`/[^\n]/g` both erases the non-`\n` terminators (the line class,
`AcornPrefix::counts_ecmascript_lines` — what an `AcornSeed` is computed from) and turns the
author's indentation into spaces (the dedent's run, `AcornPrefix::line_indentation`). The
dedent's answer is the finer of the two — it alone tells `read_pattern` apart from its
siblings, which deletes one blank from its prefix so its synthetic `(` does not shift the
pattern's columns — so it is what the region stores and the line class is derived from it.
Carrying both would let a record site pass a pair that disagrees, and nothing could catch
that; the line class is also inert on a pure-LF document, so the disagreement would surface
nowhere. The two synthetic tokens differ in kind, and the difference is load-bearing for the
dedent alone: `read_pattern`'s `(` is **spliced** between prefix and region, where
`read_type_annotation`'s `_ as ` **overwrites** the five bytes it covers — it can swallow an
author's `\n` (the line then opens further back than the document's does), and a line opening
on the insert itself has no document indentation to read. The blanking is `String.replace`,
which lays one space per UTF-16 code unit, so any non-ASCII ahead of the comment on its line
makes a byte count too long. Pinned by `tests/comment_dedent_manufactured_source.rs` and the
frozen fixture `tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent`; the
line-terminator classes the dedent's two steps read are the sibling question, pinned by
`tests/comment_dedent_line_terminators.rs`.

### Source-Based Printing

All printers accept `source: &str` to preserve escape sequences:

```rust
// Extract raw from source (preserves escapes)
let raw = &source[span.start as usize..span.end as usize];

// vs. Format from decoded AST
write!("{}", value);
```

**When to extract raw:**

- String literals (preserve unicode escapes)
- CSS selectors/property names (preserve CSS escapes)
- Comments (preserve exact formatting)

**When to format from AST:**

- Numeric literals
- Keywords and operators
- Element tag names

**What the AST stores instead of raw text.** Verbatim source text is *not* cached
on nodes — it is recovered via `span.extract(source)` on demand (string, template,
regex, selector, and comment text all read from spans; `Text` keeps a `raw_span`
with a lazily-derived `Text::data()` that borrows the slice unless HTML entities are
present). Two kinds of owned data remain on nodes, both deliberate:

- **Genuinely decoded text** — `StringCooked::Decoded`, `TemplateCooked::Decoded`
  (escape sequences resolved). Both the `tsv_ts` and `tsv_css` string literals hold a
  `StringCooked` whose common `Verbatim` arm is span-recovered and allocation-free; only
  the `Decoded` arm — which a span can't reconstruct — is arena-allocated `&'arena str`.
  Don't "restore" a `Decoded` value to span extraction — verify a field is a *verbatim*
  source slice before assuming it's a redundant copy.
- **Precomputed derived scalars** — a node caches a small derived value (a `bool`/
  `u16`), never the raw text, so hot predicate readers stay source-free without
  re-scanning: `TemplateElement.has_newline` and `RegexLiteral.pattern_width` (the
  `tsv_ts` printer's `is_simple_call_argument` checks), `Comment.multiline`.

A handful of verbatim leaves whose *enclosing* span is larger than the leaf (a CSS
function name inside `name(args)`, an at-rule name after `@`, a declaration property,
a Svelte directive name inside `prefix:name|mods`) are still stored as `&'arena str`
rather than a dedicated leaf span — a benign, low-frequency exception, not a stored
raw cache of the printed text.

## Comment Handling

Comments are stored **separately from AST nodes** in a flat comment array at each root level (`Program.comments` — an arena slice `&'arena [Comment]`; `CssStyleSheet.comments` and `Root.comments` — `Vec<Comment>`). This is the "detached model" used by prettier.

### Core Type

```rust
pub struct Comment {
    pub content_span: Span,          // content WITHOUT delimiters; text via content(source)
    pub is_block: bool,              // true for /* */ or <!-- -->, false for //
    pub multiline: bool,             // content contains '\n' (precomputed; block-only in practice)
    pub span: Span,                  // full comment span, delimiters included
    pub emit_character_field: bool,  // Serializer hint: include `character` in JSON loc
    pub bump_pattern_columns: bool,  // Serializer hint: Svelte block-pattern column shift
    pub owned_by_node: bool,         // bound to the token after it; printed by that node,
                                     // not the enclosing gap (see docs/comments.md)
}
```

Comment content is **not stored owned**. The text is a pure delimiter-stripped
sub-slice of source (no decoding for JS/TS/CSS comments), so `Comment` keeps a
`content_span` and the text is recovered on demand via `Comment::content(source) ->
&str` (slicing the host document the spans were recorded against). This drops a
`String` allocation per comment in the lexer plus the parser's collect-clone, and
makes every field `Copy`. `multiline` is precomputed so the multi-line-block
expansion checks stay O(1) and never need `source`. A `#!` hashbang is a line
comment whose content includes the `#!` (no delimiter stripping); the lexer records
each comment's content start so derivation never has to re-guess delimiter widths.

### Lookup Functions

The `tsv_lang::comment` module provides O(log n) lookup via binary search. **Every
range lookup names the axis it answers on** — there is deliberately no axis-less
`comments_in_range`, because the three questions differ in whether an *owned*
comment counts, and a caller that doesn't say which it means is the bug. The axis
table (which caller asks which) is in [comments.md](./comments.md); the functions are:

| axis | range | after a position | existence |
| --- | --- | --- | --- |
| **to emit** — "which must *I* print here?" (owned skipped) | `comments_to_emit_in_range()` | `comments_to_emit_after()` | `has_comments_to_emit_in_range()` |
| **on page** — "does any comment occupy the page here?" (owned counted) | `comments_on_page_in_range()` | — | `has_comments_on_page_in_range()`, `has_multiline_block_comments_on_page_in_range()` |
| **in source** — "what bytes are physically here?" (owned counted) | `comments_in_source_range()` | `comments_in_source_after()` | — |

Plus the axis-independent primitives:

- `classify_comment()` — Determine if trailing, leading-own-line, or inline
- `classify_comment_fast()` — Same, using precomputed line breaks (faster)
- `ClassifiedComments::from_range()` — Batch classify all categories in one pass
- `find_first_comment_from()` — Binary-search index of first comment with `span.start >= pos`

### Printer Strategy

Printers find comments via range-based lookup between nodes — on the **to-emit**
axis when the question is "what do I print", which is the common case for a gap
between two siblings:

```rust
// Between two sibling nodes
let comments = comments_to_emit_in_range(self.comments, prev_end, node_start);

// Classify each comment
for comment in comments {
    match classify_comment(comment, prev_end, node_start, source) {
        Trailing => { /* attach to previous */ }
        LeadingOwnLine => { /* own line before next */ }
        LeadingInline => { /* same line as next */ }
    }
}
```

A layout gate over the same range asks `has_comments_on_page_in_range` instead —
an emit-keyed gate is blind to every owned comment it guards
([comments.md](./comments.md) names this as a standing corollary).

### Tradeoffs

- **Pro**: Simple AST, no duplication, memory efficient, matches prettier's model
- **Con**: Printers must manually track `prev_end` positions; edge cases require careful span math

Higher-level comment attachment helpers were evaluated for extraction to tsv_lang. The current primitives (binary search + classification) are the right abstraction. Per-printer comment handling is language-specific — each language has different rules for where comments attach relative to node types. Re-evaluate if genuine duplication emerges across multiple tools.

### Format-Ignore Directives

A `format-ignore` / `prettier-ignore` comment suppresses formatting of the construct that follows it (single directive), or — in Svelte templates — a `format-ignore-start` … `format-ignore-end` pair suppresses a range. Recognition is a thin string-level layer over this detached model: `tsv_lang::is_format_ignore_directive` (and `is_format_ignore_range_start` / `_end`) match the trimmed comment text and are the single source of truth for the directive set. Recognition alone doesn't honor: in TS a directive freezes only when it is **alone on its line** (the placement floor, `directive_alone_on_line` — any other placement is an ordinary comment; see [directives.md §Placement](./directives.md#placement)). Each printer checks the gap before a node for a recognized, placement-qualified directive and emits the node's raw source span (`span.extract(source)`) instead of a formatted doc. In a Svelte template the frozen span is that span less the one run that is not the construct: the parser folds a text node's own edge whitespace into it, and those runs are *separators* the printer still owns, so the trailing one is dropped wherever the boundary after it is the printer's to emit (`Printer::format_ignore_frozen_span`). The gap in FRONT of a frozen node is the author's and is printed exactly once — never doubled with a break the slice already carries, never invented where they glued the directive to the node (a break there injects a rendered space), never eaten where they wrote one (deleting it removes a rendered space). See [conformance_prettier_ignore.md](./conformance_prettier_ignore.md#format-ignore-directive). The tsv-native `format-ignore` family is canonical; the `prettier-ignore` family is honored as a compatibility alias for prettier-authored code. See [directives.md](./directives.md) and [conformance_prettier_ignore.md §Format-ignore directive](./conformance_prettier_ignore.md#format-ignore-directive).

## Allocation & Memory

Native tsv runs on the system allocator — no `#[global_allocator]`, no alternative-allocator dependency. The one exception is WebAssembly: `tsv_wasm` sets a wasm32-gated `#[global_allocator]` to [talc](https://github.com/SFBdragon/talc) (its `WasmGrowAndExtend` source), replacing std's default dlmalloc — the WASM format path is allocation-bound enough that the allocator itself was a measured wall, and the extend source holds the long-lived instance's linear-memory high-water at dlmalloc parity. The performance posture is otherwise structural: each layer avoids allocation by design rather than allocating faster.

**Lexing — spans, not strings.** Tokens store `u32` byte offsets (`start`, `end`) into the source, never slices or copies — `Token` is a 16-byte POD the byte cursor (`bytes: &[u8]` + `position`) emits and the parser unpacks in registers. The exception is deliberate: a string literal's decoded value is materialized only when it actually contains escape sequences, decoded into a **reused scratch buffer held out-of-band on the lexer** (`Lexer::decode_scratch`, borrowed via `decoded_str` and copied into the AST arena at receipt) so no per-literal `String` allocates and the per-token `Token` stays pointer-free. Comments are spans too — the token carries a `content_start` and the `Comment` node a `content_span`, recovered from source on demand and never copied.

**Internal AST — bump-arena nested ownership, span-identity names, no raw text.** Nodes are allocated in a per-parse bump arena: recursive children are `&'arena T` and child collections `&'arena [T]` (not `Box`/`Vec`), with small children kept inline by value (see [Nested AST](#nested-ast-bump-arena-not-flatindexed) for the layout rationale). Identifier names are span-identity — an `IdentName` records the raw name-token length and the name is re-sliced from source; only the rare `\u`-escaped (or oversized) name carries its decoded form as an `&'arena str`. Svelte element/attribute names are span-identity too (`source[name_span]`, `.trim()` for the padded-`{ shorthand }` edge), so there is no shared symbol table anywhere. Raw source text is never duplicated into the AST — printers re-slice via `span.extract(source)`; the few deliberate stored-raw caches are cataloged in [Source-Based Printing](#source-based-printing). What remains as owned data is genuinely decoded: string-literal values (only when escaped) and the like, arena-allocated as `&'arena str`.

**Svelte template nodes — contiguous storage.** Fragment children are an `&'arena [FragmentNode]` slice of enum values rather than boxed nodes, keeping siblings contiguous in arena memory for the printer's traversal loops.

**Doc building — the doc arena.** All doc nodes live in a contiguous `DocArena` (two flat `Vec`s: nodes and child lists, plus the text pool and an inline direct-mapped static cache), referenced by `u32` `DocId`s — no per-node heap allocation, no drop glue (`DocNode` is trivially droppable, `const`-asserted). Static text is **interned per document**: the static cache maps a `&'static str`'s address to both its precomputed visual width and the current document's node for it, so repeated `text(",")` calls return one shared `DocId` instead of allocating per call (`empty()` interns through a dedicated cell) — sound because statics are position-free at render, nodes are append-only, and no consumer compares `DocId` identity. The stateless singleton nodes intern the same way through dedicated generation-gated cells (no hash): the four `Line` kinds (direct-indexed by `LineKind` discriminant), `LineSuffixBoundary`, and `BreakParent` — a `Line` node carries no mode or indent (both are supplied per visit by the enclosing render command), the layout analog of "statics are position-free", so every `line()` in a document is one node. The single-shot `format()` path pre-sizes one arena from source length (~2 nodes per source byte, text pool at source/8; `DocArena::with_source_size_hint`) and drops it after rendering; multi-file drivers (the CLI dir-walk worker, the FFI/NAPI/WASM bindings) instead reuse one arena across calls via `DocArena::reset()` — clearing the node/child/text-pool/memo stores while retaining capacity (the static cache's width halves deliberately survive: they key on `'static` string addresses; the interned node halves are invalidated in O(1) by the reset's generation bump), the doc-IR analogue of the per-call AST `Bump::reset()` reuse — and the printers borrow `&DocArena` so the caller owns the reusable one (`format_in` is the borrowed-arena entry point). The builders' transient parts-lists are pooled the same way: wide-list builders (statement / object / array / parameter / specifier lists) draw a `DocBuf` from a recursion-safe arena free-list (`pooled_docbuf()`) rather than allocating a fresh `SmallVec` per call, so a document's many list-assembly spills collapse into a handful of long-lived reused buffers — byte-identical, allocation only. Embedded languages build doc nodes into the host file's arena rather than nesting their own. Identifier text never enters the doc tree: names emit as `DocText::SourceSpan` spans resolved at print time (see [DocText](#doctext-static-pooled-sourcespan-verbatimspan)); verbatim source text (comments, template chunks, Svelte markup text) is `SourceSpan` too, while a format-ignored frozen slice emits as the layout-opaque `VerbatimSpan` — and built text a printer actually constructs is copied once into the arena text pool (`Pooled`, assembled piecewise via `DocArena::pool_writer()` when no ready-made slice exists), so nodes themselves never own strings.

**Rendering — pre-sized output, stack-allocated scratch.** The per-render output `String` is reserved from arena node count (`DocArena::estimated_output_capacity`, clamped against pathological initial sizes), and the hot per-piece render-and-write seams (the TS/CSS printers' `write_arena_doc`, the Svelte printer's `render_doc_immediate` and `<script>`/`<style>` block renders) render through the `*_into` entry points into an arena-parked scratch buffer (`DocArena::take_render_scratch` / `park_render_scratch` — the render analog of `pool_writer()`'s parked scratch: one warm buffer per file instead of an alloc/free per rendered piece, with a fresh-fallback empty default so nested renders stay correct). `OutputBuffer` pre-allocates from source length for the Svelte printer's direct writes. The `fits()` lookahead and the render loop's own work-list both run on `SmallVec` stacks — the render command stack and its pending line-suffix buffer stay inline for the common small sub-render (the renderers run once per CSS declaration/value and per Svelte template expression, so each would otherwise allocate a fresh `Vec` from empty), and each top-level render additionally borrows the arena-pooled pair (`borrow_render_commands_scratch` / `borrow_line_suffix_scratch`) so their spill capacity warms once per arena instead of re-allocating per rendered piece (sub-renders keep their own inline locals and never take that borrow) — the per-render group-mode map is a fixed inline array keyed by the closed `GroupId` enum (no per-render `HashMap` allocation), and comment-classification buckets are `SmallVec`s sized for the common 0-2 comments case.

**Lazy work over eager caching.** Line/column positions are computed only at serialization time, via O(log n) binary search over newline offsets (`LocationTracker`). Error context (source line, caret) is extracted only when an error is displayed. Svelte `Text::data()` decodes entities only when entities are present, borrowing `raw` otherwise.

**Boundaries — serialize once, copy once.** `convert_ast_json_string` emits compact wire JSON without any intermediate `serde_json::Value` — all three languages write it directly from the internal AST via the writer (see [Closed Scope, Open Convention](#closed-scope-open-convention)) — into a buffer pre-sized from source length (`tsv_lang::estimated_json_capacity`, ~20 wire bytes per source byte — the JSON sibling of the render-path pre-sizing above). FFI returns a leaked `Box<[u8]>` the caller frees via `tsv_free` — one serialization, one buffer; the full ownership and panic-safety contract is in [crates/tsv_ffi/CLAUDE.md](../crates/tsv_ffi/CLAUDE.md). WASM ships the AST across the boundary as a single JSON string and hands it to the engine's native `JSON.parse` rather than building the JS object graph node-by-node. The CLI reads each file into one `String` and drops all per-file state before the next; worker threads share only an atomic index into the file list.

Profiling methodology — including when to reach for heap profiling — is in [performance.md](./performance.md).

## HTML Classification (tsv_html)

Pure functions for element classification, independent of any tool:

```rust
// Element classification
pub fn is_block_element(name: &str) -> bool;
pub fn is_void_element(name: &str) -> bool;
pub fn is_foreign_element(name: &str) -> bool;  // SVG/MathML
pub fn is_svg_element(name: &str) -> bool;
pub fn is_mathml_element(name: &str) -> bool;

// Whitespace and entities
pub fn preserves_whitespace(name: &str) -> bool;
pub fn decode_character_references(html: &str, is_attribute_value: bool) -> String;
```

Inline-ness is derived by negation in consumers (`!is_block_element(...)`) — no positive inline list is exported.

Enables reuse across formatter, linter, LSP, compiler without duplication.

## Fixture-Driven Development

Fixtures are **semantic test data** consumed by parser and formatter:

- Organized by features, not tools
- `input.svelte` is always canonical (formats to itself)
- `output_prettier.svelte` documents prettier differences
- `unformatted_*.svelte` variants test normalization
- Automatic validation enforces conventions

Scales at O(features) rather than O(tools × features).

## Key Design Decisions

- Two ASTs — Optimize internal for speed, public for compatibility
- Multi-crate — Compile isolation, independent consumption
- Closed scope, open convention — Per-language ownership; concrete types end-to-end; no central registry
- Separate lexers — Zero mode-switching overhead
- Pratt parsing — Clean operator precedence for TS expressions
- Span-identity names — no symbol table; identifier and element/attribute names are recovered from `source[span]`
- Detached comments — Simple AST, O(log n) lookup, matches prettier
- Doc builder — Prettier-style declarative formatting
- Source threading — Preserve escapes without AST duplication
- Lazy locations — Parse-time speed, serialize-time computation
- Fixtures as data — Reusable across tools, O(features) scaling

## Traversal and Extensibility

A generic `visit(node, callback)` across all three languages is not feasible — the AST types are fundamentally different (TypeScript's large expression grammar vs CSS's small node set vs Svelte's elements/text/blocks). No useful common `Node` trait exists.

tsv_svelte already does multi-language traversal in its printer: walk the Svelte AST, delegate to tsv_ts for `<script>`, delegate to tsv_css for `<style>`. Future tools (linter, LSP) would follow the same delegation pattern.

The crate structure scales to new languages and tools. A new language crate depends on `tsv_lang`, implements its own lexer/parser/AST/printer, and gets the doc builder formatting algorithm for free. A new tool (linter, LSP) consumes the same internal AST and adds its own layers (visitor traits, scope resolution, error recovery, etc.).

## Architectural Decisions

Decisions made during development with rationale preserved for future reference.

### Nested AST (Bump-Arena, Not Flat/Indexed)

tsv keeps the nested ownership model — not flat array layouts with index-based
references — but allocates the nodes in a **per-parse bump arena** (`bumpalo`)
tied to the program lifetime. Recursive children are `&'arena T<'arena>` (not
`Box`), child collections are `&'arena [T<'arena>]` (not `Vec`), and decoded
strings are `&'arena str`, so a whole parse is one bump-allocated graph freed
wholesale when the arena drops, with no per-node `Drop`:

```rust
pub struct Program<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub comments: &'arena [Comment], // arena-gathered; not the per-node target
    pub span: Span,
    // no interner / symbol table — identifier names are span-identity, recovered
    // from source[span]; the rare escaped name carries an &'arena str
}

pub enum Statement<'arena> {
    VariableDeclaration(VariableDeclaration<'arena>), // small → inline by value
    IfStatement(IfStatement<'arena>),                 // test inline; consequent: &'arena Statement
    // …
}
```

The caller owns the arena (`parse(source, &arena)`); the returned AST borrows it,
and `format`/`convert` consume it into an owned `String`/JSON, so the arena never
escapes the call (no self-referential ownership — `unsafe_code = "forbid"`, safe
bumpalo API only). Identifier and element/attribute names are **span-identity** —
recovered from `source[span]` at each consumer, with the rare escaped name carried
as an `&'arena str` — so the arena is the *only* thing threaded through
parse/format, and no name-table lifetime lands on the AST or the parser.

**Inline-by-value layout, size-minimized only at the variants that set an enum's
width.** A node holds its children inline by value where they were owned inline
before; only genuinely recursive children sit behind `&'arena`. Inline fields are
*not* indirected to shrink node size in general — the formatter is traversal-bound,
and the extra pointer-chases that size-minimization adds on hot traversal paths cost
more than the cache-density they buy. (The arena allocation itself is the win; the
node *layout* favors traversal locality over byte size.)

**The one deliberate exception is a density rule, and it is worth stating precisely,
because an enum's width is the ELEMENT width of everything that holds one.** An
`Expression` slot is paid on both of a `Property`'s key and value, on a
`VariableDeclarator`'s id and init, and on every element of every `&[Expression]`; a
`Statement` slot on every element of every statement slice and every
`?`-propagation copy out of the parser. So a variant wide enough to set its enum's
size on its own taxes the whole tree, while a pointer chase is paid only where that
variant is actually spelled. Where such a variant is also **rare**, it holds its
payload by `&'arena` reference:

- `Expression` is **72 B**, not 176 — `ClassExpression`, `FunctionExpression`,
  `ArrowFunctionExpression`, `MetaProperty` and `TaggedTemplateExpression` are boxed.
  Together they are ~3% of expressions in real source (the two widest, ~0.02%). The
  next-widest variant is `CallExpression` at 64 B and it is 14–21% of expressions,
  which is where the ladder stops.
- `Statement` is **104 B**, not 544 — its eight widest declaration heads
  (`TSTypeAliasDeclaration`, `ExportDefaultDeclaration`, `ClassDeclaration`,
  `FunctionDeclaration`, `TSInterfaceDeclaration`, `TSDeclareFunction`,
  `ExportAllDeclaration`, `TSImportEqualsDeclaration`) are boxed, and one level down
  `ForStatement`, `ForOfStatement`, `ForInStatement` and `TryStatement` hold their
  heads as references. Each of the eight is ≤0.2% of statements on real source and a
  classic `for (;;)` is 0.05–0.22%. The next-widest variants are `IfStatement` and
  `SwitchStatement` at 96 B, and `IfStatement` is 3–6% of statements, which is where
  the ladder stops.

Both widths are pinned by `const` asserts in `ast/internal/mod.rs`, so a variant that
widens either enum fails the build rather than silently fattening every slice element
and lowering the nesting ceiling. **Rarity is the whole of the argument** — the same
move on a common variant buys density and pays an allocation plus a pointer chase on
the majority of the traffic.

The fat inline nodes carry no by-value-return penalty in the **expression**
recursion, either: each node is built in the arena and threaded up the recursive
descent **by reference** (the expression parser's transient `ParsedExpr` wrapper
holds an `&'arena Expression`, not the node), so the recursion moves pointers
regardless of node size. The wrapper is kept register-returnable end to end — an
8-byte reference plus two `u32` paren-bound positions (16 bytes), with `ParseError`
boxing its own payload so the fallible `Result<ParsedExpr, ParseError>` is the same
16 bytes and returns in registers rather than through an sret stack slot. What that
buys is the *caller's frame* as much as the return: an arm that holds a node by
value reserves its bytes at every recursion level, whichever arm the dispatcher
takes, so the choice sets nesting depth rather than throughput — the per-construct
ceilings are in [cli.md §Recursion Depth](./cli.md#recursion-depth). The two
concerns are decoupled — node *layout* is tuned for the format traversal, while the
parse-time recursion cost is paid in pointer moves — so the *parse recursion* is
never the reason to box a fat inline variant. The density rule above is; and because
the enums sit on the expression and statement cycles, shrinking them moves the
nesting ceilings too, as a side effect rather than as the goal.

**Rationale vs flat/indexed:** Flat/indexed layouts (index arrays, à la Zig's
`MultiArrayList`) were benchmarked early in development and were slower —
traversal replaced direct pointer/reference access with index lookups, and a
formatter traverses constantly. The arena keeps direct `&'arena` access (full
traversal speed) while eliminating per-node `malloc`/`free` and improving locality
(nodes are bump-allocated in ≈parse order, which approximates traversal order).
The `DocArena` (the doc-builder IR) is the index-arena precedent for the
build-once/render-once doc tree; the AST uses references rather than indices
because it is traversed repeatedly.

**Still open (separate axis):** re-run the **flat/indexed structure** comparison
on the mature codebase — an independent question from allocation strategy (the
early prototype conflated the two). Bump allocation for the nested model is
the implemented design.

### Red-Green Trees (Deferred)

Don't add red-green tree infrastructure now. Evaluate when LSP development starts.

**Rationale:** Red-green adds complexity to parser and all consumers. Current parsing is sub-millisecond on typical source files (see [performance.md](./performance.md) for measurement methodology), but the real value of red-green is structural sharing for incremental _type checking_, not just parsing. rust-analyzer uses red-green despite fast parsing.

**Evaluation trigger:** When LSP work begins, benchmark with realistic workloads. If p95 latency exceeds 16ms on typical files, or if incremental type checking shows clear wins from structural sharing, revisit.

### Shared Parser, Divergent Tools

Share parser and AST across tools; let each tool add its own layers:

```
┌─────────────────────────────────────────────────────┐
│                    Shared Layer                     │
│  - Lexer (tsv_*/lexer/)                            │
│  - Parser (tsv_*/parser/)                          │
│  - Internal AST (tsv_*/ast/internal/)              │
│  - Wire-JSON writer (tsv_*/ast/convert/write/)     │
│  - Comment helpers (tsv_lang/comment)              │
└─────────────────────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
   ┌───────────┐   ┌───────────┐   ┌───────────┐
   │ Formatter │   │  Compiler │   │    LSP    │
   │           │   │           │   │           │
   │ printer/* │   │ HIR/IR    │   │ red-green │
   │ (current) │   │ codegen   │   │ wrapper   │
   └───────────┘   └───────────┘   └───────────┘
```

**Rationale:** Formatter is stable; compiler needs transforms/codegen that formatter doesn't; LSP needs incremental parsing that CLI tools don't. Each tool optimizes for its needs.

### Positioning vs. oxc and Biome

The closest Rust projects embody the alternative shapes, which makes the trade-offs concrete:

- **[oxc](https://github.com/oxc-project/oxc)** is single-language (JS/TS). Its signature
  choice — one central `oxc_ast` crate shared by parser, linter, transformer, minifier, and
  formatter — answers a different question: many _tools_ sharing one language's AST. tsv does
  the same per language (see [Shared Parser, Divergent Tools](#shared-parser-divergent-tools));
  the per-language crate split is the multi-language question oxc never faces. Allocation is
  convergent: like oxc, tsv bump-allocates lifetime-threaded (`&'arena`) AST types — but
  keeps an **inline-by-value node layout** (not size-minimized via boxing/indirection, which
  regressed its traversal-bound formatter; see [Nested AST](#nested-ast-bump-arena-not-flatindexed)),
  stays `unsafe_code = "forbid"` (safe bumpalo API only), and recovers source text via `span`
  slices rather than zero-copy atoms. The other convergences are just as real: u32 spans,
  detached comments stored flat on the program, concrete types without dyn dispatch,
  prettier-style doc IR.
- **[Biome](https://biomejs.dev/)** is multi-language like tsv and chose the centralized shape
  tsv rejects: a shared red-green CST (rowan) with unified formatter infrastructure across
  languages, comments attached to tokens as trivia. tsv instead keeps concrete per-language
  ASTs with detached comments, defers red-green until LSP work shows the need (see
  [Red-Green Trees](#red-green-trees-deferred)), and gets link-level tree-shaking per artifact
  in exchange.

## Open Concerns

Issues that need architectural decisions before building future tools.

- **Scope/symbol resolution** — The shipped ASTs are syntax-only; meaningful linting requires name resolution. The experimental `tsv_check` binder (`crates/tsv_check`, consumed only by `tsv_debug`) is the candidate answer, but nothing shipped links it. *(When: before linter.)*
- **Error recovery** — Fail-fast parsers block LSP/linter (need partial ASTs from broken code); also required for full CSS-spec compliance — CSS Syntax 3 §5.5 recovery (drop the bad rule, keep parsing), see conformance_svelte.md §CSS Parser Scope. *(When: for full CSS-spec compliance (CSS) / before LSP/linter.)*
- **Source maps** — A shipped compiler must map output positions to input; the experimental `tsv_svelte_compile` emits none. How do spans survive transforms? *(When: before the compiler ships.)*
- **Cancellation** — LSP operations must be cancellable mid-parse. Current parser has no cancellation points. *(When: before LSP.)*

## References

- [Flattening ASTs](https://www.cs.cornell.edu/~asampson/blog/flattening.html) — Adrian Sampson on arena patterns (context for Nested AST decision)
- [Zig Parser](https://mitchellh.com/zig/parser) — Mitchell Hashimoto on Zig's MultiArrayList AST
- [Prettier Technical Details](https://prettier.io/docs/en/technical-details) — comment attachment heuristics
- [OXC AST](https://github.com/oxc-project/oxc) — central shared AST + arena allocation in Rust (the contrasting design; see [Positioning vs. oxc and Biome](#positioning-vs-oxc-and-biome))
