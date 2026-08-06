# sparse_hole_after_comma_comment_prettier_divergence

The [end_of_line_block_comment](../end_of_line_block_comment_prettier_divergence/) rule with a
hole between the comment and the element it leads. Prettier moves a block comment written after
an array element's comma *back across that comma*, from leading the next element to trailing the
previous one — `[x, /* c */ , y]` becomes `[x /* c */, , y]`, flipping the binding from `y` to
`x`. Prettier classifies on newlines alone (`endOfLine`), so neither comma plays a part.

We keep the comment after the element's comma, where it was written, leading `y`.

An elision is a **slot**, not an element: it prints nothing, and its comma is re-emitted
structure outside every element span — the same status the sibling README gives a comma the
author pushed onto its own line. So the comment slides *forward* past those anonymous commas to
the element it leads (`unformatted_ours_authored`), which is lossless and leaves the binding
alone. Sliding it *backward* past `x`'s comma is what changes what the comment is about, and
that is the move we refuse — the direction, not the distance, is the rule.

Both positions are dual-stable: `[x, , /* c */ y]` and `[x /* c */, , y]` are each idempotent
under both formatters (`variant_before_comma`). The divergence is in normalization — prettier
normalizes the authored form to before the element's comma, we normalize it to after.

Order survives the slide. Two comments in the gap come back in source order (`b`), including the
mixed authoring `[x⏎/* c1 */, /* c2 */ , y]`, where prettier's per-comment classification splits
the pair across the comma and returns it **reversed** (`[x /* c2 */, , /* c1 */ y]`).

The trailing-hole face of the same rule — no next element to lead, so the run slides to the
array's own trailing position — is
[sparse_trailing_hole_comment](../sparse_trailing_hole_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`Array element end-of-line block comment`).
