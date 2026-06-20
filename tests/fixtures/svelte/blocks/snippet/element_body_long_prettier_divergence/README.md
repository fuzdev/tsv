# element_body_long_prettier_divergence

A `{#snippet}` inside an inline component (`<Container>`) whose **breakable element
body** exceeds printWidth. tsv **drops the body to its own line** (uniform body-expand,
including paramless snippets), while prettier **hugs** the `}` and breaks the element
internally (`prettier_variant_hug.svelte`).

Both forms are stable under their own formatter; tsv normalizes prettier's hug (and the
compact/space-padded variants) back to `input.svelte`.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and body
shapes — `{#snippet}` is not special-cased, so its body drops like every other block's
(a one-pass `conditional_group`, no breakable hug path). See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [snippet/inline_element_long](../inline_element_long_prettier_divergence/) — `{#snippet}` params-inline + body-expand inside an inline element
- [if/element_body_long](../../if/element_body_long_prettier_divergence/) — the same drop at block level
- [await/element_body_long](../../await/element_body_long_prettier_divergence/) — the same inside `<Container>` for `{#await}`
