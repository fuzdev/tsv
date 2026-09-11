# comment_after_colon_prettier_divergence

Prettier moves single line comments from after `:` to trailing on the true branch.
For example, `? foo : // about bar\nbar` becomes `? foo // about bar\n: bar`,
changing the comment's semantic association from the false branch to the true branch.

We preserve the comment's position to maintain user intent — the comment was
written about `bar`, not `foo`.

Both positions are dual-stable: `? foo // c\n: bar` and `? foo\n: // c\n  bar`
are each idempotent under both formatters. The divergence is in normalization —
prettier normalizes the compact form (`? foo : // c\nbar`) to trailing, while
we normalize it to after `:`.

An own-line comment or run after `:` keeps its own line under tsv, where prettier pulls the
first comment up onto the `:` line —
[branch_own_line_line_comment](../branch_own_line_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
