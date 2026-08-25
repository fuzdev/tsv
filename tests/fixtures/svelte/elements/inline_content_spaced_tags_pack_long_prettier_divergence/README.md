# inline_content_spaced_tags_pack_long_prettier_divergence

Space-separated expression tags **pack per width** on their content line. The separator between
two tags is a fill boundary: a space while the next tag fits, a break only before the tag that
would cross print width — a pair at exactly 100 chars packs and at 101 breaks before the second
tag, and a three-tag run in a block-styled `<small>` packs all three at 100 and keeps the first
two packed at 101, breaking only before the third. One tag per line is never the answer to width.

**Prettier** breaks its `line` between two tags with the container, so once the container is
multiline every tag takes its own line (`output_prettier.svelte`); on a compact authoring it
pre-breaks the opening tag and dangles the closing one instead.

`unformatted_ours_compact.svelte` is the hugged authoring of every case: tsv lays the container
out block-style and packs the run to `input.svelte` in one pass.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the print-width boundary of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/): the whitespace-only
separator before a tag defers to the tag's per-width `group([line, tag])`, so the run reflows
exactly as a fill of words does, whatever the run holds. The element-boundary side of the same
shape — block-style where prettier dangles — is
[inline_content_spaced_tags_long](../inline_content_spaced_tags_long_prettier_divergence/). See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
and
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
