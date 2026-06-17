# Divergence: line comments between a `for` header `)` and the body

Line comments the author wrote after a non-empty `for (…)` header's closing `)`,
before the body (`for (…) // a\n// b\n{`).

Prettier **relocates** the first comment back *inside* the parens to trail the
last clause (`i++ // a`), keeping only the rest after `)`; tsv preserves every
comment where the author placed them — after `)`.

```ts
// prettier (relocates into the parens)   // tsv (preserves placement)
for (                                      for (
	let i = 0;                                 let i = 0;
	i < n;                                     i < n;
	i++ // a                                   i++
) // b                                     ) // a
{                                          // b
	x();                                   {
}                                              x();
                                           }
```

A line comment after `)` runs to end-of-line, so the comments drop to their own
lines (the body `{` follows on its own line) — without that break the `//` would
swallow the following comment and the body (content loss). tsv treats comment
placement as intentional and never relocates across the `)` boundary, consistent
with its handling across if/else, try/catch, switch, for, while, do-while — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy. **Empty** `for (;;)` headers
with comments after `)` are instead a plain match (prettier has no clause to
relocate into) — [empty_clauses_body_comment](../empty_clauses_body_comment/).
