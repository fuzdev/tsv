# tsv_svelte

> Svelte component parser + formatter — drop-in replacement for Svelte's compiler frontend, with a near-Prettier formatter tracking Prettier's `svelte` plugin.

## Architecture Position

Depends on:

- [`tsv_lang`](../tsv_lang/CLAUDE.md) — `Span`, `ParseError`, doc builder, `EmbedContext` / `LayoutMode`
- `tsv_ts` — embedded TypeScript: `<script>` bodies and template `{expr}` slots
- `tsv_css` — embedded CSS: `<style>` blocks
- `tsv_html` — element classification (block / inline / void), whitespace rules

Sources of truth: Svelte's compiler for AST shape, Prettier (`svelte` plugin) for formatting. Scope is `.svelte` files only — `.svelte.ts` rune modules are pure TypeScript and go through `tsv_ts` (this crate does not handle them).

See [../../CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure) for the standard crate layout (`ast/` / `lexer/` / `parser/` / `printer/`) and project-wide conventions.

## Public API

`src/lib.rs` exports free functions matching the tsv pattern:

- `parse(source, arena: &'arena Bump) -> Result<Root<'arena>>` — internal AST, bump-arena-allocated (caller owns the `Bump`). The Svelte parser creates the one arena per document and shares it with every embedded sub-AST — `tsv_ts` (`<script>` / `{expr}`) and `tsv_css` (`<style>`) — so the whole component is one bump-allocated graph. See [docs/architecture.md §Nested AST](../../docs/architecture.md#nested-ast-bump-arena-not-flatindexed).
- `format(&Root, source) -> String` — round-trips through the doc builder; `format_in(&Root, source, &DocArena)` is the same into a caller-provided doc arena (multi-file drivers reuse one via `DocArena::reset()`; embedded `<style>` still uses its own per-block arena)
- `convert_ast_json_bytes(...) -> Vec<u8>`, `convert_ast_json_string(...) -> String`, and `convert_ast_json(...) -> serde_json::Value` — the public JSON AST (gated on `feature = "convert"`). The bytes variant is the **sole emission path** (FFI/CLI non-pretty; the string variant adds one output UTF-8 validation for `&str` boundaries — WASM `JSON.parse`, N-API): the writer (`ast/convert/write.rs`) walks the *internal* Svelte AST once and emits the wire JSON directly, never materializing a typed public tree, fusing byte→UTF-16 offset translation into the walk (per-language pipeline shapes: [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)). The Svelte spine (elements, blocks, tags, directives, attributes, `name_loc`) emits fused; embedded `<script>`/`{expr}` route through `tsv_ts`'s embedded writers and `<style>` children through `tsv_css`'s `write_css_node`. Template-expression comments (and `<script>` comments) fuse via an island-scoped attach: each comment-bearing island's wire node tree is recorded during a byte-space skeleton emit (`tsv_ts`'s `SkeletonRecorder`, driven by `ast/convert/special.rs`'s `build_*_writer_comments` — no re-parse of the emitted bytes), the acorn attach machinery in `ast/convert/comment_attachment.rs` walks the recorded tree, and the assignments fold into a span-keyed `WriterComments` the fused writer consults at each node's close (so `leadingComments`/`trailingComments` serialize in place). `convert_ast_json` is a thin wrapper (`serde_json::from_slice(&convert_ast_json_bytes(...))`) for the `Value` consumers (the CLI's `--pretty`, the fixture gate); not an independent conversion. Gated against the canonical Svelte parser's `expected.json`, including the multibyte and template-comment fixtures that exercise the fused offset translation and island-scoped attach.
- `convert_ast_json_bytes_no_locations(...)` / `convert_ast_json_string_no_locations(...)` — the opt-in **span-only** variant: drops *every* line/column object from the Svelte wire — the acorn `loc` on `<script>`/`{expr}` nodes (via `emit_loc` threaded into the embedded `tsv_ts` writers), the root-comment `loc`, **and the element/attribute/directive `name_loc`**. Only `start`/`end` offsets remain; because that removes all line/column emission, the LF line table is never queried. A name's exact span reconstructs as `node.start + a fixed per-node-type prefix` (`<`→+1, plain attribute→+0, directive→its keyword+`:` length, shorthand→+1) plus source. Mirrors acorn's `locations: false`; a distinct narrower product, not a change to the drop-in wire (which is byte-identical). Invariant: `tests/no_locations.rs`.
- `script_content_spans(&Root) -> Vec<(u32, u32)>` — byte spans of the instance/module `<script>` contents (gated on `feature = "convert"`). The writer uses it to partition comments into template-expression comments (outside the spans) vs `<script>` comments

## Distinctives

What separates this crate from `tsv_ts` / `tsv_css`:

- **Embeds two other languages.** The printer delegates `<script>` and every template `{expr}` to `tsv_ts` doc builders (e.g. `tsv_ts::build_program_doc`) and `<style>` to `tsv_css::format_embedded`, passing an `EmbedContext` with the current indent state and `LayoutMode::Embedded` so binary expressions use continuation indent. See `printer/script_style.rs` and `printer/nodes/tags_doc.rs`.
- **Element classification adapter.** `printer/classification/element.rs` resolves interned tag-name symbols and dispatches to the pure `tsv_html` functions. The Svelte-specific overlay (Components are inline; non-empty `<script>`/`<style>` are block) lives here, not in `tsv_html`.
- **No `escapes/` module.** String/template-literal escapes are handled inside the embedded TS/CSS — Svelte itself has no escape rules at the template level.
- **Lazy entity decoding on `Text`.** The internal `Text` node stores `raw` plus a parse-time `TextDecoding` context (`Fragment` / `AttributeValue` / `Raw`); the decoded form comes from `Text::data() -> Cow<str>`, which borrows `raw` when no `&` is present (decode is identity) or in `Raw` context. Contexts mirror the canonical parser: fragment text decodes with text-content rules, quoted and unquoted attribute values with attribute rules, raw-content element text not at all. The printer reads `raw`; `data()` serves boundary and cold paths (public-AST conversion, `lang`/`context` attribute checks, root-span whitespace trimming).
- **Dual schema for `<script>` conversion, chosen component-globally.** The writer picks `tsv_ts::ast::convert::Schema::Acorn` when the component parses as TypeScript and `Schema::SvelteScript` otherwise; the latter omits `importKind` / `exportKind = "value"` and always emits `attributes` on import/export declarations to match Svelte's parser output. The TS decision is **component-global** (`ast/convert/special.rs`'s `component_is_typescript`, mirroring Svelte's single `this.ts` flag): the first `<script>` in source order carrying a `lang` attribute decides it for *every* script, so a plain `<script>` beside a `lang="ts"` sibling still emits the acorn shape.

## See Also

- [`docs/checklist_svelte.md`](../../docs/checklist_svelte.md) — implementation checklist
- [`docs/conformance_svelte.md`](../../docs/conformance_svelte.md) — documented divergences from Svelte's parser
