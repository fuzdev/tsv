# block_children_prettier_divergence

A `<slot>` whose children are block elements. The block children force the slot multiline, and it
lays out **block-style** — both tags intact, content on its own indented lines — exactly like the
`<span>` beside it in the same shape.

That parity is the point. `svelte:*` elements (and `<slot>`, `<title>`) run the **same** layout
analysis as regular elements rather than a private copy of it, so "block children force multiline"
reaches them too. A private copy is how the two drifted: a `<slot>` used to stay on one line here
where a `<span>` expanded.

Prettier keeps the content glued to the tags and dangles both delimiters
(`prettier_variant_dangle`, which tsv normalizes to `input`). The hugged content boundary is
render-free under Svelte 5 — start/end-of-content whitespace is removed at compile — so it carries
no signal and must not select the layout; `unformatted_ours_hug` is that hugged authoring, which
tsv likewise converges on `input`.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
