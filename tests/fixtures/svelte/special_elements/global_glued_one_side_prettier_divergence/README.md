# global_glued_one_side_prettier_divergence

A global `svelte:*` element (`<svelte:window>` / `<svelte:document>` / `<svelte:body>` /
`<svelte:head>`) glued to content on **one** side only. tsv gives it its own line from every
authoring; prettier keeps each authoring as its own stable form.

- `prettier_variant_glued_before.svelte` — each element glued to the node before it: the text
  after `<svelte:window>` is spaced, and `<svelte:body>` and `<svelte:document>` sit at the
  fragment's edges.
- `prettier_variant_glued_after.svelte` — `<svelte:window>` glued to the text after it, spaced
  from the text before it.

tsv normalizes both to `input.svelte`; all three render identically.

## Reason

Design choice, render-free under Svelte 5. The compiler hoists these four elements out of the
fragment before its whitespace rules run, so the break tsv writes on the glued side merges with
the whitespace on the other side into the one rendered space that was already there — or, at a
fragment edge, is trimmed with it. The break is therefore free, and tsv spends it on the element's
own line, as it does for a space-authored one
([special_element_newline_flow](../../elements/special_element_newline_flow_prettier_divergence/)).
Glued on **both** sides the break is not free — it would render a space between neighbours that
meet directly — so there the element stays on the content's line under both formatters
([global_glued_both_sides](../global_glued_both_sides/) and its siblings).

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
