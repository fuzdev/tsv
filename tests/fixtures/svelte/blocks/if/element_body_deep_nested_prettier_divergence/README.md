# element_body_deep_nested_prettier_divergence

A `{#if}` whose component body drops to its own line, **deeply nested** (5 `<div>`
levels) so the hugged head line overflows. The depth is what makes this distinct from
the shallow [if/element_body_long](../element_body_long_prettier_divergence/): the
dropped component lands at the body indent (6 tabs), where the **100/101 boundary plays
out on the component itself**:

1. **== 100** — the dropped component fits on one line.
2. **== 101** (one char longer) — the dropped component wraps its own attributes.

In both cases tsv **drops the body** uniformly; prettier instead **hugs** the `}` and
breaks the component internally (`prettier_variant_hug.svelte`). prettier keeps tsv's
dropped form stable, so the divergence shows only when normalizing the compact one-liner
(`unformatted_ours_compact.svelte`) — tsv normalizes both it and prettier's hug back to
`input.svelte` in one pass.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and body
shapes (one-pass `conditional_group`, no breakable special-case) — at any nesting depth,
and even when the dropped element itself still overflows and re-wraps. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../element_body_long_prettier_divergence/) — the same drop, shallow (dropped element fits)
- [elements/inline_if_sibling_fill_long](../../../elements/inline_if_sibling_fill_long_prettier_divergence/) — a `{#if}` body drop next to an inline sibling, also nested
