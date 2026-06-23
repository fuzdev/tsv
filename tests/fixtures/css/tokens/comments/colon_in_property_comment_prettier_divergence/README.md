# colon_in_property_comment_prettier_divergence

A block comment between a property name and the `:` may itself contain a colon
(`color /* x:y */ : red`). The declaration's real `property : value` colon is the
one *outside* the comment — the scan must skip comment contents to find it.

tsv: `color /* x:y */ : red;` (normalized single spaces; the comment, including
its inner colon, is preserved)
Prettier: `color/* x:y */ : red;` (no space before the comment)

This is the same spacing divergence as
[in_property_value_before_colon](../in_property_value_before_colon_prettier_divergence/);
the colon inside the comment is the added case — it must not be mistaken for the
declaration's `:` (doing so mangled the output to `color /* x : red;`, dropping
`y */`).

## Reason

tsv normalizes comment spacing consistently, and locates the declaration colon
with a comment-skipping scan. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §CSS: Comments.
