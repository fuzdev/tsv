# trailing_text_head_wrap_inline_parent_long_prettier_divergence

An `{#await}` as the content of an **inline element** that **trailing text** follows after a
space (`<b>{#await …}…{/await}</b> tail`), at the 100/101 boundary of the block-head line. The
element is either the last child before the tail or sits between two text runs of a paragraph,
and one pair carries a `then` clause. At 100 the head stays on one line; at 101 it wraps and its
`}` dangles (a `then` clause dangles with it), exactly as it does with no trailing text. In every
case the construct is too wide for one line, so the element lays out block-style, the body drops,
and the tail hugs the intact closing tag.

- `unformatted_ours_compact.svelte` — every case authored on one line; tsv normalizes it
  to `input.svelte`, prettier to `prettier_variant_hug.svelte`.
- `prettier_variant_hug.svelte` — prettier's form of the compact authoring: the element's
  delimiters dangle (`<b⏎\t\t>…</b⏎\t>`) and the block stays inline past print width.
  Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: prettier never wraps a
  block head, so it rejoins each wrapped head onto one line, past print width.

## Reason

A block head wraps and dangles its `}` once its line exceeds print width, whatever the parent
and whatever follows the parent. Trailing text after the element is what folds the element and
the text into one flowing run; the head must still reach the same form in one pass. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [await/sibling_head_wrap_inline_parent_long](../sibling_head_wrap_inline_parent_long_prettier_divergence/) — the head wrap after a breakable sibling inside an inline parent
- [await/inline_element_long](../inline_element_long_prettier_divergence/) — the head wrap inside an inline element with no trailing text
- [elements/inline_content_expression_break_trailing_long](../../../elements/inline_content_expression_break_trailing_long_prettier_divergence/) — an expression that breaks inside the same trailing-text shape
