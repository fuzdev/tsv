# element_body_long_prettier_divergence

An `{#await … then}` / `{#await … catch}` shorthand inside an inline component
(`<Container>`) whose **breakable element body** exceeds printWidth. tsv **drops the
body to its own line** (uniform body-expand), while prettier **hugs** the `}` and
breaks the element internally (`prettier_variant_hug.svelte`).

Both forms are stable under their own formatter; tsv normalizes prettier's hug (and
the compact one-liner) back to `input.svelte`. The body-expand is render-safe inside
an inline element — block-body boundary whitespace is non-significant there (verified
against the Svelte compiler).

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and
body shapes, including inside inline elements/components — a one-pass `conditional_group`
with no breakable special-case. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../if/element_body_long_prettier_divergence/) — the same drop at block level
- [snippet/element_body_long](../../snippet/element_body_long_prettier_divergence/) — the same inside `<Container>` for `{#snippet}`
- [await/inline_element_long](../inline_element_long_prettier_divergence/) — head-wrap + body-expand for `{#await}` inside an inline element
