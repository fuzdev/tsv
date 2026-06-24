# inline_element_long_prettier_divergence

An `{#await …}` whose head exceeds printWidth, placed inside an **inline** element
(`<span>`). tsv applies the same layout as at block level — the head wraps, its `}`
dangles (with the ` then r` clause), and the body + `{/await}` drop to their own lines —
while the element hugs the outer boundary (`<span⏎\t>…</span⏎>`). Prettier keeps the
whole construct inline past printWidth, wrapping only the enclosing element.

This is safe because block-body boundary whitespace is render-non-significant inside
an inline element (verified against the Svelte compiler); only `<pre>` /
`white-space:pre` gates the expand off.

## Reason

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks) for the full head-wrap + `}` dangle +
body-expand model (uniform at block level and inside inline elements) and why tsv
diverges.

## Related

- [await/long](../long_prettier_divergence/) — the same divergence standalone (block level)
- [each/inline_element_long](../../each/inline_element_long_prettier_divergence/) — a `{#each}` inside an inline element
