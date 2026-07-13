# closing_tag_inline_content_prettier_divergence

A `<slot>` / `<svelte:element>` with wrapping attributes and hugged expression content, directly
followed by an `{#if}` sibling.

Once the attributes wrap, the content goes multiline and tsv lays it out **block-style** — both tags
intact, `{expr}` on its own indented line — with the `{#if}` sibling still hugging the intact
`</slot>`. Prettier hugs the content to the tags and **dangles** the closing delimiter, so the
sibling ends up hugging a bare `>` on its own line (`</slot⏎\t>{#if cond}…`).

The content boundary is render-free under Svelte 5, so it must not select the layout. Note the
sibling boundary is a *different* boundary: whitespace *between* nodes IS render-significant, and tsv
preserves it — the `{#if}` hugs the closing tag in both formatters, because that is how it was
authored.

The third case (short attributes, everything inline) matches prettier, pinning that the divergence
appears only when the element actually breaks.

`prettier_variant_dangle` is prettier's stable form; `unformatted_ours_compact` is a compact authoring
tsv converges on `input` and prettier does not.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
