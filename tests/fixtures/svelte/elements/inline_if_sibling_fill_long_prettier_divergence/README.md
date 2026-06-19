# inline_if_sibling_fill_long_prettier_divergence

A `{#if}` preceded by an inline `<span>` sibling, whose own body is a breakable
`<span>`, nested in a `<small>` (fill context). When the combined content exceeds
printWidth, tsv keeps the `</span>{#if …}` sibling boundary hugged (injecting
whitespace there would be render-significant) and **drops the `{#if}` body to its own
line**. Prettier instead hugs the `}` and breaks the inner `<span>` internally
(`prettier_variant_hug.svelte`).

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and
body shapes — including a block sitting after an inline sibling, where only the body
drops and the sibling boundary stays hugged. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop, standalone
- [components/attrs_nested_long](../../components/attrs_nested_long_prettier_divergence/) — a `{#if}` body drop at deep nesting
