# element_body_leading_node_long_prettier_divergence

A breakable component that is **not the first node** of an `{#if}` body — preceded by
leading text, a comment, or a void element — still drops to its own line when the body
overflows, uniformly with every other body shape. These are the exact non-first-node
positions that made the earlier breakable-hug path over-wrap the head and become 2-pass
non-idempotent; the uniform drop handles them in one pass.

tsv drops the whole body (the leading node + component stay together on the dropped line,
which fits); prettier hugs the `}` and breaks the component internally
(`prettier_variant_hug.svelte`). prettier keeps tsv's dropped form stable, so the
divergence shows only when normalizing the compact one-liner
(`unformatted_ours_compact.svelte`) — tsv normalizes both it and prettier's hug back to
`input.svelte` in one pass.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and body
shapes (one-pass `conditional_group`, no breakable special-case) — including when the
breakable element is not the first body node. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../element_body_long_prettier_divergence/) — the same drop, breakable element as the sole body node
- [elements/inline_component_else_body_long](../../../elements/inline_component_else_body_long_prettier_divergence/) — the non-first-node case in an atomic `{:else}` branch
- [elements/block_body_drop_nested_siblings](../../../elements/block_body_drop_nested_siblings_prettier_divergence/) — breakable body after an inline sibling, realistic context
