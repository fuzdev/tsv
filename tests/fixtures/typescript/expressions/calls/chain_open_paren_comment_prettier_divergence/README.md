# chain_open_paren_comment_prettier_divergence

A comment trailing a member-chain call's opening `(` on the same line
(e.g. `obj.method( // c` or `obj.a().method( /* c */`) is preserved on the `(`
line. Prettier relocates it to its own line as the first argument's leading
comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
obj.method( // c1              obj.method(
	a                                  // c1
);                                a
                                );
```

## Reason

Same rule as the simple-callee form
([open_paren_comment](../open_paren_comment_prettier_divergence/)), applied to
the member-chain call path: a comment the author parked after `(` is a trailing
comment on that line, so tsv preserves it in place rather than relocating it.
This also fixes a source-order bug — when a block comment trails `(` and an
own-line comment leads the first arg (`obj.method( /* paren */\n\t// lead\n\ta`),
tsv previously reversed them; it now keeps `/* paren */` on the `(` line and
`// lead` on its own line, in source order. When the author writes the comment
on its own line, both formatters keep it there — the two positions are
dual-stable.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation (Call open paren `(`).
