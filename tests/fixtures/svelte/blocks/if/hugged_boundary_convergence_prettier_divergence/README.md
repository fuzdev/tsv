# hugged_boundary_convergence

A block body's content boundary is **render-free** (Svelte trims start/end-of-content
whitespace from every fragment at compile), so it carries no authorship signal and cannot
select the layout. The consequence pinned here: **hug is all-or-nothing.**

A body welded to the head only (`prettier_variant_hugged_leading`), to the tail only
(`prettier_variant_hugged_trailing`), or welded on one branch while another breaks
(`prettier_variant_hugged_mixed`) all lay out block-style in tsv — head and tail intact,
every branch body on its own indented line. A one-sided weld is not an expansion signal
any more than a one-sided break is; the same invariant `ElementLayout::WithContent(BoundaryMode)`
carries for an element's content boundary, where a lone opening newline likewise collapses
back rather than half-breaking the tags.

Prettier keeps a *different* stable form for each of these authorings — all four files here
are one document, and prettier preserves all four. tsv converges them onto `input.svelte`.
The `unformatted_ours_*` variants are authorings only tsv normalizes to `input.svelte`
(prettier takes them to one of its welded forms instead).

The cross-block-type statement of the same rule — `{#each}` / `{#key}` / `{#snippet}` /
each `{#await}` phase — is
[blocks/content_boundary_convergence](../../content_boundary_convergence_prettier_divergence/).

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
and [§Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).
