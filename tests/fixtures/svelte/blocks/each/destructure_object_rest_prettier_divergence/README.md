# destructure_object_rest_prettier_divergence

An object destructuring pattern with a rest element in a `{#each … as PATTERN}`
binding hugs its braces (`bracketSpacing: false`).

tsv: `{#each items as {id, ...rest}}` (hugged)
Prettier: `{#each items as { id, ...rest }}` (spaces inside braces)

## Reason

**Design choice.** Same uniform `bracketSpacing: false` as
[destructure_object](../destructure_object_prettier_divergence/) — prettier-plugin-svelte
ignores the option for the `{#each … as}` binding pattern. See
[conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).
