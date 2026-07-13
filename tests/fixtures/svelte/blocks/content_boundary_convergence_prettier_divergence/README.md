# content_boundary_convergence

A block body's content boundary — the whitespace between `{#if cond}` and its first
child, and between its last child and `{/if}` — is **render-free** under Svelte 5: the
compiler removes start/end-of-content whitespace from every fragment, and a block branch
is a fragment exactly like an element's content. So it carries no authorship signal and
must not select the layout.

tsv lays a multiline block body out **block-style** — head and tail intact, body on its
own indented lines — however the author wrote that boundary. Prettier lets the boundary
decide, so it keeps a *different* stable form for each authoring: welded on both sides
(`prettier_variant_hugged`), welded to the head only (`prettier_variant_hugged_leading`),
or welded to the tail only (`prettier_variant_hugged_trailing`). All four forms here are
one document; prettier keeps all four stable, tsv converges them onto `input.svelte`.

This covers every fragment a block can open — `{#if}` / `{#each}` / `{#key}` /
`{#snippet}` / each `{#await}` phase — and every *branch* boundary (`{:else if}`,
`{:else}`, `{:then}`, `{:catch}`), not just the first.

A body that fits on one line still collapses inline (`{#if cond}<div>a</div>{/if}`) —
the stance is about layout, and layout is only at stake once the body breaks.

The same rule, for the other fragment family (an element's content), is
[content_boundary_convergence](../../components/content_boundary_convergence_prettier_divergence/).

Not to be confused with the two dangles tsv keeps deliberately — the block **head** `}`
dangle (`{#if cond⏎}`, keyed on head width) and the **sibling `>`** dangle
(`</a⏎>{#each …}`, keyed on *inter-sibling* whitespace, which Svelte keeps). Neither is a
content boundary.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
and [§Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).
