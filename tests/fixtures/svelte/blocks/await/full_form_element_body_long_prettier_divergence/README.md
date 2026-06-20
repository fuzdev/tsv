# full_form_element_body_long_prettier_divergence

The full `{#await}` / `{:then}` / `{:catch}` form (not the `then`/`catch` shorthand) where
**each section's body is a breakable component** that overflows: tsv drops every section
body to its own line — uniformly, the await analog of the `{:else}` body drop. prettier
hugs each `}` and breaks the component internally (`prettier_variant_hug.svelte`).

prettier keeps tsv's dropped form stable, so the divergence shows only when normalizing
the compact one-liner (`unformatted_ours_compact.svelte`) — tsv normalizes both it and
prettier's hug back to `input.svelte` in one pass.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads, body
shapes, and **sections/branches** (one-pass `conditional_group`, no breakable
special-case). See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [await/element_body_long](../element_body_long_prettier_divergence/) — the same drop, then/catch shorthand inside `<Container>`
- [await/long](../long_prettier_divergence/) — the full-form section expansion with text bodies
- [elements/inline_component_else_body_long](../../../elements/inline_component_else_body_long_prettier_divergence/) — the same per-section drop in an `{:else}` branch
