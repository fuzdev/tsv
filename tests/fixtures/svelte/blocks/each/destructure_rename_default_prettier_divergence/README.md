# destructure_rename_default_prettier_divergence

**Prettier bug.** A renamed (non-shorthand) destructuring property that carries a
**default value** loses its source key in prettier-plugin-svelte's `{#each … as}`
binding printer: `{a: b = 1}` (read property `a`, bind to `b`) prints as `{ b = 1 }`
(read property `b`) — a **semantic change**, since it reads a different source
property. Only the defaulted property is affected; the plain-rename sibling `c: d`
is kept, and a nested pattern with a default drops its key the same way
(`{ a: { b } = c }` → `{ { b } = c }`). tsv keeps the key in every case.

tsv: `{#each items as { a: b = 1 }}`, `{#each items as { a: { b } = c }}` (key preserved)
Prettier: `{#each items as { b = 1 }}`, `{#each items as { { b } = c }}` (key dropped — wrong property)

## Reason

**Prettier bug.** prettier-plugin-svelte prints these binding patterns from raw
source and mishandles a non-shorthand property whose value is an `AssignmentPattern`,
emitting only the value and discarding the key — so its output reads a different
source property and is not semantically equivalent to its input. tsv preserves the
key. Plain renames without a default (`{a: b}`) are printed correctly by both. See
[conformance_prettier.md §Svelte: destructuring rename-with-default key drop](../../../../../../docs/conformance_prettier.md#svelte-destructuring-rename-with-default-key-drop).
