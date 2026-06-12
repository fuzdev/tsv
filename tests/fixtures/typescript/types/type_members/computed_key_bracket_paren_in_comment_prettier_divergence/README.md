# Computed key bracket comment with paren in comment

Edge case of the computed key bracket comment divergence where the comment
contains a `(` character (e.g., `/* a(b) */`). Tests that the source scanner
correctly skips delimiters inside comments when finding the `(` for method params.

Same divergence as `computed_key_bracket_comment_prettier_divergence` — prettier
moves comments inside brackets; tsv preserves between `]` and the next token.
