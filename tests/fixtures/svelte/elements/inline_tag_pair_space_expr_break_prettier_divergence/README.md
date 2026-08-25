# inline_tag_pair_space_expr_break_prettier_divergence

A space-spelled tag pair where one expression must break. When the **first** tag's expression
breaks, the second tag hugs its closing `)}` line (`)} {expr}`) — the tag expands inside its own
braces and leaves its adjacencies untouched, so the boundary after it is the ordinary per-width
fill boundary, exactly as an inline element or a word takes after a broken tag. Prettier's bare
`line` between two tags breaks with the container and puts `{expr}` on its own line
(`output_prettier.svelte`).

The two controls agree with prettier: the **newline** spelling of that boundary is held (the
prose gate holds an authored newline in a wordless run), and a **second** tag whose expression
cannot fit flat after its predecessor travels to a fresh line whole and breaks internally there
([fill_spaced_tag_travel_long](../fill_spaced_tag_travel_long_prettier_divergence/) for the text
predecessor).

`unformatted_ours_space.svelte` is the one-line authoring of every case: tsv breaks each
expression and normalizes to `input.svelte` in one pass; prettier lands on `output_prettier`.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the rule of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/) at a predecessor that
renders multiline. A TAG predecessor is excluded from the layout-keyed hold
([sibling_newline_after_multiline](../sibling_newline_after_multiline_prettier_divergence/))
because its break lands inside its own expression, so the space-spelled boundary after it stays
the per-width hug for every follower kind; only the newline spelling is held, and by the prose
gate rather than the multiline predecessor.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
