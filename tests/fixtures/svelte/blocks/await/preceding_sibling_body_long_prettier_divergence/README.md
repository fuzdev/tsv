# preceding_sibling_body_long_prettier_divergence

An `{#await … then}` whose **overflowing body** follows a preceding inline sibling
(an expression tag `{x}`). tsv **drops the body to its own line** (uniform
body-expand), while prettier **hugs** the `}` and breaks the element internally
(`prettier_variant_hug.svelte`).

The body-drop is **independent of the head `}` dangle**: the head here is short and
stays on one line, yet the body still drops on overflow. (A head past print width after
the same sibling wraps and dangles its `}` like any other —
[sibling_head_wrap_inline_parent_long](../sibling_head_wrap_inline_parent_long_prettier_divergence/)
pins it in an inline parent.)
The `{x}` expression tag has no closing `>`, so there is no sibling-`>` dangle here —
this isolates the body-drop from the [sibling-`>` dangle](../../../elements/inline_sibling_gt_dangle_prettier_divergence/).

`unformatted_ours_compact.svelte` (the inline one-liner) normalizes to `input.svelte`
under tsv; `prettier_variant_hug.svelte` is prettier-stable, and tsv normalizes it to
`input.svelte` too.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and
body shapes — keyed on whether the construct overflows, not on whether the head wraps.
See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [await/element_body_long](../element_body_long_prettier_divergence/) — the same drop with no preceding sibling
- [elements/inline_sibling_gt_dangle](../../../elements/inline_sibling_gt_dangle_prettier_divergence/) — the sibling-`>` dangle (inline-element sibling) for all 5 block heads
