# inline_content_break_before_trailing_long_prettier_divergence

Text followed by one more unit — an `{#await}`, an inline element, or an expression tag — as the
content of an **inline element** that **trailing text** follows after a space
(`lead <span>text {…}</span> tail`, and once with no text before the element), at the 100/101
boundary of the content line. At 100 the text and the unit share the line; at 101 the unit is
too wide to stay behind the text, so it starts a fresh line and the text keeps its own, exactly
as it does with no trailing text.

`input.svelte` is prettier's own form: the content breaks before the unit the same way with or
without the trailing text, and the tail hugs the intact closing tag. The divergence is the
one-line authoring, which tsv lays out block-style.

- `unformatted_ours_compact.svelte` — every case authored on one line; tsv normalizes it to
  `input.svelte`, prettier to `prettier_variant_hug.svelte`.
- `prettier_variant_hug.svelte` — prettier's form of the compact authoring: the element's
  delimiters dangle (`<span⏎\t\t>…</span⏎\t>`) and the content stays on one line, past print
  width. Prettier-stable; tsv normalizes it to `input.svelte`.

## Reason

Content that overflows lays out block-style (both tags intact, the content on its own indented
lines), and a unit that no longer fits behind the text before it starts a fresh line. Trailing
text after the element does not change either decision. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [inline_content_expression_break_trailing_long](../inline_content_expression_break_trailing_long_prettier_divergence/) — an expression that breaks inside the same trailing-text shape
- [inline_break_before_wrap_long](../inline_break_before_wrap_long_prettier_divergence/) — the break before an inline element, with no enclosing trailing-text run
- [blocks/multiline_head_after_text](../../blocks/multiline_head_after_text_prettier_divergence/) — the break before a block that renders multiline
