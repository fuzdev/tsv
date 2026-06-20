# destructure_computed_key_prettier_divergence

A **computed key** (`[expr]`) in an object destructuring pattern keeps its `[ ]`
brackets in a `{#each … as PATTERN}` binding, exactly like Prettier — the only
difference is the object braces, which tsv hugs (`bracketSpacing: false`) where
prettier-plugin-svelte spaces them.

tsv: `{#each objs as {[id]: value}}` (hugged braces, brackets preserved)
Prettier: `{#each objs as { [id]: value }}` (spaces inside braces, brackets preserved)

## Reason

**Design choice (braces only).** The bracket preservation itself is a plain match —
both formatters keep the `[ ]` around a computed key, so the key reads the same
property at runtime. The lone divergence is the brace spacing: `bracketSpacing:
false` is one of tsv's four fixed identity settings, applied uniformly to objects
everywhere, while prettier-plugin-svelte hardcodes the spaced `{ … }` form for the
binding patterns of `{#each … as}`, `{#await … then}`, `{:then}`, and `{:catch}`.
The brackets, the template-literal key, the nested object-pattern value, and the
rest sibling are all preserved identically by both formatters.

`unformatted_ours_spaces.svelte` (spaced/loose) normalizes to `input.svelte` under
tsv; Prettier normalizes it to `output_prettier.svelte` instead.

See [conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).

## Related

- [destructure_object](../destructure_object_prettier_divergence/) — basic object pattern (`{a, b}`)
- [destructure_object_rest](../destructure_object_rest_prettier_divergence/) — rest element (`{id, ...rest}`)
- [await/destructure_computed_key](../../await/destructure_computed_key_prettier_divergence/) — `then`/`:then`/`:catch` contexts
