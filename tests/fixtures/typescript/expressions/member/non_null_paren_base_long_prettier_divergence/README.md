# non_null_paren_base_long_prettier_divergence

Prettier lays out a parenthesized base whose inner call breaks two different ways depending on whether a non-null assertion follows it.

tsv: hangs the outer parens whether or not `!` follows (`(\n\tawait call(...)\n)!.member`)
Prettier: hangs the parens without `!`, but hugs the inner call with `!` (`(await call(\n...\n))!.member`)

| Form                          | tsv                | Prettier           |
| ----------------------------- | ------------------ | ------------------ |
| `(await call(...)).member`    | hangs outer parens | hangs outer parens |
| `(await call(...))!.member`   | hangs outer parens | hugs inner call    |

## Reason

tsv lays out a parenthesized base the same way regardless of a trailing non-null assertion, so the two forms stay visually consistent. Content is identical (the ASTs match); only the parenthesized-base layout differs, and only in the `!` form.

## Related

- `member/paren_base_trailing_long/` — the no-`!` form, where tsv matches Prettier (single trailing member hugs the closing `)`).

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §TypeScript.
