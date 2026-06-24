# Computed key bracket comment with paren in comment

Edge case of the computed key bracket comment divergence where the comment
contains a `(` character (e.g., `/* a(b) */`). Tests that the source scanner
correctly skips delimiters inside comments when finding the `(` for method params.

Same divergence as [computed_key_bracket_comment](../computed_key_bracket_comment_prettier_divergence/)
— prettier moves comments inside brackets; tsv preserves between `]` and the next
token. The comment's `(` must not be mistaken for the method's `(` when locating
the param list (a delimiter-scan robustness case). Both positions are dual-stable.

## Reason

Comment relocation (comment position) — see [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
