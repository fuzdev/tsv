# block_after_spaced_text_kinds_prettier_divergence

[block_after_spaced_text](../block_after_spaced_text_prettier_divergence/) across every block
**kind** and every fragment **family** it does not vary: a `<pre>`, a void `<hr />`, a
whitespace-collapsing `<table>` (which lays out block-style), a list, a heading, the
block-classified `svelte:*` specials at the root (`<svelte:head>`, `<svelte:window />`), a block
whose attributes wrap, a one-punctuation text, and the `{#each}` / `{#snippet}` / `{:else}` /
`{#await}` bodies and `<slot>` / `<svelte:boundary>` containers. Every one takes its own line
after a spaced text. Prettier hugs each to a first-child text (`prettier_variant_space.svelte` —
for the attribute-wrapping block it dangles the attributes from the text line; for the
`svelte:*` specials it has no block classification at all and hugs them after any predecessor);
tsv normalizes that spelling, and a tab-spelled separator (`unformatted_ours_tab.svelte`, which
prettier respells as the space), to `input.svelte` in one pass.

## Reason

Design choice — the same rule as the parent fixture, pinned across the axes a rule keyed on a
node's kind or its fragment could have split on.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
