# specifier_end_of_line_block_comment_prettier_divergence

The array literal's
[end_of_line_block_comment](../../../expressions/arrays/end_of_line_block_comment_prettier_divergence/)
rule at the import/export specifier list. Prettier moves a block comment that ends a list
item's line *across the item's comma*, from leading the next specifier to trailing the
previous one (`{ a1, /* c */⏎b1 }` becomes `{ a1 /* c */, b1 }`), classifying on newlines
alone (`endOfLine`) so the comma — which is what carries the association — plays no part.
tsv keeps the comment after the comma, leading `b1`.

Both positions are dual-stable (`variant_before_comma`), so the divergence is in
normalization: prettier normalizes the newline-after form to before the comma while we
normalize it to after it (`unformatted_ours_newline_after`).

The comma pushed onto its own line with the comment (`{ a1⏎, /* c */⏎b1 }`) is the same
authoring one notch further — the comma is re-emitted structure, outside every item span,
so the comment still sits after it — and it takes the same normalization
(`unformatted_ours_comma_own_line`). The author's line break around the comma is not
own-line-ness: the list flattens when it fits, so the break collapses with the rest of its
layout, exactly as the same authoring does in an array literal, a tuple and a
type-parameter list. A comment the author gave a line **of its own** still takes one and
expands the list — [specifier_own_line_block_comment_expand](../specifier_own_line_block_comment_expand/).

A comment written *before* the comma trails the previous specifier in both formatters
(`a3`), and a line comment expanding the list does not move the glued comment off its
specifier (`b4`).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
