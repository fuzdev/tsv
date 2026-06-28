# in_of_close_paren_comment_prettier_divergence

A comment in the gap between a `for-in`/`for-of` header's closing `)` and the
(non-block) body.

A **block** comment stays inline after `)` in both formatters (`for (x in obj)
/* a */ expr;`) — no divergence. A **line** comment the author wrote trailing
`)` (`for (x in obj) // a`) diverges: prettier drops it to its own line, indented
with the body; tsv preserves it trailing `)`.

## Reason

tsv treats the author's same-line placement after `)` as intentional, consistent
with its handling across if/else, try/catch, switch, for, while, do-while.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
