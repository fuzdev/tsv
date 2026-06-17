# destructure_with_index_key_prettier_divergence

An object destructuring pattern combined with an index and a key
(`{#each … as PATTERN, i (key)}`) hugs its braces (`bracketSpacing: false`). The
index and key clauses are unaffected.

tsv: `{#each items as {id, name}, i (id)}` (hugged)
Prettier: `{#each items as { id, name }, i (id)}` (spaces inside braces)

## Reason

**Design choice.** Same uniform `bracketSpacing: false` as
[destructure_object](../destructure_object_prettier_divergence/) — prettier-plugin-svelte
ignores the option for the `{#each … as}` binding pattern regardless of the
trailing index/key. See
[conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).
