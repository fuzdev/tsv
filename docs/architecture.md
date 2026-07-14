# Architecture

Design decisions and technical rationale for tsv.

## Core Philosophy

tsv is a **multi-tool foundation** for Svelte/TypeScript/CSS—formatter, parser, and future linter/LSP. JSON serialization for testing compatibility is secondary to efficient internal manipulation.

This inverts the typical approach where JSON compatibility drives AST design.

**Optimal artifacts (invariant).** Runtime speed _and_ compiled code size are first-class, non-negotiable goals for **every** shipped artifact. The format-only `@fuzdev/tsv_format_wasm` is the current yardstick—it's the most-developed and first-shipped artifact—but it holds no long-term primacy; `@fuzdev/tsv_parse_wasm`, the CLI, and future bindings count just as much as they mature. The architecture serves this directly: concrete types end-to-end (no `dyn` dispatch), per-language crates that WASM tree-shakes independently, and unneeded layers excluded at the link level — the printers from parse-only builds, the convert layer from format-only builds (see §"Closed Scope, Open Convention"). Heavier infrastructure for future tools—incremental reparse, red-green/CST layers for LSP—must be added as later, feature-gated layers that don't regress this, not as weight in the initial artifacts (see §"Red-Green Trees (Deferred)").

**Safety constraint**: `unsafe_code = "forbid"` at the workspace level — no unsafe Rust in core crates. The `tsv_ffi` crate overrides to `"allow"` for the C ABI boundary. Combined with a single-digit core-library dependency set (authoritative list: `[workspace.dependencies]` in the root `Cargo.toml`, whose externals are the nine library crates plus the CLI/debug-only `argh`/`tokio`/`futures-util`; purpose table in [CLAUDE.md § Rust Crates](../CLAUDE.md#rust-crates-minimal-deps)), the attack surface and audit burden stay minimal.

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
├── tsv_cli      # Production CLI binary (pure Rust)
├── tsv_debug    # Dev utilities (uses embedded Deno sidecar for JS tools)
├── tsv_ffi      # C FFI bindings
└── tsv_wasm     # WebAssembly bindings
```

`tsv_html` and `tsv_ignore` are independent zero-`tsv_*`-dep leaves (pure
functions). `tsv_discover` is a thin policy layer whose only `tsv_*` dep is
`tsv_ignore` — it owns the build-output heuristic + safety-net pruning *decision*
(the matcher stays a pure gitignore(5) matcher). Both are consumed by `tsv_cli`
directly and by `tsv_wasm` under its `format` feature (the matcher exposed as the
`IgnoreStack` class, the policy as that class's verdict methods), so the CLI, the
WASM CLI, and the VS Code extension all share one discovery matcher *and* one
prune decision. `tsv_discover` is file-*scope* policy — the one sanctioned config
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

   tsv_cli and tsv_wasm also consume tsv_discover (→ tsv_ignore).
   tsv_ffi, tsv_napi, and tsv_wasm also consume tsv_arena — per-thread
   reusable AST/doc arenas (→ bumpalo; → tsv_lang under `format`).
```

### Design Rationale

**Independent Consumption** — Use just `tsv_ts` without pulling in Svelte/CSS.

**Compile-Time Isolation** — Cargo prevents circular dependencies. CSS changes don't trigger TypeScript recompilation.

**Clean API Boundaries** — Each language exports `parse()`, `format()`, and `convert_ast_json_bytes()` / `convert_ast_json_string()` (with `convert_ast_json()` a thin `Value` wrapper over the bytes). tsv_ts and tsv_css also provide embedding APIs (`parse_with_interner`, `parse_embedded`, expression formatting, `build_*_doc`) used by tsv_svelte for nested language support.

**Scalability** — Easy to add new crates (`tsv_ffi`, `tsv_wasm` already done; `tsv_linter`/`tsv_lsp`/`tsv_md` planned).

### Closed Scope, Open Convention

tsv commits to a closed scope of languages (TypeScript, CSS, Svelte) but
its architecture is **open by convention at the Rust source/crate
level**. The shape of a "tsv language" is a social contract, not a Rust
trait:

```rust
pub fn parse(source: &str) -> Result<InternalAst, ParseError>;
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
than emitting it (the parse WASM packages ship that derivation as a pure-JS
`reconstruct_locations` helper); it's an opt-in span-only product mirroring
acorn's `locations: false`, not a second encoding of the drop-in wire, which
stays byte-identical. Each writer is a faithful emission of the acorn /
`parseCss` quirk catalog; the fixture suite gates its output against the
canonical parser's `expected.json` on every fixture (including the multibyte
and template-comment ones that exercise the fused offset translation and
island-scoped comment attach). tsv_svelte's template-expression comments
(outside `<script>`) fuse via an island-scoped attach pass: each
comment-bearing island's wire node tree is recorded structurally during a
byte-space skeleton emit (`SkeletonRecorder` — open/close events from the
writer itself, never a re-parse of the emitted bytes), the shared acorn
attach walks the recorded tree, and the assignments fold into a span-keyed
map the fused writer consults at each node's close, so `leadingComments` /
`trailingComments` serialize in place. `<script>` content, block patterns,
`{@const}`/`{const}`/`{let}` declarations, and `<svelte:options>` fuse the
same way, and embedded `<style>` children fuse via `tsv_css`'s
`write_css_node`.

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
  binary level. Users of the published `tsv` CLI or the WASM packages
  (`@fuzdev/tsv_format_wasm` / `@fuzdev/tsv_parse_wasm`) would need to compose
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

The npm publish surface (`@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_parse_wasm`) groups
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
# Minimal: parse + format only
tsv_ts = { version = "0.1", default-features = false }

# Full: also build the wire-JSON parse output (`convert_ast_json*`)
tsv_ts = { version = "0.1", features = ["convert"] }
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
- `interner` — String interner utilities (`SymbolResolver` trait)
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
- String interning (shared: Yes, should-be: Yes) — Traits + shared interner across TS/Svelte in same file
- Width / indent (shared: Yes, should-be: Yes) — Hardcoded as `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts in `tsv_lang::config`
- EmbedContext (shared: Yes, should-be: Yes) — Embedding knobs (base_indent_offset, first_line_offset, suffix_width, mode)
- String formatting (shared: Yes, should-be: Yes) — Quote selection, escape swapping, visual width
- Error types (shared: Yes, should-be: Yes) — ParseError with context enrichment
- Position tracking (shared: Yes, should-be: Yes) — Span (u32), LocationTracker

**Code distribution** (from `cargo run -p tsv_debug metrics`):

```
foundation (tsv_lang + tsv_html): ~7% of codebase
languages (tsv_ts + tsv_css + tsv_svelte): ~82%
tooling (tsv_cli + tsv_debug + bindings): ~11%

printer % of language code: ~50%
```

The 7% foundation / 82% language split reflects genuine domain complexity, not missing extraction opportunities. The doc builder already factors out the rendering algorithm (the expensive shared part); what remains language-specific is the _formatting decisions_ themselves — when to break, how to indent, where to attach comments — which differ fundamentally between TypeScript, CSS, and Svelte.

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
    Text(DocText),                              // Static, pooled, source-span, or symbol
    MultilineText { span, first_width },        // Pooled multi-line block-comment body
    Line(LineKind),                             // Normal, soft, hard, literal
    Indent(DocId),                              // Increase indent
    Dedent(DocId),                              // Decrease indent
    Align { n, contents },                      // Absolute indentation
    Group { contents, expanded_states, id, should_break },  // All-or-nothing breaking
    IfBreak { break_doc, flat_doc },            // Conditional on parent
    IndentIfBreak { contents, group_id, negate },  // Conditional indent
    Concat(ChildRange),                         // Sequence
    Fill(ChildRange),                           // Greedy line packing
    WithContext { doc, context },                // Rendering hints
    LineSuffix(DocId),                          // End-of-line content
    LineSuffixBoundary,                         // Flush pending suffixes
    BreakParent,                                // Force parent group to break
    IsolatedGroup { contents },                 // Prevent hardline propagation
}
```

### Key Algorithms

**Group Breaking** — Try flat mode first. If content exceeds print width, break all lines in the group (all-or-nothing).

**Fill Packing** — Pack items left-to-right, breaking only when next item doesn't fit. Used for CSS values, long attribute lists.

**Look-Ahead** — When checking if a group fits, examine what follows. `(longExpr)!.method()` needs to consider the suffix when deciding whether to break.

### DocText: Static, Pooled, SourceSpan, Symbol

```rust
pub enum DocText {
    Static(&'static str, u16),  // Punctuation, keywords — no allocation
    Pooled(PoolSpan, u16),      // Built text — copied once into the arena text pool at doc-build time
    SourceSpan(Span, u16),      // Verbatim source slice — resolved at print time, no allocation
    Symbol(u32),                // Interned identifier — resolved at print time
}
```

The `u16` is a cached visual width with two sentinel values (`TEXT_WIDTH_HAS_NEWLINE`, `TEXT_WIDTH_NOT_COMPUTED`). `Pooled`, `SourceSpan`, and `Static` text always precompute at build — a real width or the newline sentinel — so `fits()` answers from the node alone (never borrowing the pool; for `Pooled` only the render loop reads the bytes, through one pool borrow hoisted per render) and `render_text`'s column advance skips its per-text byte scan. For `Static` the precompute is amortized through the arena's direct-mapped static width cache — measured once per *unique* string per arena (the address is a link-time constant, so the slot hash folds per `text()` call site), never per node (both the per-node eager measure and an inline-pointer→table-index narrowing were measured losses). The exceptions defer to on-demand measurement: identifier names (`source_span_ident` — high-frequency, newline-free, rarely fits-measured, so the build-time scan costs more than it saves; measured both ways) and `Symbol`, which defers both resolution and width to print time.

The precompute itself (`pooled_text_width`) settles all three questions it needs — newline, ASCII, tab count — in **one** byte pass, because the text reaching it is overwhelmingly a short slice (a CSS property name, a value chunk) where three separate searchers cost more in *setup*, paid regardless of length, than one walk costs in total. Past a length threshold it flips to the searcher shape instead: there `contains('\n')` and `is_ascii` are SIMD and a tab count auto-vectorizes, so three vector passes beat one scalar walk. The gate is load-bearing rather than decorative — ungated, the fused walk is a measurable regression on TypeScript, whose text nodes run longer, while CSS never reaches it. **The correctness of this arithmetic is guarded by an exhaustive equivalence test beside the function and by nothing else**: a width only changes the output once it crosses the print width, so an error on a rare byte leaves every formatted file byte-identical and passes the fixture suite and any size of corpus diff. See [`crates/tsv_lang/CLAUDE.md`](../crates/tsv_lang/CLAUDE.md) and [`docs/performance.md`](performance.md#a-corpus-cannot-grade-arithmetic).

`Pooled` stores its bytes in the arena-owned text pool (a `String` on `DocArena`, indexed by `PoolSpan { start, len }`), and `MultilineText` bodies live there too — so `DocNode` carries **no drop glue** (`const`-asserted via `needs_drop`), and `DocArena::reset()`/drop free the node store without walking every node to run destructors on the <1% of nodes that would otherwise own `String`s. A printer with a ready-made slice passes it to `text_pooled(&str)`; one that must *assemble* the text streams it through `DocArena::pool_writer()` instead of building a transient `String` — the returned `PoolTextWriter` owns a scratch buffer parked on the arena (capacity retained across uses and `reset()`), holds no pool borrow while open (interleaved arena calls stay correct by construction), and its consume-on-finish `finish_text()`/`finish_multiline_text()` moves the bytes into the pool atomically. `SourceSpan` is the same deferral keyed on a span instead of an interner id (identifier names use its `source_span_ident` constructor, which also skips the width precompute — names are newline-free — so identifiers allocate and scan nothing during doc building): it stores a `Span` into the document `source` and resolves to the verbatim slice at print time (via a source-aware `TextResolver`), so unmodified text — comments, template chunks, already-canonical literals (TS numbers/strings, CSS dimensions), Svelte markup text — emits with **no `String` allocation** and **no lifetime on `DocArena`** (the lifetime-free alternative to borrowing `&'src str` into the doc tree, which would forfeit the cross-file arena `reset()` reuse).

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

Parser state flags manage context sensitivity: `allow_in` (disables `in` operator in for-loop headers), `allow_ts_type_assertions` (Svelte `#each` binding context), `grouping_depth` (parenthesis nesting), `in_ambient_context` (`declare` blocks).

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
script.rs          — <script> → tsv_ts::parse_with_interner()
style.rs           — <style> → tsv_css::parse_embedded()
```

Script/style tag content is extracted by **raw byte scanning** for closing delimiters (`</script>`, `</style>`) — no tokenization inside tags.

### Multi-Language Embedding

The Svelte parser shares a single `Rc<RefCell<StringInterner>>` with tsv_ts, so identifiers are deduplicated across template expressions and script blocks. Each embedded region gets a fresh parser instance — reusing one would require `reset()` (bug-prone, error-unsafe) to save only a small fixed allocation per region.

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

Comments are stored **separately from AST nodes** in a flat `Vec<Comment>` at each root level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`). This is the "detached model" used by prettier.

### Core Type

```rust
pub struct Comment {
    pub content_span: Span,        // content WITHOUT delimiters; text via content(source)
    pub is_block: bool,            // true for /* */, false for //
    pub multiline: bool,           // content contains '\n' (precomputed; block-only in practice)
    pub span: Span,                // full comment span, delimiters included
    pub emit_character_field: bool, // Serializer hint: include `character` in JSON loc
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

The `tsv_lang::comment` module provides O(log n) lookup via binary search:

- `comments_in_range()` — Find comments between two positions (O(log n))
- `classify_comment()` — Determine if trailing, leading-own-line, or inline
- `classify_comment_fast()` — Same, using precomputed line breaks (faster)
- `ClassifiedComments::from_range()` — Batch classify all categories in one pass
- `has_comments_in_range()` — Quick existence check
- `comments_after()` — Iterate comments at or after a position (O(log n))
- `find_first_comment_from()` — Binary-search index of first comment with `span.start >= pos`

### Printer Strategy

Printers find comments via range-based lookup between nodes:

```rust
// Between two sibling nodes
let comments = comments_in_range(&self.comments, prev_end, node_start);

// Classify each comment
for comment in comments {
    match classify_comment(comment, prev_end, node_start, source) {
        Trailing => { /* attach to previous */ }
        LeadingOwnLine => { /* own line before next */ }
        LeadingInline => { /* same line as next */ }
    }
}
```

### Tradeoffs

- **Pro**: Simple AST, no duplication, memory efficient, matches prettier's model
- **Con**: Printers must manually track `prev_end` positions; edge cases require careful span math

Higher-level comment attachment helpers were evaluated for extraction to tsv_lang. The current primitives (binary search + classification) are the right abstraction. Per-printer comment handling is language-specific — each language has different rules for where comments attach relative to node types. Re-evaluate if genuine duplication emerges across multiple tools.

### Format-Ignore Directives

A `format-ignore` / `prettier-ignore` comment suppresses formatting of the construct that follows it (single directive), or — in Svelte templates — a `format-ignore-start` … `format-ignore-end` pair suppresses a range. Recognition is a thin string-level layer over this detached model: `tsv_lang::is_format_ignore_directive` (and `is_format_ignore_range_start` / `_end`) match the trimmed comment text and are the single source of truth for the directive set. Each printer checks them via `comments_in_range()` in the gap before a node and emits the node's raw source span (`span.extract(source)`) instead of a formatted doc. The tsv-native `format-ignore` family is canonical; the `prettier-ignore` family is honored as a drop-in alias. See [directives.md](./directives.md) and [conformance_prettier.md §Format-ignore directive](./conformance_prettier.md#format-ignore-directive).

## Allocation & Memory

Native tsv runs on the system allocator — no `#[global_allocator]`, no alternative-allocator dependency. The one exception is WebAssembly: `tsv_wasm` sets a wasm32-gated `#[global_allocator]` to [talc](https://github.com/SFBdragon/talc) (its `WasmGrowAndExtend` source), replacing std's default dlmalloc — the WASM format path is allocation-bound enough that the allocator itself was a measured wall, and the extend source holds the long-lived instance's linear-memory high-water at dlmalloc parity. The performance posture is otherwise structural: each layer avoids allocation by design rather than allocating faster.

**Lexing — spans, not strings.** Tokens store `u32` byte offsets (`start`, `end`) into the source, never slices or copies — `Token` is a 16-byte POD the byte cursor (`bytes: &[u8]` + `position`) emits and the parser unpacks in registers. The exception is deliberate: a string literal allocates its decoded value only when it actually contains escape sequences, held **out-of-band on the lexer** (`Lexer::decoded`, drained via `take_decoded`) so the per-token `Token` stays pointer-free. Comments are spans too — the token carries a `content_start` and the `Comment` node a `content_span`, recovered from source on demand and never copied.

**Internal AST — bump-arena nested ownership, span-identity names, no raw text.** Nodes are allocated in a per-parse bump arena: recursive children are `&'arena T` and child collections `&'arena [T]` (not `Box`/`Vec`), with small children kept inline by value (see [Nested AST](#nested-ast-bump-arena-not-flatindexed) for the layout rationale). Identifier names are span-identity — an `IdentName` records the raw name-token length and the name is re-sliced from source; only the rare `\u`-escaped (or oversized) name interns its decoded form, and the interner is shared across embedded languages in a Svelte file (its main tenants are the Svelte parser's element/attribute names). Raw source text is never duplicated into the AST — printers re-slice via `span.extract(source)`; the few deliberate stored-raw caches are cataloged in [Source-Based Printing](#source-based-printing). What remains as owned data is genuinely decoded: string-literal values (only when escaped) and the like, arena-allocated as `&'arena str`.

**Svelte template nodes — contiguous storage.** Fragment children are an `&'arena [FragmentNode]` slice of enum values rather than boxed nodes, keeping siblings contiguous in arena memory for the printer's traversal loops.

**Doc building — the doc arena.** All doc nodes live in a contiguous `DocArena` (two flat `Vec`s: nodes and child lists, plus the text pool and an inline direct-mapped static cache), referenced by `u32` `DocId`s — no per-node heap allocation, no drop glue (`DocNode` is trivially droppable, `const`-asserted). Static text is **interned per document**: the static cache maps a `&'static str`'s address to both its precomputed visual width and the current document's node for it, so repeated `text(",")` calls return one shared `DocId` instead of allocating per call (`empty()` interns through a dedicated cell) — sound because statics are position-free at render, nodes are append-only, and no consumer compares `DocId` identity. The stateless singleton nodes intern the same way through dedicated generation-gated cells (no hash): the four `Line` kinds (direct-indexed by `LineKind` discriminant), `LineSuffixBoundary`, and `BreakParent` — a `Line` node carries no mode or indent (both are supplied per visit by the enclosing render command), the layout analog of "statics are position-free", so every `line()` in a document is one node. `Symbol` nodes intern per document too, through a generation-gated table direct-indexed by the interner id (ids are small dense per-document integers, so no hash probe) — repeated `symbol(id)` calls for the same element/attribute name return one shared node. The single-shot `format()` path pre-sizes one arena from source length (~2 nodes per source byte, text pool at source/8; `DocArena::with_source_size_hint`) and drops it after rendering; multi-file drivers (the CLI dir-walk worker, the FFI/NAPI/WASM bindings) instead reuse one arena across calls via `DocArena::reset()` — clearing the node/child/text-pool/memo stores while retaining capacity (the static cache's width halves deliberately survive: they key on `'static` string addresses; the interned node halves are invalidated in O(1) by the reset's generation bump), the doc-IR analogue of the per-call AST `Bump::reset()` reuse — and the printers borrow `&DocArena` so the caller owns the reusable one (`format_in` is the borrowed-arena entry point). The builders' transient parts-lists are pooled the same way: wide-list builders (statement / object / array / parameter / specifier lists) draw a `DocBuf` from a recursion-safe arena free-list (`pooled_docbuf()`) rather than allocating a fresh `SmallVec` per call, so a document's many list-assembly spills collapse into a handful of long-lived reused buffers — byte-identical, allocation only. Embedded languages build doc nodes into the host file's arena rather than nesting their own. Identifier text never enters the doc tree: names emit as `DocText::SourceSpan` spans resolved at print time, and `DocText::Symbol` stores an interner ID (Svelte element/attribute names, escaped identifiers) resolved the same way (see [DocText](#doctext-static-pooled-sourcespan-symbol)); verbatim source text (comments, template chunks, Svelte markup text) is `SourceSpan` too — and built text a printer actually constructs is copied once into the arena text pool (`Pooled`, assembled piecewise via `DocArena::pool_writer()` when no ready-made slice exists), so nodes themselves never own strings.

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
- Shared interner — one per document for what still interns (Svelte element/attribute names, escaped identifiers); identifier names are span-identity
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
    pub comments: Vec<Comment>, // root-owned; not the per-node target
    pub span: Span,
    // …interner (Rc<RefCell<…>>, shared across embedded languages)
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
bumpalo API only). The interner stays `Rc<RefCell<…>>` (created before the arena,
mutated during parse, shared across the Svelte→TS embedding boundary) — orthogonal
to `'arena`.

**Inline-by-value layout, deliberately not size-minimized.** A node holds its
children inline by value where they were owned inline before; only genuinely
recursive children sit behind `&'arena`. Variants are *not* boxed and inline
fields are *not* indirected to shrink node size — the formatter is traversal-bound,
and the extra pointer-chases that size-minimization adds on hot traversal paths
cost more than the cache-density they buy. (The arena allocation itself is the
win; the node *layout* favors traversal locality over byte size.)

The fat inline nodes carry no by-value-return penalty in the parser, either: each
node is built in the arena and threaded up the recursive descent **by reference**
(the expression parser's transient `ParsedExpr` wrapper holds an `&'arena
Expression`, not the node), so the recursion moves pointers regardless of node
size. The wrapper is kept register-returnable end to end — an 8-byte reference plus
two `u32` paren-bound positions (16 bytes), with the error boxed so the fallible
`Result<ParsedExpr, Box<ParseError>>` stays 16 bytes and returns in registers rather
than through an sret stack slot. The two concerns are decoupled — node *layout* is
tuned for the format traversal, while the parse-time recursion cost is paid in
pointer moves — so a fat inline variant is not a reason to box it.

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
early prototype conflated the two). Bump allocation for the nested model is now
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
  the per-language crate split is the multi-language question oxc never faces. Allocation has
  converged: like oxc, tsv now bump-allocates lifetime-threaded (`&'arena`) AST types — but
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

- **Scope/symbol resolution** — Syntax-only ASTs today. Meaningful linting requires name resolution. *(When: before linter.)*
- **Error recovery** — Fail-fast parsers block LSP/linter (need partial ASTs from broken code); also required for full CSS-spec compliance — CSS Syntax 3 §5.5 recovery (drop the bad rule, keep parsing), see conformance_svelte.md §CSS Parser Scope. *(When: for full CSS-spec compliance (CSS) / before LSP/linter.)*
- **Span encoding** — Byte offsets vs UTF-16 code units. LSP protocol uses UTF-16; mismatch = position bugs. *(When: before LSP.)*
- **Source maps** — Compiler must map output positions to input. How do spans survive transforms? *(When: before compiler.)*
- **Cancellation** — LSP operations must be cancellable mid-parse. Current parser has no cancellation points. *(When: before LSP.)*

## References

- [Flattening ASTs](https://www.cs.cornell.edu/~asampson/blog/flattening.html) — Adrian Sampson on arena patterns (context for Nested AST decision)
- [Zig Parser](https://mitchellh.com/zig/parser) — Mitchell Hashimoto on Zig's MultiArrayList AST
- [Prettier Technical Details](https://prettier.io/docs/en/technical-details) — comment attachment heuristics
- [OXC AST](https://github.com/oxc-project/oxc) — central shared AST + arena allocation in Rust (the contrasting design; see [Positioning vs. oxc and Biome](#positioning-vs-oxc-and-biome))
