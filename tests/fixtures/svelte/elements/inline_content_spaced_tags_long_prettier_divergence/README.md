# inline_content_spaced_tags_long_prettier_divergence

An inline element (`<small>`) whose content is two **space-separated expression tags**
(`{a} {b}`) and overflows print width.

**tsv** lays the element out **block-style** — both tags intact, content on its own indented
line — and the tags **pack on that content line**: the separating whitespace is a fill boundary
that breaks per width, never with the container. Content that fits stays inline (the 100-char
case). Text between the tags (the third case) flows as one fill the same way.

**Prettier** puts each tag on its own line once the element is multiline — its `line` between
two tags breaks with the container (`output_prettier.svelte`) — and on a compact single-line
authoring instead **dangles** the closing delimiter (`<small>{a} {b}</small⏎>`) and lets the
content run past printWidth (`prettier_variant_compact`).

The layout is driven by width, never by the authored boundary whitespace: the compact authoring
(`unformatted_ours_compact`) and prettier's dangle (`prettier_variant_compact`) both normalize to
`input.svelte` under tsv in one pass, so the block-style form tsv emits is its own fixed point.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

tsv treats printWidth as a hard limit and prefers block-style content over a dangled, over-width
line. Content-boundary whitespace is render-free under Svelte 5, so its spelling must not select
the tags' layout — and the block-style form tsv emits is its own fixed point, since the emitted
boundary air re-selects the multiline form on the next parse. The separator between the tags is
the fill boundary of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/): it packs per width
whatever the run holds and is never forced open by the container; where the packed line itself
reaches print width is
[inline_content_spaced_tags_pack_long](../inline_content_spaced_tags_pack_long_prettier_divergence/).
See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
