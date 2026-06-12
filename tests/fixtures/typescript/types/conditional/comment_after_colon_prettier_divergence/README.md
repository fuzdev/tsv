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

Note: when multiple line comments appear on their own lines after `:`, prettier
also keeps them after `:`. The divergence only affects single-line cases.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
