# Computed access, own-line block from a stripped shell before the `[`

The stripped-shell spelling of
[computed_pre_bracket_block_comment](../computed_pre_bracket_block_comment_prettier_divergence/):
an own-line block in grouping parens the parser strips from around a computed
access in a call chain (`(⏎/* c1 */⏎((x.y) /* t1 */)[i])[j].u()`), ahead of the
block the author trailed after the object. The `unformatted_ours_stripped_shell`
variant holds the authored shells; `input.svelte` is where tsv prints them, in one
pass.

- **tsv**: the own-line block leads the chain, on its own line, and the trailing
  block keeps its place on the object's line before the `[`, for an unsealed (c1)
  and a sealed (c2) object alike. The two stay in authored order: the access's gap
  takes the shell's comment only while it is the one comment that gap prints
  ([stripped_shell_comment_source_order](../../calls/chained/stripped_shell_comment_source_order_prettier_divergence/)).
- **prettier**: glues the own-line block behind the trailing one
  (`x.y /* t1 */ /* c1 */[i][j]`), reordering the two, and for the sealed object
  pulls both inside the pair that seals the chain
  (`(x?.y /* t2 */ /* c2 */)[i][j].u();`). From `input.svelte` it keeps c1's line and
  pulls only the trailing block inside the sealing pair (`(x?.y /* t2 */)[i][j].u();`,
  `output_prettier.svelte`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
