# debug_comment_prettier_divergence

Prettier strips comments from `{@debug}` expressions. tsv preserves them.

tsv: `{@debug /* expected: 5 */ a}` (preserved)
Prettier: `{@debug a}` (comment stripped)

## Reason

Comments in debug statements often contain important context (expected values, why the variable is being debugged). Stripping them loses developer intent.

## Related

- [expr_trailing](../../syntax/comments/expr_trailing_prettier_divergence/) — same pattern across all template expressions
