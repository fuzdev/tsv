# tsv_ts

> TypeScript parser and formatter for `tsv`

## Architecture Position

Depends only on `tsv_lang` (see [`tsv_lang/CLAUDE.md`](../tsv_lang/CLAUDE.md)). Consumed top-level by `tsv_cli`, `tsv_wasm`, `tsv_ffi`, and as an embedding dependency by `tsv_svelte` for `<script>` blocks and template expressions `{expr}`. Also handles `.svelte.ts` rune modules.

**Sources of truth**: acorn + acorn-typescript for parsing, Prettier for formatting. Read `../../../prettier/src/language-js/` before guessing layout behavior. See [../../CLAUDE.md §Canonical References](../../CLAUDE.md#canonical-references).

Standard `ast/` (internal + public + convert), `lexer/`, `parser/`, `printer/` layout — see [../../CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure). Module catalog lives in the root.

## Public API

**Standalone** (`.ts` files, top-level callers):

- `parse(source, arena: &'arena Bump) -> Result<Program<'arena>>` — the AST is bump-arena-allocated; the caller owns the `bumpalo::Bump` (caller-owns-arena, so the returned AST borrows it and the arena never escapes the call). See [docs/architecture.md §Nested AST](../../docs/architecture.md#nested-ast-bump-arena-not-flatindexed). `parse` is the `Goal::Module` form of `parse_with_goal(source, goal, arena)` — pass `Goal::Script` for a standalone strict script (`await` an ordinary identifier; `import`/`export`/`import.meta` errors). `Program.goal` drives the public `sourceType`. See **Strict mode only** below.
- `format(program, source) -> String` — the format entry point (output is identical for standalone `.ts` and Svelte-embedded TS). `format_in(program, source, &DocArena)` is the same but builds into a caller-provided doc arena, so multi-file drivers (CLI/FFI/NAPI) reuse one arena across files (`DocArena::reset()` between files); `format` is the fresh-arena wrapper
- `convert_ast_json_bytes(...) -> Vec<u8>` / `convert_ast_json_string(...) -> String` / `convert_ast_json(...) -> serde_json::Value` (all gated on `convert` feature). The bytes variant is the **sole emission path** — the compact-wire hot path (FFI/CLI non-pretty; the string variant is the same bytes plus one output UTF-8 validation, for `&str` boundaries — the WASM binding's `JSON.parse`, N-API strings). It is a **writer-mode conversion** (`ast/convert/write/`) that emits the wire JSON directly during a single walk of the *internal* AST — no typed public tree is ever materialized — and **fuses byte→UTF-16 offset translation into the walk**: the writer threads a `tsv_lang::LocationMapper` (tracker + `ByteToCharMap`) and emits final char-space `start`/`end`/`loc` directly, so no post-conversion translation walk runs (per-language pipeline shapes: [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)). The writer is a faithful emission of the acorn quirk catalog (field order, skip rules, scalar formatting; dynamic strings and non-integral floats delegate to `serde_json` so escaping matches exactly). `convert_ast_json` is a thin wrapper — `serde_json::from_slice(&convert_ast_json_bytes(...))` — for the `Value` consumers (the CLI's `--pretty`, the fixture gate), not an independent conversion. The oracle the writer is gated against is the canonical parser's `expected.json`, including the multibyte and `<script>`-comment fixtures that exercise the fused offset translation. `tsv_svelte` composes the embedded writers (`write_program_embedded`, `write_expression_embedded`, `write_pattern_embedded`, `write_variable_declaration_embedded`, and their `_with_comments` forms) plus `write_identifier_expression_with_character`; `WriterComments` carries an island's precomputed per-node comment assignments.

**Embedding** (used by `tsv_svelte` — shares interner, indent, comment buffers with the host document):

- `parse_with_interner`, `parse_expression_with_comments`, `parse_pattern_with_comments`, `parse_type_annotation_partial`, `parse_expression_partial_with_comments`
- `PrinterInputs { source, interner, comments, line_breaks }` — the per-document environment the format entry points share, so embedders don't re-thread the same values per call (the per-call `EmbedContext` and the expression/program stay separate args). `tsv_svelte` builds one via its `Printer::ts_inputs()` helper.
- `format_expression(expression, &PrinterInputs, EmbedContext) -> String` — renders an expression to a string
- `build_program_doc`, `build_expression_doc_with_comments`, `build_function_params_doc_with_comments`, `build_type_parameters_doc_with_comments` — emit a `DocId` into the caller's `DocArena` so Svelte can compose the doc tree before rendering; `build_expression_doc_with_comments(arena, expression, &PrinterInputs, &EmbedContext)` takes the shared bundle (`build_program_doc` derives it from the `Program`). `build_function_params_doc_with_comments(arena, params, params_start, trailing_comments_end, &PrinterInputs, &EmbedContext)` renders a parameter list `(…)` through the same comment-aware, `FunctionParameter`-context printer a real signature uses (single-pattern hug, nesting-depth expansion); `tsv_svelte` uses it for `{#snippet}` parameters. `build_type_parameters_doc_with_comments(arena, type_parameters, &PrinterInputs, &EmbedContext)` is its type-parameter counterpart — renders a `<…>` declaration through the same wrapping/comment-aware type-parameter printer (constraints, defaults, modifiers, width-based wrapping in its own group); `tsv_svelte` uses it for `{#snippet}` generics
- `should_inline_logical_expression`, `conditional_should_break_after_op` — Prettier assignment-layout predicates, exposed so embedders that mirror the assignment layout (Svelte's `{@const}`) apply the same break-after-operator rules instead of re-implementing them
- `ast::convert::translate_column` — `pub` so the delta-preserving byte→char column math exists once; `tsv_svelte`'s writer reuses it for the embedded `<script>` `Program`'s tag-line column positions. `ast::convert::name_cow` — the interned-name borrow-or-own helper (`tsv_svelte`'s writer uses it for element/attribute names)

## Distinctives

- **Context-free TypeScript formatting.** tsv emits identical output whether the TS is standalone (`.ts` / `.svelte.ts`) or embedded in a Svelte `<script>` / template — there is no per-context formatting knob. Notably, single-unconstrained arrow type params stay bare (`<T>`), unlike prettier-in-Svelte's forced `<T,>` (tsv has no JSX, and Svelte's parser accepts the bare form in every position); see [../../docs/conformance_prettier.md §TypeScript](../../docs/conformance_prettier.md).
- **`Schema`** in [`ast/convert/mod.rs`](src/ast/convert/mod.rs) selects the wire-JSON shape and is threaded through the writer. `convert_ast_json_bytes` always uses `Schema::Acorn`; `tsv_svelte` passes `Schema::SvelteScript` (omit `importKind`/`exportKind="value"`, always emit `attributes`) to `write_program_embedded` for a non-`lang="ts"` `<script>`. Tracked alongside the hand-maintained `tsv_ast.d.ts` — see [../tsv_wasm/CLAUDE.md §TS type maintenance](../tsv_wasm/CLAUDE.md#ts-type-maintenance).
- **`lexer/escapes.rs`** owns ECMAScript string/template escape decoding (acorn parity). `tsv_lang::escapes` only handles quote swapping at print time; full decoding lives here.
- **Strict mode only** — no `with`, no legacy octals, no duplicate parameters. Strict is orthogonal to the **goal** axis (`Goal::{Module, Script}`, default `Module`): both goals are strict; the goal only toggles `await`-as-identifier and the `import`/`export`/`import.meta` gates. See [../../CLAUDE.md §Strict Mode Only](../../CLAUDE.md#strict-mode-only).

## Reference

- [`docs/checklist_typescript.md`](../../docs/checklist_typescript.md) — feature coverage matrix
- [`docs/conformance_prettier.md`](../../docs/conformance_prettier.md), [`docs/conformance_svelte.md`](../../docs/conformance_svelte.md) — documented divergences
