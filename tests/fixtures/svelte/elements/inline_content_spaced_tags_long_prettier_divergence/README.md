# inline_content_spaced_tags_long_prettier_divergence

An inline element (`<small>`) whose content is two **space-separated expression
tags** (`{a} {b}`) and overflows print width.

**tsv** lays the element out **block-style** — both tags intact, content on its own
indented line — and once the element is multiline each expression tag takes its own
line. The separating whitespace is a collapsible break: a space while the element
fits on one line, a newline once it breaks. Content that fits stays inline (the
100-char case). Text between the tags (the third case) flows as one fill, so the
tags share the content line.

**Prettier** keeps the tags split the same way once the element is multiline, but on
a compact single-line authoring it instead **dangles** the closing delimiter
(`<small>{a} {b}</small⏎>`) and lets the content run past printWidth — see
`prettier_variant_compact`.

The layout is driven by width, never by the authored boundary whitespace: the
compact authoring (`unformatted_ours_compact`) and prettier's dangle
(`prettier_variant_compact`) both normalize to `input.svelte` under tsv in one pass,
so the block-style form tsv emits is its own fixed point.

## Reason

tsv treats printWidth as a hard limit and prefers block-style content over a dangled,
over-width line. Content-boundary whitespace is render-free under Svelte 5, so it must
not select the layout — otherwise the block-style form tsv emits would reflow on the
next pass. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
