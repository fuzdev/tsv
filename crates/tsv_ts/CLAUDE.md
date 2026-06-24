# tsv_ts

> TypeScript parser and formatter for `tsv`

## Architecture Position

Depends only on `tsv_lang` (see [`tsv_lang/CLAUDE.md`](../tsv_lang/CLAUDE.md)). Consumed top-level by `tsv_cli`, `tsv_wasm`, `tsv_ffi`, and as an embedding dependency by `tsv_svelte` for `<script>` blocks and template expressions `{expr}`. Also handles `.svelte.ts` rune modules.

**Sources of truth**: acorn + acorn-typescript for parsing, Prettier for formatting. Read `../../../prettier/src/language-js/` before guessing layout behavior. See [../../CLAUDE.md §Canonical References](../../CLAUDE.md#canonical-references).

Standard `ast/` (internal + public + convert), `lexer/`, `parser/`, `printer/` layout — see [../../CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure). Module catalog lives in the root.

## Public API

**Standalone** (`.ts` files, top-level callers):

- `parse(source) -> Result<Program>`
- `format(program, source) -> String` — the single format entry point (output is identical for standalone `.ts` and Svelte-embedded TS)
- `convert_ast(program, source) -> public::Program` / `convert_ast_json(...) -> serde_json::Value` / `convert_ast_json_string(...) -> String` (all gated on `convert` feature). The string variant is the compact-wire hot path (FFI/WASM/CLI non-pretty): it serializes the typed public AST directly when eligible, never materializing the intermediate `Value` (per-language eligibility matrix: [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)). The typed offset-translation walk (`ast/convert/translate_typed.rs`) is the typed mirror of the `Value` walk in `ast/convert/mod.rs` — the two must stay byte-identical, gated by the fixture suite's string-path identity check and typed-walk parity probes (synthesized multibyte variants plus extracted `<script>` contents, so every fixture's AST shapes are covered) and `json_profile`'s corpus comparison. Output is byte-identical to serializing `convert_ast_json`'s `Value`.

**Embedding** (used by `tsv_svelte` — shares interner, indent, comment buffers with the host document):

- `parse_with_interner`, `parse_expression_with_comments`, `parse_pattern_with_comments`, `parse_type_annotation_partial`, `parse_expression_partial_with_comments`
- `PrinterInputs { source, interner, comments, line_breaks }` — the per-document environment the format entry points share, so embedders don't re-thread the same values per call (the per-call `EmbedContext` and the expression/program stay separate args). `tsv_svelte` builds one via its `Printer::ts_inputs()` helper.
- `format_expression(expression, &PrinterInputs, EmbedContext) -> String` — renders an expression to a string
- `build_program_doc`, `build_expression_doc_with_comments`, `build_function_params_doc_with_comments`, `build_type_parameters_doc_with_comments` — emit a `DocId` into the caller's `DocArena` so Svelte can compose the doc tree before rendering; `build_expression_doc_with_comments(arena, expression, &PrinterInputs, &EmbedContext)` takes the shared bundle (`build_program_doc` derives it from the `Program`). `build_function_params_doc_with_comments(arena, params, params_start, trailing_comments_end, &PrinterInputs, &EmbedContext)` renders a parameter list `(…)` through the same comment-aware, `FunctionParameter`-context printer a real signature uses (single-pattern hug, nesting-depth expansion); `tsv_svelte` uses it for `{#snippet}` parameters. `build_type_parameters_doc_with_comments(arena, type_parameters, &PrinterInputs, &EmbedContext)` is its type-parameter counterpart — renders a `<…>` declaration through the same wrapping/comment-aware type-parameter printer (constraints, defaults, modifiers, width-based wrapping in its own group); `tsv_svelte` uses it for `{#snippet}` generics
- `should_inline_logical_expression`, `conditional_should_break_after_op` — Prettier assignment-layout predicates, exposed so embedders that mirror the assignment layout (Svelte's `{@const}`) apply the same break-after-operator rules instead of re-implementing them

## Distinctives

- **Context-free TypeScript formatting.** tsv emits identical output whether the TS is standalone (`.ts` / `.svelte.ts`) or embedded in a Svelte `<script>` / template — there is no per-context formatting knob. Notably, single-unconstrained arrow type params stay bare (`<T>`), unlike prettier-in-Svelte's forced `<T,>` (tsv has no JSX, and Svelte's parser accepts the bare form in every position); see [../../docs/conformance_prettier.md §TypeScript](../../docs/conformance_prettier.md).
- **`Schema`** in [`ast/convert/mod.rs`](src/ast/convert/mod.rs) selects the public-AST shape. `convert_ast()` always uses `Schema::Acorn`; callers needing Svelte's non-`lang="ts"` `<script>` shape (omit `importKind`/`exportKind="value"`, always emit `attributes`) invoke `ast::convert::convert_program(..., Schema::SvelteScript)` directly. Tracked alongside the hand-maintained `tsv_ast.d.ts` — see [../tsv_wasm/CLAUDE.md §TS type maintenance](../tsv_wasm/CLAUDE.md#ts-type-maintenance).
- **`lexer/escapes.rs`** owns ECMAScript string/template escape decoding (acorn parity). `tsv_lang::escapes` only handles quote swapping at print time; full decoding lives here.
- **Strict mode only** — no `with`, no legacy octals, no duplicate parameters. See [../../CLAUDE.md §Strict Mode Only](../../CLAUDE.md#strict-mode-only).

## Reference

- [`docs/checklist_typescript.md`](../../docs/checklist_typescript.md) — feature coverage matrix
- [`docs/conformance_prettier.md`](../../docs/conformance_prettier.md), [`docs/conformance_svelte.md`](../../docs/conformance_svelte.md) — documented divergences
