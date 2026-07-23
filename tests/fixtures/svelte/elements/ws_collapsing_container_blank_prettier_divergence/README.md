# ws_collapsing_container_blank_prettier_divergence

A whitespace-collapsing container (`select` / `table` / … — Svelte's `clean_nodes`
`can_remove_entirely`; see the sibling
[ws_collapsing_containers](../ws_collapsing_containers_prettier_divergence/)) lays out
**block-style**. This fixture pins the one thing block-style must still carry across the
render-free inter-sibling boundary: an **authored blank line**.

An inter-sibling whitespace-only run is render-free in these containers (the compiler removes
it entirely), so tsv trims it — but a blank line (2+ newlines) is a Tier-2 *authoring* signal,
not a render one, and tsv preserves it everywhere else it lays out block-style (element/component
content, block bodies). So inside a collapsing container a blank line between two children is
kept (collapsed to a single blank, as everywhere), sitting between the two block-style lines.

tsv: block-style, the authored blank line preserved between `<option>a</option>` and
`<option>b</option>`.

Prettier: from a block-style authoring prettier keeps the same block-style form (so `input.svelte`
is a shared fixed point); from a compact authoring it **dangles** the container delimiters instead
— see `prettier_variant_dangle.svelte` (prettier keeps that form stable; tsv normalizes it back to
`input.svelte`). `unformatted_ours_compact.svelte` is the compact authoring both formatters start
from: tsv normalizes it to `input.svelte`, prettier normalizes it to the dangle form.

## Reason

Design choice, render-free under Svelte 5. The inter-sibling whitespace is trimmed because the
compiler removes it; the blank line survives because blank-line preservation is an authoring
concern independent of render, uniform with every other block-style boundary tsv produces.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
