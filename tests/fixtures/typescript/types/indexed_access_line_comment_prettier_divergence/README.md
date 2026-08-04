# indexed_access_line_comment_prettier_divergence

A line comment in an indexed access type's `[`→index gap (`A[ // c⏎K]`). tsv
keeps the comment where the author wrote it and drops the index type to the next
line:

```
type I =
	A[// c
	K];
```

**Prettier** relocates the comment out past the access to a statement-trailing
position (`type I = A[K]; // c`).

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the index
type — non-idempotent content loss; the line comment now forces the break (the
shared `build_trailing_comments_hang_next`).

The neighbouring object→`[` gap (`A // c⏎[K]`) has no counterpart fixture because
the shape does not exist: a type's index suffix may not follow a line break, so
both parsers read `A⏎[K]` as two statements (`type X = A;` plus an
`ArrayExpression`), and a `//` there forces exactly that break. That gap can hold
only a single-line block (`A /* c */[K]`), which stays glued.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
