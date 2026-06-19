# inline_element_long_prettier_divergence

A `{#if …}` whose head exceeds printWidth, placed inside an **inline** element
(`<span>`). tsv applies the same layout as at block level — the head wraps, its `}`
dangles, and the consequent + `{:else}` / `{:else if}` branches + `{/if}` drop to their own lines — while the element hugs the outer boundary
(`<span⏎\t>…</span⏎>`). Prettier keeps the whole construct inline past printWidth,
wrapping only the enclosing element.

This is safe because block-body boundary whitespace is render-non-significant inside
an inline element (verified against the Svelte compiler); only `<pre>` /
`white-space:pre` gates the expand off.

## Reason

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks) for the full head-wrap + `}` dangle +
body-expand model (uniform at block level and inside inline elements) and why tsv
diverges.

## Related

- [if/long](../long_prettier_divergence/) — the same divergence standalone (block level)
- [snippet/inline_in_element](../../snippet/inline_in_element_prettier_divergence/) — a `{#snippet}` inside an inline element
