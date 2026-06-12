# as_satisfies_value_line_comment_prettier_divergence

A line comment after an `as`/`satisfies` cast keyword, before the cast type
(`x as // c\n\tA`).

**tsv**: keeps the comment after the keyword, with the type on the next line:

```
const a = x as // c
	A;
```

**Prettier**: floats the comment out past the whole expression, to a
statement-trailing position:

```
const a = x as A; // c
```

Per Comment Position Philosophy: the user wrote the comment after the cast
keyword, so tsv keeps it associated with the cast rather than floating it past
the type and the statement. Both forms are idempotent in their respective
formatters.

Previously tsv emitted the comment inline and **swallowed the cast type**
(`x as // c A` — `A` absorbed into the comment, a non-idempotent content loss);
keeping it on the keyword line via `line_suffix` with the type indented on the
next line fixes the loss and preserves the user's placement. A same-line block
comment (`x as /* c */ A`) stays inline in both formatters and is not a
divergence (see the regular
[as_satisfies_keyword_comment](../as_satisfies_keyword_comment/) fixture).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
