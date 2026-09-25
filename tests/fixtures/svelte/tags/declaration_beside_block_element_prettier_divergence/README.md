# declaration_beside_block_element_prettier_divergence

A declaration (`{@const}`, `{const}`, `{let}`, `{#snippet}`) takes its own line unless it is
glued to content on **both** sides. A block element beside it is not that content: it owns its
own line, so the boundary beside it is a break either way, and whitespace at a block-level
boundary is not rendered. A declaration between a block element and content (text, an expression
tag, an inline element) is therefore glued on one side only, and it takes its line, the content
beside it too (`<div>{y}</div>⏎{@const z = 1}⏎t`).

The cases cover `{#snippet}`, `{@const}`, `{const}` and `{let}`; text, an expression tag or an
inline element after the declaration, and text or an inline element before it; inline, component,
block and block-body parents and the root; and a block element too wide for its line. The control
is a declaration glued to text on both sides, which stays glued.

- `unformatted_ours_compact.svelte` — every case on one line. tsv normalizes it to
  `input.svelte`; prettier to `prettier_variant_hugged.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: the declaration
  stays glued to the content beside it (`{@const z = 1}t`). Prettier-stable; tsv normalizes it to
  `input.svelte`.

Prettier keeps `input.svelte` as written. Every form renders the same.

## Reason

Design choice. The declaration's own line is the layout tsv gives every declaration whose break
is render-free, and a break beside a block element is. Counting the block element as glued
content would make the answer depend on the formatter's own output: the first pass puts the block
element on its own line and keeps the declaration glued to the content, and the next pass reads
the declaration as glued on one side only and splits it from the content. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [tags/declaration_own_line](../declaration_own_line_prettier_divergence/) — the declaration's own line, and the glued-both-sides exception
- [blocks/snippet/own_line](../../blocks/snippet/own_line_prettier_divergence/) — the same rule for `{#snippet}`
- [elements/inline_parent_block_child_await](../../elements/inline_parent_block_child_await_prettier_divergence/) — a block element beside an `{#await}`
