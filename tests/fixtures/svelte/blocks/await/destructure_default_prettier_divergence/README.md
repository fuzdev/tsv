# destructure_default_prettier_divergence

Object destructuring patterns in `{#await … then}`, `{:then}`, and `{:catch}`
bindings hug their braces (`bracketSpacing: false`), the same as the `{#each … as}`
binding.

tsv: `{#await promise then {a, b = 1}}`, `{:then {a, b = "x"}}`, `{:catch {msg, code = 0}}` (hugged)
Prettier: `{#await promise then { a, b = 1 }}`, `{:then { a, b = "x" }}`, `{:catch { msg, code = 0 }}` (spaces)

## Reason

**Design choice.** Same uniform `bracketSpacing: false` as the each-block
[destructure_object](../../each/destructure_object_prettier_divergence/) — prettier-plugin-svelte
ignores the option for await/then/catch binding patterns too. See
[conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).
