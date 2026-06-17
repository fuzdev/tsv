# destructure_object_default_prettier_divergence

Object destructuring patterns with default values in a `{#each … as PATTERN}`
binding hug their braces (`bracketSpacing: false`), including a nested object
**value** in a default (`c = {x: 1}`).

tsv: `{#each items as {a, b = 1}}`, `{#each items as {a = 1, b = [1, 2], c = {x: 1}}}` (hugged)
Prettier: `{#each items as { a, b = 1 }}`, `… c = { x: 1 } }` (spaces inside braces)

## Reason

**Design choice.** Same uniform `bracketSpacing: false` as
[destructure_object](../destructure_object_prettier_divergence/) — prettier-plugin-svelte
ignores the option for the `{#each … as}` binding pattern, including object values
nested inside defaults. Array defaults (`[1, 2]`) and the string default keep the
form both formatters already agree on. See
[conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).
