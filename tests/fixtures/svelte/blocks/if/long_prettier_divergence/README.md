# long_prettier_divergence

A standalone `{#if …}` head that exceeds printWidth. tsv wraps the head, dangles its
closing `}` (with any `{:else if}` head, which wraps independently) on its own line at the tag's base indent, and expands the body +
`{/if}` onto their own lines. Prettier never width-wraps a block head — it keeps the
whole head inline past printWidth.

Boundary shapes covered: a head that fits (≤100) stays fully inline; a single call
whose args wrap hugs `)}`; a binary / multi-group member chain drops its `}` to base;
a 2-group member chain across the fit → middle-zone → wrap boundary; a 3+ group member
chain always wraps. An `{:else if}` head wraps and dangles independently (`{#if}` has
no `as`/`then` clause).

## Reason

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks) for the full head-wrap + `}` dangle +
clause-hug + body-expand + middle-zone model and why tsv diverges (consistent with its
JS `if (⏎…⏎) {` and broken-element `>`; block-body whitespace is render-non-significant).

## Related

- [if/long](../../if/long_prettier_divergence/) · [each/long](../../each/long_prettier_divergence/) · [key/long](../../key/long_prettier_divergence/) · [await/long](../../await/long_prettier_divergence/) — the same divergence per block head
- [if/inline_element_long](../inline_element_long_prettier_divergence/) — the same layout inside an inline element
