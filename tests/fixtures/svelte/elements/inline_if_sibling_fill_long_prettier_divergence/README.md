# inline_if_sibling_fill_long_prettier_divergence

A `{#if}` preceded by an inline `<span>` sibling, whose own body is a breakable
`<span>`, nested in a `<small>` (fill context). When the combined content exceeds
printWidth, tsv **dangles the `</span>` closing `>`** onto its own line so the `{#if}`
head starts fresh, and **drops the `{#if}` body to its own line**. The `>` moves only
*inside* the closing tag (`</span⏎>`), injecting no whitespace between `</span>` and
`{#if}`, so it is render-safe. Prettier keeps `</span>{#if}` hugged and the body on its
own line (`output_prettier.svelte`); on the compact one-liner prettier instead hugs the
`}` and breaks the inner `<span>` internally (`prettier_variant_inline.svelte`).

## Reason

tsv dangles the `>` token immediately preceding an expanding block's `{#…}` (a
preceding inline sibling's closing `>` here) and expands the body uniformly across all
block heads and body shapes. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop, standalone
- [if/element_body_deep_nested](../../blocks/if/element_body_deep_nested_prettier_divergence/) — a `{#if}` body drop at deep nesting
