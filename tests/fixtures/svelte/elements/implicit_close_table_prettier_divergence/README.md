# implicit_close_table_prettier_divergence

Two things at once: the parser's **implicit end-tag** handling for table sections/rows/cells
(`<thead>` closed by the next `<tbody>`, `<td>` by `<th>`, `<tr>` by `<tr>` — captured in
`expected.json`), and the formatter's **block-style layout** of the table.

`<table>` and its `<tbody>` are whitespace-collapsing containers (Svelte's `clean_nodes`
`can_remove_entirely`), so their inter-sibling whitespace never renders; tsv lays the content
out block-style — each section, row, and cell on its own line — with the inter-sibling
whitespace trimmed. Prettier keeps a container authored inline/glued on one line.

- `prettier_variant_glued.svelte` — prettier's stable glued form (sections and rows on one
  line); tsv normalizes it to the block-style `input.svelte`.
- `unformatted_ours_implicit.svelte` — the maximally implicit-close authoring
  (`<td>a<th>b</tr><tr>` with no explicit end tags); tsv normalizes it to `input.svelte`,
  prettier normalizes it to the glued form (not `input.svelte`).

## Reason

Design choice, render-free under Svelte 5 — the same whitespace-collapsing-container rule as
[ws_collapsing_containers](../ws_collapsing_containers_prettier_divergence/). Verified against
`svelte/compiler` `clean_nodes` and by compile-diff (the block-style and glued forms compile
identically).
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
