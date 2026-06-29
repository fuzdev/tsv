# keyword_line_comment_prettier_divergence

A line comment after a mapped type's `in` keyword (`[K in // c⏎T]`) or `as`
keyword (`[K in T as // c⏎N]`). tsv keeps the comment after the keyword and drops
the following type to the next line:

```
type M = {
	[K in // c
	T]: V;
};
```

**Prettier** expands the `[…]` and relocates the comment to *after* the whole
constraint / `as` clause (`[⏎K in T // c⏎]`).

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the following
type — non-idempotent content loss; the line comment now forces the break (the
shared `build_trailing_comments_break_for_line`).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
