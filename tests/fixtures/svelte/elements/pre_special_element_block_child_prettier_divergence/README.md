# pre_special_element_block_child_prettier_divergence

A block (`{#if}`, `{#each}`, `{#key}`, `{#await}`) among a special element's content inside
`<pre>`. The block's tags stay glued to the text around them, since a break beside a block body
there is rendered text, and the special element's own tags follow the one rule of the
whitespace-sensitive family: a delimiter moves for the content's edge and for width, never for
the kind of child the content holds. An inline element and a component in the same position take
the same layout (the last two cases).

- `input.svelte` — tsv's form: every case fits, so no tag breaks.
- `unformatted_ours_tag_split.svelte` — every tag broken before its `>`; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier breaks the `>` onto the next line and dangles the closing
  tag's `>` whenever the content holds an `{#if}`, `{#each}` or `{#key}` block, at any width and
  whatever the parent (an inline element, a component or a special element). An `{#await}` block
  moves nothing, so the `<slot>` row, which holds one, stays whole and matches tsv. Both layouts
  break only inside tags, so both render the same.

## Reason

Design choice: the family's delimiters read the content's edges, not its node kinds, so a
special element, an inline element and a component answer this position the same way. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
