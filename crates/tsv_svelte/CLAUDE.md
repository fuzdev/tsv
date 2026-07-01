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
- `convert_ast(&Root, source) -> public::Root`, `convert_ast_json(...) -> serde_json::Value`, and `convert_ast_json_string(...) -> String` — public JSON AST (byte→char position translation + template-expression comment attachment happen here, gated on `feature = "convert"`). The string variant is the compact-wire hot path (FFI/WASM/CLI non-pretty): it serializes the typed public AST directly on every input (per-language pipeline shapes: [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)). Template-expression comments go through the island-scoped attach pass first (`ast/convert/attach_typed.rs` — only the expressions a comment lands on convert to `Value` islands via `public::ExpressionIsland`, mirroring the `Value` dispatcher in `ast/convert/comment_attachment.rs` that `convert_ast_json` still runs whole-document), then multibyte sources get the hybrid typed offset-translation walk (`ast/convert/translate_typed.rs` — typed Svelte nodes + `name_loc`, embedded `tsv_ts`/`tsv_css` subtrees delegated to those crates' typed walks, `serde_json::Value` islands to the `Value` walk). `Script.content` follows the same island pattern at conversion time (`public::ProgramIsland`): a `lang="ts"` script with nothing to inject (no script comments, no preceding HTML comment) stays a typed `Program`, skipping the per-script JSON roundtrip. Attribute and style-directive values are fully typed (`public::AttributeValueField` — the wire's `true | object | array` shapes as an untagged enum, no per-attribute `Value` building). Output is byte-identical to the `Value` path, gated by the fixture suite's string-path identity check and its per-fixture synthesized-multibyte and synthesized-template-comment `.svelte` parity probes.
- `script_content_spans(&Root) -> Vec<(u32, u32)>` — byte spans of the instance/module `<script>` contents (gated on `feature = "convert"`). Feeds the attach passes' template-comment filter; also used by tooling that extracts script contents as standalone TS (`tsv_debug`'s typed-walk parity probes, `json_profile`)

## Distinctives

What separates this crate from `tsv_ts` / `tsv_css`:

- **Embeds two other languages.** The printer delegates `<script>` and every template `{expr}` to `tsv_ts` doc builders (e.g. `tsv_ts::build_program_doc`) and `<style>` to `tsv_css::format_embedded`, passing an `EmbedContext` with the current indent state and `LayoutMode::Embedded` so binary expressions use continuation indent. See `printer/script_style.rs` and `printer/nodes/tags_doc.rs`.
- **Element classification adapter.** `printer/classification/element.rs` resolves interned tag-name symbols and dispatches to the pure `tsv_html` functions. The Svelte-specific overlay (Components are inline; non-empty `<script>`/`<style>` are block) lives here, not in `tsv_html`.
- **No `escapes/` module.** String/template-literal escapes are handled inside the embedded TS/CSS — Svelte itself has no escape rules at the template level.
- **Lazy entity decoding on `Text`.** The internal `Text` node stores `raw` plus a parse-time `TextDecoding` context (`Fragment` / `AttributeValue` / `Raw`); the decoded form comes from `Text::data() -> Cow<str>`, which borrows `raw` when no `&` is present (decode is identity) or in `Raw` context. Contexts mirror the canonical parser: fragment text decodes with text-content rules, quoted and unquoted attribute values with attribute rules, raw-content element text not at all. The printer reads `raw`; `data()` serves boundary and cold paths (public-AST conversion, `lang`/`context` attribute checks, root-span whitespace trimming).
- **Dual schema for `<script>` conversion.** `ast/convert/special.rs` picks `tsv_ts::ast::convert::Schema::Acorn` for `lang="ts"` and `Schema::SvelteScript` otherwise. The latter omits `importKind` / `exportKind = "value"` and always emits `attributes` on import/export declarations to match Svelte's parser output.

## See Also

- [`docs/checklist_svelte.md`](../../docs/checklist_svelte.md) — implementation checklist
- [`docs/conformance_svelte.md`](../../docs/conformance_svelte.md) — documented divergences from Svelte's parser
