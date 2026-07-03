# debug_comma_comment_prettier_divergence

Prettier strips comments from `{@debug}` expressions — including one between a
comma and the next identifier. tsv preserves it.

tsv: `{@debug a, /* c */ b}` (preserved)
Prettier: `{@debug a, b}` (comment stripped)

## Reason

Content preservation — the same class as
[debug_comment](../debug_comment_prettier_divergence/), exercised at the
inter-item (post-comma) position.

See [conformance_prettier.md §Svelte: Elements](../../../../../../docs/conformance_prettier.md#svelte-elements)
(the `@debug comments` catalog entry).

## Related

- [debug_comment](../debug_comment_prettier_divergence/) — the leading-comment case
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same pattern across all template expressions
