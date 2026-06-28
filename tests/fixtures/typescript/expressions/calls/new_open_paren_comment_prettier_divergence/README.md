# new_open_paren_comment_prettier_divergence

A comment trailing a `new` expression's opening `(` on the same line
(e.g. `new Foo( // c` or `new Foo( /* paren */`) is preserved on the `(` line.
Prettier 3.9 drops it onto its **own line** inside the parens, as the first
argument's leading comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier 3.9: relocates the comment to its own line inside the parens

```
// tsv               // prettier 3.9
new Foo( // c1       new Foo(
	a                    // c1
);                       a
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

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation (Call open paren `(`).
