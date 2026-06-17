# destructure_rename_default_prettier_divergence

**Prettier bug.** The same renamed-property key drop as the each-block
[destructure_rename_default](../../each/destructure_rename_default_prettier_divergence/),
across `{#await … then}`, `{:then}`, and `{:catch}` bindings: a non-shorthand
property with a default (`{a: b = 1}`, read property `a`) prints as `{ b = 1 }`
(read property `b`) — a semantic change. tsv preserves the key in every branch.

tsv: `{:then {a: b = 1}}`, `{:catch {a: b = 1}}` (key preserved)
Prettier: `{:then { b = 1 }}`, `{:catch { b = 1 }}` (key dropped — wrong property)

## Reason

**Prettier bug.** prettier-plugin-svelte discards the key of a non-shorthand
destructuring property whose value is an `AssignmentPattern`, in every await binding
position — so the output reads a different source property and is not semantically
equivalent to its input. tsv preserves the key. See
[conformance_prettier.md §Svelte: destructuring rename-with-default key drop](../../../../../../docs/conformance_prettier.md#svelte-destructuring-rename-with-default-key-drop).
