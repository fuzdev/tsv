# attrs_long_prettier_divergence

`<svelte:component>` at the print-width boundary: self-closing (attributes wrap at 101), and with a
child element.

The self-closing cases match prettier. The divergence is the last one: once the attributes wrap, the
content goes multiline, and tsv lays it out **block-style** — both tags intact, the child on its own
indented line. Prettier keeps the child glued to the tags and **dangles** both delimiters
(`⏎\t><span>child</span></svelte:component⏎>`), because the source hugged the content boundary.

That hug is render-free under Svelte 5 (start/end-of-content whitespace is removed at compile), so it
carries no signal and must not select the layout. A `svelte:*` element runs the same layout analysis
as a regular one, so it converges on the same block-style form
([elements/inline_content_hug_long](../../elements/inline_content_hug_long_prettier_divergence/)).

`prettier_variant_dangle` is prettier's stable form; the `unformatted_ours_*` variants are other
authorings tsv converges on `input` and prettier does not.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
