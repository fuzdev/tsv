# inline_wide_content_trailing_sandwiched_long_prettier_divergence

The **sandwiched** counterpart to `inline_wide_content_trailing_long`: a wide inline element whose
**own content** overflows, but here a text sibling **precedes** it (so a preceding break can push
it onto its own line), followed by **terminal** trailing text authored with a **space** boundary.

tsv lays the element out **block-style** (both tags intact, the over-wide content wrapped within
printWidth) and the space-authored terminal text **hugs the intact closing tag** (`</tag> tail`),
respecting the author's space — exactly as the first-child case does. A preceding sibling does
**not** change that terminal trailing text hugs, because nothing follows the tail, so the hug stays
convergent (contrast `inline_wide_content_text_sibling_long`, where the text is *non-terminal* and
hugging it would be non-convergent). This is the guard that the terminal-hug is scoped to *terminal*
trailing text, **not** to a first-child element — a wide element that drops to its own line via a
preceding break still lets its terminal tail hug.

Prettier keeps this block-style form (a leading sibling forces the paragraph multiline, so prettier
lays the wide child block-style too and hugs the tail the same way). On a **single-line** authoring
it instead keeps the content on one over-width line and **dangles** the delimiter
(`<tag …>…content…</tag⏎> tail`) — see `prettier_variant_compact`, which prettier keeps stable and
tsv normalizes to `input`. So the divergence is the block-style vs dangle layout, identical to the
first-child sibling.

The `unformatted_ours_multiline` variant pins idempotence: the single-line authoring normalizes to
`input` under tsv.

## Reason

A wide inline element's content lays out block-style to honor printWidth (prettier keeps it inline
and dangles); its *terminal* trailing text respects the author's **space** boundary and hugs the
closing tag, whether the element is a first child or sandwiched by a preceding sibling — the hug is
convergent because nothing follows the tail.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
