# long_prettier_divergence

A standalone `{#each …}` head that exceeds printWidth. tsv wraps the head, dangles its
closing `}` (with the ` as item` clause) on its own line at the tag's base indent, and
expands the body + any `{:else}` fallback + `{/each}` onto their own lines. Prettier
never width-wraps a block head — it keeps the whole head inline past printWidth.

Boundary shapes covered: a head that fits (≤100) stays fully inline; a single call whose
args wrap hugs `)` then the ` as item` clause + `}`; a binary / multi-group member chain
drops its clause + `}` to base; a 2-group member chain across the fit → middle-zone →
wrap boundary; a 3+ group member chain always wraps. A `{:else}` fallback expands with
the body when the construct overflows (head fits alone).

## Reason

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks) for the full head-wrap + `}` dangle +
clause-hug + body-expand + middle-zone model and why tsv diverges (consistent with its
JS `if (⏎…⏎) {` and broken-element `>`; block-body whitespace is render-non-significant).

## Related

- [if/long](../../if/long_prettier_divergence/) · [each/long](../../each/long_prettier_divergence/) · [key/long](../../key/long_prettier_divergence/) · [await/long](../../await/long_prettier_divergence/) — the same divergence per block head
- [each/inline_element_long](../inline_element_long_prettier_divergence/) — the same layout inside an inline element
