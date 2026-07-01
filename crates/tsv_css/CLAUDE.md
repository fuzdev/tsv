# tsv_css

> CSS parser and formatter — drop-in for Svelte's `parseCss` (AST), near-Prettier formatting

## Architecture Position

Depends on `tsv_lang` for shared primitives (spans, doc builder, comments, embedding config). Consumed by `tsv_svelte` for `<style>` block embedding, and by `tsv_cli` / `tsv_wasm` / `tsv_ffi` for top-level CSS files.

**Sources of truth**: Svelte's `parseCss` defines the public AST shape; Prettier's `css` printer defines formatter output. Both are checked at fixture-validation time.

Standard `ast/lexer/parser/printer` crate layout — see [root CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure) and [`tsv_lang/CLAUDE.md`](../tsv_lang/CLAUDE.md) for the cross-cutting types (`DocArena`, `Span`, `EmbedContext`). Public/internal AST split follows the workspace convention — see [root CLAUDE.md §AST Architecture](../../CLAUDE.md#ast-architecture-internal-vs-public).

## Public API

**Standalone** (top-level CSS files):

- `parse(source, arena: &'arena Bump) -> Result<CssStyleSheet<'arena>>` — parse a full CSS file. The internal AST is bump-arena-allocated (caller-owns-`Bump`); `CssStyleSheet<'arena>` borrows from it. Matches `tsv_ts`/`tsv_svelte`'s signature for the shared `lang_bindings!` macro + CLI/FFI/WASM callers.
- `format(&stylesheet, source) -> String` — format with default config; `format_in(&stylesheet, source, &DocArena)` is the same into a caller-provided doc arena (multi-file drivers reuse one via `DocArena::reset()`; `format` is the fresh-arena wrapper)
- `convert_ast(&stylesheet, source) -> StyleSheet` — internal → public JSON-ready AST (gated on `convert` feature)
- `convert_ast_json(&stylesheet, source) -> serde_json::Value` — public AST with byte-to-char offset translation; matches Svelte's `parseCss()` JSON shape
- `convert_ast_json_string(&stylesheet, source) -> String` — the compact-wire hot path (FFI/WASM/CLI non-pretty): serializes the typed public AST (`ast::public`, built by `convert_stylesheet_file`) directly into a pre-sized buffer (`tsv_lang::estimated_json_capacity`), never materializing the intermediate `serde_json::Value`. Multibyte sources get the typed offset-translation walk (`translate_byte_to_char_offsets_typed` in `ast/convert/translate_typed.rs`), the typed mirror of the `Value` walk in `ast/convert/mod.rs` — the two must stay byte-identical (gated by the fixture suite's string-path identity check, the CSS typed-walk parity probe, and `corpus:compare:parse --multibyte-only`). Output is byte-identical to serializing `convert_ast_json`'s `Value`

**Embedding** (used by `tsv_svelte` for `<style>` blocks):

- `parse_embedded(source, base_offset, arena: &'arena Bump) -> Result<CssStyleSheet<'arena>>` — same parser, but span positions are shifted by `base_offset` so they index into the parent Svelte file; `arena` is the host document's `Bump`, so the embedded CSS AST shares it
- `format_embedded(&stylesheet, source, EmbedContext)` — formats with `EmbedContext::base_indent_offset` so wrapped lines respect outer Svelte indentation
- `ast::convert::translate_style_sheet_byte_to_char_offsets_typed` — byte→char offset translation over the typed `StyleSheet` envelope (the `<style>`-element counterpart of the standalone typed walk), called by `tsv_svelte`'s typed walk; covers the envelope's typed positions only — the caller owns its `serde_json::Value` islands (`attributes`, `content.comment`)

The two `StyleSheet` / `StyleContent` types (re-exported only with `convert`) are the public-AST envelopes `tsv_svelte` uses when embedding CSS in a `<style>` element's JSON. Distinct from `CssStyleSheet` (the internal AST root) and `CssNode` (the top-level statement enum).

## Distinctives

- **No canonical CSS parser of our own** — fixture validation parses through Svelte's `parseCss`, not a standalone CSS reference parser. Practical implication: AST _shape_ questions go to Svelte, not the CSS Syntax spec directly. _Validity_ (what to accept/reject) is a different axis: the north star is CSS-spec compliance, the near-term enforced goal is parity with `parseCss`, and where Svelte over-accepts invalid CSS the spec wins (tsv rejects). The parser currently **hard-fails** on the first invalid construct; spec-style error recovery is a committed post-v0.1 goal. See [`../../docs/conformance_svelte.md`](../../docs/conformance_svelte.md) §CSS Parser Scope & Error Model.
- **CSS escapes are positional Unicode** (`\XXXXXX` with an optional whitespace terminator), unlike `tsv_lang::escapes` (which handles JS-style quote swapping). They decode in two places by token kind: **string values** in `escapes.rs` (`decode_escape_sequences`, at parse time) and **identifiers** in `lexer/identifiers.rs` (`decode_unicode_escape`, at lex time). Identifier decoding is **lazy** — `read_identifier` allocates a decoded `String` only when a `\` escape is actually present; the no-escape common case allocates nothing and the identifier's text is recovered as a verbatim source slice (so the lexer's out-of-band `decoded` slot and the parser's `current_decoded` are `None` for plain identifiers). `tsv_ts` has its own `lexer/escapes.rs` for the same reason; `tsv_svelte` has none and delegates.
- **`convert` feature flag** (default-on) — gates the entire `ast::public` + `ast::convert` layer plus `serde`/`serde_json` deps. Disabled in the `@fuzdev/tsv_format_wasm` build, which only needs to format. See [`tsv_wasm/CLAUDE.md`](../tsv_wasm/CLAUDE.md).
- **Comments live on `CssStyleSheet`**, not on nodes. Value comments (e.g., `color: /* x */ red;`) are detected by scanning source text directly in `printer/mod.rs::has_value_comments_in_decl`, since they're not stored as `Comment` entries.
- **Printer is organized by CSS spec hierarchy** (`rules.rs` → `declarations.rs` → `values.rs`, plus `atrules.rs` and `selectors.rs`), not by node kind. `selectors.rs` is shared between rules and at-rules, and rule and at-rule block bodies iterate their children through one shared `print_css_block_children` routine in `mod.rs` (blank-line preservation, inline trailing comments, format-ignore — one place, no drift); the `value_normalization/` module normalizes value text to prettier's form (numbers, hex colors, whitespace). Its number/dimension normalizers return `Cow`, borrowing an already-canonical value (`10px`, `0.5rem`) unchanged, so `build_dimension_doc` emits it as a zero-allocation verbatim `source_span` — which is why the CSS printer renders through a source-aware `TextResolver` (a bare source-only one; unlike `tsv_ts`/`tsv_svelte` it carries no interner, since it never emits `DocText::Symbol`).

## Checklist

See [`docs/checklist_css.md`](../../docs/checklist_css.md) for the language feature checklist.
