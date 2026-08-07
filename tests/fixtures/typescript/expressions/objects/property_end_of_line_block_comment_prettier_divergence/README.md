# property_end_of_line_block_comment_prettier_divergence

The array literal's
[end_of_line_block_comment](../../arrays/end_of_line_block_comment_prettier_divergence/)
rule at the object literal. Prettier moves a block comment that ends a property's line
*across the property's comma*, from leading the next property to trailing the previous one
(`{ property1: 1, /* c */⏎property2: 2 }` becomes `{ property1: 1 /* c */, property2: 2 }`),
classifying on newlines alone (`endOfLine`) so the comma — which is what carries the
association — plays no part. tsv keeps the comment after the comma, leading `property2`.

Both positions are dual-stable (`variant_before_comma`), so the divergence is in
normalization: prettier normalizes the newline-after form to before the comma while we
normalize it to after it (`unformatted_ours_newline_after`). That variant is also what
prettier's output of each `unformatted_ours_*` is pinned against.

The comma pushed onto its own line with the comment (`{ property1: 1⏎, /* c */⏎property2: 2 }`)
is the same authoring one notch further — the comma is re-emitted structure, outside every
property span, so the comment still sits after it — and it takes the same normalization
(`unformatted_ours_comma_own_line`). The author's line break around the comma is not
own-line-ness: this object flattens when it fits, so the break collapses with the rest of its
layout. A comment the author gave a line **of its own** still takes one and expands the object
(`c`, and [property_own_line_comma_comment](../property_own_line_comma_comment/)).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
