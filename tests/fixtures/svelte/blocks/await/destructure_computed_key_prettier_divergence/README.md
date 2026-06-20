# destructure_computed_key_prettier_divergence

A **computed key** (`[expr]`) in an object destructuring pattern keeps its `[ ]`
brackets in the `{#await … then}`, `{:then}`, and `{:catch}` binding positions,
exactly like Prettier — the only difference is the object braces, which tsv hugs
(`bracketSpacing: false`) where prettier-plugin-svelte spaces them.

tsv: `{:catch {[k]: error}}` (hugged braces, brackets preserved)
Prettier: `{:catch { [k]: error }}` (spaces inside braces, brackets preserved)

## Reason

**Design choice (braces only).** The bracket preservation itself is a plain match —
both formatters keep the `[ ]` around a computed key, so the key reads the same
property at runtime. The lone divergence is the brace spacing: `bracketSpacing:
false` is one of tsv's four fixed identity settings, applied uniformly to objects
everywhere, while prettier-plugin-svelte hardcodes the spaced `{ … }` form for the
binding patterns of `{#each … as}`, `{#await … then}`, `{:then}`, and `{:catch}`.
The computed brackets are preserved identically by both formatters across all three
await binding positions.

`unformatted_ours_compact.svelte` (compact/loose) normalizes to `input.svelte`
under tsv; Prettier normalizes it to `output_prettier.svelte` instead.

See [conformance_prettier.md §Svelte: destructuring bracket spacing](../../../../../../docs/conformance_prettier.md#svelte-destructuring-bracket-spacing).

## Related

- [each/destructure_computed_key](../../each/destructure_computed_key_prettier_divergence/) — `{#each … as}` context
- [destructure_default](../destructure_default_prettier_divergence/) — defaults across `then`/`:then`/`:catch`
