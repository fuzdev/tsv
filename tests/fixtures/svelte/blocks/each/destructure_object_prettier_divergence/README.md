# destructure_object_prettier_divergence

An object destructuring **pattern** in a `{#each … as PATTERN}` binding hugs its
braces (`bracketSpacing: false`), like every other object literal/pattern tsv
emits — including `{@const {a, b} = expr}`, which already hugs.

tsv: `{#each items as {a, b}}` (hugged, consistent with `bracketSpacing: false`)
Prettier: `{#each items as { a, b }}` (spaces inside braces)

## Reason

**Design choice.** `bracketSpacing: false` is one of tsv's four fixed identity
settings, applied uniformly to objects everywhere. prettier-plugin-svelte ignores
`bracketSpacing` for the binding pattern of `{#each … as}`, `{#await … then}`,
`{:then}`, and `{:catch}` — it hardcodes the spaced `{ … }` form — even though it
honors the option for the same destructuring in `{@const}` (which routes through
Prettier's JS printer). tsv hugs in all of them, so a destructuring pattern reads
the same wherever it appears.

`unformatted_ours_spaces.svelte` (spaced/loose) normalizes to `input.svelte` under
tsv; Prettier normalizes it to `output_prettier.svelte` instead.

See [conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).

## Related

- [destructure_object_rest](../destructure_object_rest_prettier_divergence/) — rest element (`{id, ...rest}`)
- [destructure_object_default](../destructure_object_default_prettier_divergence/) — defaults and nested object values
- [destructure_with_index_key](../destructure_with_index_key_prettier_divergence/) — destructuring with index + key
- [await/destructure_default](../../await/destructure_default_prettier_divergence/) — `then`/`:then`/`:catch` contexts
