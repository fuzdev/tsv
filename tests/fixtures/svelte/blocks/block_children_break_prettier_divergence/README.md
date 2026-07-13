# block_children_break

Consecutive block elements inside a block force its body to break (prettier's
`forceBreakContent` + `breakParent`, which tsv mirrors). Both formatters agree the body
goes multiline; they disagree on what happens to the **boundary**.

tsv lays the broken body out block-style — head and tail intact, every child on its own
indented line. Prettier keeps the body **welded** to the head and the close tag
(`{#if cond}<div>text1</div>⏎\t<div>text2</div>{/if}`), because its block layout is driven
by authored boundary whitespace, never by whether the body actually breaks. That boundary
is render-free under Svelte 5, so it cannot select the layout — see
`prettier_variant_hugged.svelte`, the form prettier keeps stable and tsv converges to
`input.svelte`.

A single block element still stays inline (`{#if cond}<div>text</div>{/if}`) — nothing
forces the body to break, so there is no layout at stake.

The general statement of this rule across every block type and branch is
[content_boundary_convergence](../content_boundary_convergence_prettier_divergence/); the
one-sided-weld case is
[if/hugged_boundary_convergence](../if/hugged_boundary_convergence_prettier_divergence/).

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
and [§Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).
