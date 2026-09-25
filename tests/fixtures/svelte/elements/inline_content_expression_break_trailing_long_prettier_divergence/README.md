# inline_content_expression_break_trailing_long_prettier_divergence

An expression tag as the content of an **inline element** that **trailing text** follows after a
space (`<b>{…}</b> tail`), at the 100/101 boundary of the line where the expression breaks. Four
expressions, each of which decides one break and then indents the lines that follow it only if
that break was taken:

- a **curried arrow chain** — at 101 its heads break one per line and the body indents under
  the last head;
- an **assignment** — at 101 the value breaks after the `=`, and the value's own lines (a
  conditional's `?` / `:`) indent one level under it;
- a **type parameter constraint** — at 101 the constraint breaks after `extends`, and its own
  lines (the type arguments) indent one level under it;
- a **type parameter default** — at 101 the default breaks after `=`, and its own lines (the
  type arguments) indent one level under it. The list has two type parameters, since a lone
  parameter with a default is printed with a trailing comma (`<T,>`).

`input.svelte` is prettier's own form: every expression breaks the same way it does with no
trailing text, and the tail hugs the intact closing tag. The divergence is the one-line
authoring, which tsv lays out block-style.

- `unformatted_ours_compact.svelte` — every case authored on one line; tsv normalizes it to
  `input.svelte`, prettier to `prettier_variant_hug.svelte`.
- `prettier_variant_hug.svelte` — prettier's form of the compact authoring: the element's
  delimiters dangle (`<b⏎\t\t>{…}</b⏎\t>`) around the same broken expression. Prettier-stable;
  tsv normalizes it to `input.svelte`.

## Reason

Expression content that overflows lays out block-style (both tags intact, the content on its own
indented line), and trailing text after the element does not change how the expression breaks
there. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [inline_content_hug_long](../inline_content_hug_long_prettier_divergence/) — expression content laid out block-style, with no trailing text
- [inline_wide_content_trailing_long](../inline_wide_content_trailing_long_prettier_divergence/) — wide text content followed by trailing text
- [await/trailing_text_head_wrap_inline_parent_long](../../blocks/await/trailing_text_head_wrap_inline_parent_long_prettier_divergence/) — a block head that wraps inside the same trailing-text shape
