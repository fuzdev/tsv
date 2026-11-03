# type_modifier_reserved_word_local_svelte_divergence

A local export (no `from` clause) whose specifier names a reserved word, with and without the inline `type` modifier: `export { type default }`, `export { type if as b }`, `export { default as c }`. **tsc's parser accepts all three** (empty `parseDiagnostics`) and prettier formats them unchanged. **acorn-typescript rejects them** (`Unexpected keyword 'default'`), so `expected_svelte.json` pins that failure.

## Why tsv Differs

The specifier's names are `ModuleExportName`s — a string or **any** `IdentifierName`, reserved words included — which is why the same specifier is valid behind a `from` clause ([type_modifier_reserved_word_specifier](../type_modifier_reserved_word_specifier/)). Without one the local name is an `IdentifierReference`, and a reserved word there is the `ReferencedBindings` **Static Semantics early error** (ecma262 §sec-exports-static-semantics-early-errors), not a production. tsv defers early errors to diagnostics and takes the grammar alone, so it accepts the spelling; the `type` modifier does not change which name the grammar reads, so the modified and unmodified specifiers parse alike.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections) and [checklist_typescript.md §Strictness](../../../../../../docs/checklist_typescript.md).
