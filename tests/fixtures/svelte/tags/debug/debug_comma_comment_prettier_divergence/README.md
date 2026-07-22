# debug_comma_comment_prettier_divergence

Prettier strips comments from `{@debug}` expressions — including one adjacent to
the comma separating two identifiers. tsv preserves it, on **either side** of the
comma:

- After the comma (leads the next identifier): `{@debug a, /* c */ b}`
- Before the comma (trails the previous identifier): `{@debug x /* t */, y}`

Prettier: `{@debug a, b}` / `{@debug x, y}` (comments stripped)

## Reason

Content preservation — the same class as
[debug_comment](../debug_comment_prettier_divergence/), exercised at the
inter-item positions on both sides of the comma. A comment before the comma
trails the previous identifier; a comment after the comma leads the next one.

See [conformance_prettier.md §Svelte: Elements](../../../../../../docs/conformance_prettier.md#svelte-elements)
(the `@debug comments` catalog entry).

## Related

- [debug_comment](../debug_comment_prettier_divergence/) — the leading-comment case
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same pattern across all template expressions
