# inline_sibling_newline_interior_blank_prettier_divergence

The sibling-newline flow rule at an authored blank line **inside one text node**
(`text1 text2⏎⏎text3 text4`, a single Text node) — the one blank that bounds nothing.
An authored blank bounds a run wherever the parser puts it beside a sibling — a whitespace-only
node, or a content text's edge whitespace
([inline_sibling_newline_run_bounded](../inline_sibling_newline_run_bounded_prettier_divergence/))
— but a run is a partition of nodes and cannot be split inside one, so an interior blank bounds
nothing: the node's four words make the run prose, and the sibling newlines on both sides of it
flow (`prettier_variant_newline.svelte`, which prettier keeps and tsv normalizes to
`input.svelte`).

## Reason

Design choice, and the honest one: the interior blank is collapsed by the fill under **both**
formatters (`unformatted_ours_interior_blank.svelte` carries it — tsv normalizes it to
`input.svelte`, prettier to `prettier_variant_newline.svelte`, the blank gone in each), so there
is no blank left for a run boundary to protect. Holding the sibling newlines on account of a
blank the output does not contain would key the layout on bytes the formatter erases — a layout
selected by nothing the next pass can see. The element-interior reader
(`content_is_reflowable_fill`) asks the same two edge predicates, so both readers of the one fill
answer agree at every blank — this node included, where neither bounds and the element keeps its
layout ([content_interior_blank_collapse](../content_interior_blank_collapse/) is that half).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
