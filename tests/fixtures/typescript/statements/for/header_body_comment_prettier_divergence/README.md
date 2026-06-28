# Divergence: line comments between a `for` header `)` and the body

Line comments the author wrote after a non-empty `for (…)` header's closing `)`,
before the body (`for (…) // a\n// b\n{`).

Both formatters now keep **both** comments where the author placed them — after
`)`, on their own lines, with the body `{` following. The divergence is purely
**header layout**: a line comment in the header→body gap forces tsv to keep the
C-style header expanded (one clause per line), while prettier collapses the
header back inline (it fits) and trails the first comment after `)`.

```ts
// prettier (collapses the header)         // tsv (keeps the header expanded)
for (let i = 0; i < n; i++) // a           for (
// b                                           let i = 0;
{                                              i < n;
	x();                                       i++
}                                          ) // a
                                           // b
                                           {
                                               x();
                                           }
```

A line comment after `)` runs to end-of-line, so the comments drop to their own
lines (the body `{` follows on its own line) — without that break the `//` would
swallow the following comment and the body (content loss). tsv keeps the header
structurally open whenever a gap comment trails `)`, so the comments and body
read as a deliberately laid-out block; tsv treats comment placement as
intentional, consistent with its handling across if/else, try/catch, switch,
for, while, do-while — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy. **Empty** `for (;;)` headers
with comments after `)` are instead a plain match — [empty_clauses_body_comment](../empty_clauses_body_comment/).
