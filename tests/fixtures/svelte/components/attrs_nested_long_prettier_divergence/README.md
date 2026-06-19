# attrs_nested_long_prettier_divergence

A `{#if}` whose body is a component with attributes, deeply nested (7 levels →
14-char indent) so the construct exceeds printWidth, sitting between a leading `>`
boundary and a trailing `&nbsp;{expr}`. tsv **drops the body to its own line** (the
component then fits on one line at the body indent), while prettier **hugs** the `}`
and wraps the component's attributes internally (`prettier_variant_hug.svelte`).

This exercises the drop in real deep context with inline boundaries on both sides —
the `>` open and the `{expr}` sibling stay hugged to the block while only the body
expands.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and
body shapes (one-pass `conditional_group`, no breakable special-case). See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop, standalone
- [elements/inline_if_sibling_fill_long](../../elements/inline_if_sibling_fill_long_prettier_divergence/) — a `{#if}` body drop next to an inline sibling
