# Computed access, own-line block from a stripped shell before the `[`

The stripped-shell spelling of
[computed_pre_bracket_block_comment](../computed_pre_bracket_block_comment_prettier_divergence/):
an own-line block in grouping parens the parser strips from around a computed
access in a call chain (`(⏎/* c1 */⏎((x.y) /* t1 */)[i])[j].u()`). The chain
places the block in that access's object→`[` gap, together with the block the
author trailed after the object. The `unformatted_ours_stripped_shell` variant
holds the authored shells; `input.svelte` is where tsv prints them, in one pass.

- **tsv**: the trailing block stays on the object's line and the own-line block
  keeps its own line before the `[`, the gap's own-line answer, for an unsealed
  (c1) and a sealed (c2) object alike. The own-line block sits *ahead* of the
  trailing one in source, so it is printed from the gap's own classification
  rather than found by a rescan from past the trailing block.
- **prettier**: does not keep the own-line block's line. Its fixed point glues it
  behind the trailing block (`x.y /* t1 */ /* c1 */[i][j]`), and for the sealed
  object pulls both inside the pair that seals the chain
  (`(x?.y /* t2 */ /* c2 */)[i][j].u();`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
