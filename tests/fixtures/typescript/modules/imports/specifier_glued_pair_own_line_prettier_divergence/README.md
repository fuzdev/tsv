# specifier_glued_pair_own_line_prettier_divergence

The pair form of
[specifier_end_of_line_block_comment](../specifier_end_of_line_block_comment_prettier_divergence/),
and the one where prettier's relocation stops being position-only.

A run of block comments the author **glued** to each other and gave a line of its own
(`{ a1,⏎/* c1 */ /* c2 */⏎b1 }`) is not an own-line comment: only `c1` has a newline
before it and only `c2` has one after, so prettier's own `printLeadingComment` gives the
run a soft `line` and the list still flattens when it fits. Both formatters agree on the
flat form (`input`), which is what makes the authored spelling a *normalization* question
rather than a layout one.

They normalize it differently, and prettier's answer **reorders the pair**: it splits the
run across the specifier's comma, sending `c2` back to trail `a1` and leaving `c1` to lead
`b1` (`variant_prettier_split`, dual-stable). The authored order `c1 c2` comes out `c2 c1`
— so unlike the single-comment case, which moves a comment without changing what it says,
this one loses the relation between the two. tsv keeps the run whole and in source order,
after the comma, leading `b1` (`unformatted_ours_own_line`).

That is the same rule tsv applies at every other list — the author's line break *around*
the comma is layout, not own-line-ness, so it collapses with the rest of the layout — and
here it is also the only rendering that preserves what the author wrote.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
