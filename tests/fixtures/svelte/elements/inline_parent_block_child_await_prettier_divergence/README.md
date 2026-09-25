# inline_parent_block_child_await_prettier_divergence

An inline element or component holding a **block element** beside an `{#await}`. A block
element beside any control-flow block is mixed content, exactly as beside an `{#if}` or an
expression tag, so the parent lays out multiline: the block element and the `{#await}` each on a
line of their own, block-style.

That holds whatever the block element's width, in either order, with a `then` clause or a
`{:catch}` branch, with a space between the two, for a component parent, inside an inline parent,
and nested. A block parent whose `{#await}` comes first takes the same form, at any width.
Before a glued block the parent keeps its own `>` (`</b>{#if c}`): its multiline cause is
structural, and a structurally multiline element never sheds its `>`.

Two controls: an `{#if}` beside a block element, which already lays out this way, and a parent
with no block element, which keeps a short `{#await}` inline.

- `unformatted_ours_compact.svelte` — every case on one line. tsv normalizes it to
  `input.svelte`; prettier to `prettier_variant_hugged.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: the same lines,
  with the parent's delimiters dangled (`<b⏎\t><div>…</div>⏎\t{#await p}x{/await}</b⏎>`).
  Prettier-stable; tsv normalizes it to `input.svelte`.

Prettier keeps `input.svelte` as written. Every form renders the same: the forms differ only in
whitespace beside a block element or at a content boundary, which is not rendered.

## Reason

Design choice. A layout keyed on the element's own authored line breaks can be re-decided on the
next pass, so mixed content must be a structural cause, and it must count every control-flow
block. Were an `{#await}` not counted, a parent too wide for its line would print the
`{#await}` hugging the block element (`</div>{#await p}x{/await}`), and the next pass would read
that output as authored multiline and split them. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
(`{#await}`).

## Related

- [elements/await_snippet_breakable_sibling](../await_snippet_breakable_sibling_prettier_divergence/) — a short `{#await}` after a breakable sibling in a block parent
- [components/await_sibling_inline](../../components/await_sibling_inline/) — a component parent with no block element stays inline
- [tags/declaration_beside_block_element](../../tags/declaration_beside_block_element_prettier_divergence/) — a `{#snippet}` or a declaration tag beside a block element
