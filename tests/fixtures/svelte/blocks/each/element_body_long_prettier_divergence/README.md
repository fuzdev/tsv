# element_body_long_prettier_divergence

An `{#each}` whose head fits but whose **breakable element body** (a component with
attributes) exceeds printWidth when hugged on the head line. tsv **drops the body to its
own line** — uniformly with every other block body — at the 100/101 boundary:

1. **inline form == 100** — the construct stays fully inline (both formatters).
2. **inline form == 101** — tsv drops the body; prettier hugs the `}` and breaks the
   component internally (`prettier_variant_hug.svelte`).

prettier keeps tsv's dropped form stable, so the divergence shows only when normalizing
the compact one-liner (`unformatted_ours_compact.svelte`) — tsv normalizes both it and
prettier's hug back to `input.svelte` in one pass.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and body
shapes (one-pass `conditional_group`, no breakable special-case). See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [each/long](../long_prettier_divergence/) — the head-wrap + dangle + body-expand divergence, standalone
- [if/element_body_long](../../if/element_body_long_prettier_divergence/) — the same drop for `{#if}`
- [if/element_body_deep_nested](../../if/element_body_deep_nested_prettier_divergence/) — the same drop, deeply nested (dropped element re-wraps)
