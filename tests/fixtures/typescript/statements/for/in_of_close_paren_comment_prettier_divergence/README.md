# in_of_close_paren_comment_prettier_divergence

A comment in the gap between a `for-in`/`for-of` header's closing `)` and the
(non-block) body.

A **block** comment stays inline after `)` in both formatters (`for (x in obj)
/* a */ expr;`) — no divergence. A **line** comment the author wrote trailing
`)` (`for (x in obj) // a`) diverges under prettier 3.9: prettier drops it to its
own line, indented with the body; tsv preserves it trailing `)`.

```ts
// prettier 3.9 (own line)   // tsv (trailing `)`)
for (x in obj)               for (x in obj) // a
	// a                         expr;
	expr;
```

## Reason

tsv treats the author's same-line placement after `)` as intentional, consistent
with its handling across if/else, try/catch, switch, for, while, do-while. (Under
prettier 3.8 this was a plain match; prettier 3.9 began relocating the line
comment to its own line.)

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
