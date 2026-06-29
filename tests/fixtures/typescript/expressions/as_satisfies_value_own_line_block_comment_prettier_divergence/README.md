# as_satisfies_value_own_line_block_comment_prettier_divergence

A block comment in an `as`/`satisfies` cast's keyword→type gap, with the cast
type authored on a **later line** than the comment. tsv keeps the type on its
own line and the comment where the author wrote it; Prettier relocates the
comment.

**Own-line** comment (`x as⏎/* c */⏎A`) — tsv keeps it on its own line, type on
the next line:

```
const a = x as
	/* a */
	A;
```

Prettier pulls it up onto the keyword line (`x as /* c */⏎A`).

**Same-line** comment, type on the next line (`x as /* c */⏎A`) — tsv keeps the
comment trailing the keyword and the type on its own line:

```
const d = x as /* d */
	D;
```

Prettier moves the comment *before* the keyword and collapses the type back up
(`x /* d */ as D`).

Per Comment Position Philosophy: the user put the type on its own line, so tsv
preserves that break and keeps the comment associated with the cast where it was
written. An author blank line after an own-line comment is preserved. Both forms
are idempotent in their respective formatters.

A block comment with the type **glued to it** (`x as /* e */ E`, all on one
line) stays fully inline in both formatters and is **not** a divergence (case
`e`); the **line**-comment case floats out separately
([as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/)).
Emitting an own-line/same-line-break comment inline (the previous behavior)
reflowed the author's break and glued the type onto the comment's line.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
