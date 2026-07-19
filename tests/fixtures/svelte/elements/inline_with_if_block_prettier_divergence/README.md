# inline_with_if_block_prettier_divergence

tsv lays out this inline element's wrapping content **block-style** — both tags stay intact and the
content moves to its own indented line(s), collapsing to `<tag>content</tag>` when it fits. Prettier
instead dangles the tag delimiters (`<tag⏎\t>content</tag⏎>`). Content-boundary whitespace is
render-free under Svelte 5 (start/end-of-content whitespace is trimmed at compile), so the injected
block-style boundaries are render-equivalent.

The `unformatted_ours_*` variants are compact authorings that tsv normalizes to the block-style
input; prettier does not (it dangles), so they carry the divergence.
`unformatted_ours_spaces.svelte` authors the if-body's section boundaries with spaces
(`{#if cond} text {/if}`) — render-free, so tsv glues them and converges on the same input as the
compact authoring. Prettier instead expands the spaced if inside its dangle
(`divergent_variant_expanded.svelte`, stable as such); tsv rewrites that to a third stable form,
pinned dual-stable in `variant_expanded.svelte` (block-style span, if kept multiline).

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
