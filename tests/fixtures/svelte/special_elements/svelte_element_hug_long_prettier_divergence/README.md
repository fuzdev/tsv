# svelte_element_hug_long_prettier_divergence

`<svelte:element>` at the print-width boundary: 100 chars stays inline, 101 self-closing wraps its
attributes, and 101 with a text child goes multiline.

The first two cases match prettier. In the third, tsv lays the content out **block-style** — both
tags intact, the text on its own indented line — while prettier keeps the text glued to the tags and
**dangles** both delimiters (`⏎\t>text</svelte:element⏎>`), because the source hugged the content
boundary.

That hug is render-free under Svelte 5, so it carries no signal and must not select the layout.
`<svelte:element>` runs the same layout analysis as a regular element, so it converges on the same
form ([elements/inline_content_text_wrap](../../elements/inline_content_text_wrap_prettier_divergence/)).

`prettier_variant_dangle` is prettier's stable form; the `unformatted_ours_*` variants are other
authorings tsv converges on `input` and prettier does not.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
