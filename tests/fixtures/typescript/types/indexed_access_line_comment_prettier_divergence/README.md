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
shared `build_trailing_comments_break_for_line`).

The object→`[` gap (`A // c⏎[K]`) is also fixed at the printer (the
`build_leading_comments_break_for_line` path), but isn't fixtured here: acorn
parses `A⏎[K]` (a newline before the `[`) as two statements via ASI where tsv
keeps the single `TSIndexedAccessType`, a pre-existing parser divergence the break
would otherwise entangle with. The swallow itself is guarded by `swallow_audit`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
