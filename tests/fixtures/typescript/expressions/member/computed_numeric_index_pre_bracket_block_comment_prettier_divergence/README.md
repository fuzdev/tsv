# computed_numeric_index_pre_bracket_block_comment_prettier_divergence

An own-line block comment in a computed access's object→`[` gap where the index is a
**numeric literal** — the numeric-index form of
[computed_pre_bracket_block_comment](../computed_pre_bracket_block_comment_prettier_divergence/),
split out the way its `//` counterpart
([computed_numeric_index_pre_bracket_line_comment](../computed_numeric_index_pre_bracket_line_comment_prettier_divergence/))
is: the index kind changes prettier's destination, because its member-chain grouping
glues a numeric-index access into the preceding group instead of starting a new one.

**tsv** treats the gap exactly as for a non-numeric index: the comment keeps its own
line and the bracket follows on the next line, for a bare object and at the end of a
call chain alike — one authored position, one answer.

**Prettier** is non-idempotent here: pass 1 relocates the comment *inside* the
brackets (`arr[/* c1 */⏎0]`, the committed `output_prettier.svelte`), and pass 2
carries it back *out*, glued before the `[` (`arr /* c1 */[0]`) — the fixed point the
`audit_signature.txt` pins. The comment crosses the `[` boundary twice and ends at a
position the author never wrote, with its own-line-ness erased.

```
// tsv                     // prettier (pass 1 → pass 2)
const a =                  const a =              const a = arr /* c1 */[0];
	arr                        arr[/* c1 */
	/* c1 */                   0];
	[0];
```

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
rather than relocating it across the `[` boundary — the same answer the identifier
index gets, so the index kind cannot flip the comment's position. Prettier's
two-pass walk shows there is no stable prettier reading of the authored position at
all: only tsv's preserved form and prettier's final glued form are fixed points, and
reaching the latter erases the authoring in two moves.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
