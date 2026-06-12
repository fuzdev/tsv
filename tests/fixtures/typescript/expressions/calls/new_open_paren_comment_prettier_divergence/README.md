# new_open_paren_comment_prettier_divergence

A comment trailing a `new` expression's opening `(` on the same line
(e.g. `new Foo( // c` or `new Foo( /* c */`) is preserved on the `(` line.
Prettier moves it — a line comment floats out to a statement-trailing comment
(`new Foo(a); // c`), and a block comment is relocated before the `(`
(`new Foo /* c */(…)`).

tsv: keeps the comment trailing `(` where the user placed it
Prettier: floats the line comment past the statement / moves the block before `(`

```
// tsv                          // prettier
new Foo( // c1                 new Foo(a); // c1
	a,
);
```

## Reason

Same rule as the simple-callee and member-chain forms
([open_paren_comment](../open_paren_comment_prettier_divergence/),
[chain_open_paren_comment](../chain_open_paren_comment_prettier_divergence/)),
applied to the `new` path. This also fixes content loss: a line comment trailing
`(` in a `new` expression was previously **dropped entirely** (`new Foo( // c\n\ta)`
→ `new Foo(a)`); it is now preserved on the `(` line. When the author writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation (Call open paren `(`).
