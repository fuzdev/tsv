# catch_param_multiline_block_prettier_divergence

A multiline block comment glued to a `catch` parameter. tsv opens the parens — the
comment cannot fit on the line, and tsv's catch parens are an ordinary width-driven
group. Prettier keeps them flat at any width, because for this shape it emits no
break point at all.

## Reason

The other face of [catch_param_comment_break](../catch_param_comment_break_prettier_divergence/):
prettier's `printCatchClause` prints the parens **ungrouped**, and picks the form from
`parameterHasComments` rather than from the width. A block comment glued to the
parameter fails that predicate (it is leading, with no newline after it), so prettier
emits `["(", param, ") "]` — a shape with nowhere to break — and the comment's own
newlines run past printWidth unchallenged.

tsv prints all five paren-headed constructs through the one width-driven condition
group, so this comment breaks the parens exactly as it does in `if`, `while` and
`switch`, where prettier breaks them too. Catch is the odd one out in prettier, not
in tsv.

This is the direction where the two have genuinely different fixed points, so it is
pinned by `output_prettier.svelte` rather than a variant: prettier holds the flat form
and rewrites tsv's, tsv holds the broken form and rewrites prettier's.

See [conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)
for the principle and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
for the catalog entry.
