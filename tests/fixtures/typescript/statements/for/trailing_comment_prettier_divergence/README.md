# trailing_comment_prettier_divergence

A line comment after a C-style `for (…)` header's closing `)`, before the body
(`for (…) // comment\n{`).

Both formatters keep the comment after `)`. The divergence is **header layout**:
the gap line comment forces tsv to keep the C-style header expanded (one clause
per line), while prettier collapses the header back inline (it fits) and trails
the comment after `)`.

```ts
// prettier (collapses the header)    // tsv (keeps the header expanded)
for (i = 0; i < 10; i++) // comment   for (
{                                         i = 0;
	a();                                  i < 10;
}                                         i++
                                      ) // comment
                                      {
                                          a();
                                      }
```

## Reason

A line comment after `)` runs to end-of-line, so the body `{` follows on its own
line in both formatters. tsv keeps the header structurally open whenever a gap
comment trails `)`, treating the layout as intentional — consistent with tsv's
handling across if/else, try/catch, switch, for, while, do-while.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
