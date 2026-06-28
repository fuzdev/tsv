# new_open_paren_comment_prettier_divergence

A comment trailing a `new` expression's opening `(` on the same line
(e.g. `new Foo( // c` or `new Foo( /* paren */`) is kept on the `(` line where
the author placed it. Prettier drops it onto its **own line** inside the parens,
as the first argument's leading comment.

## Reason

Same rule as the simple-callee and member-chain forms
([open_paren_comment](../open_paren_comment_prettier_divergence/),
[chain_open_paren_comment](../chain_open_paren_comment_prettier_divergence/)),
applied to the `new` path. Keeping the comment on the `(` line also avoids
content loss — a line comment trailing `(` must be preserved, not dropped. When
the author writes the comment on its own line, both formatters keep it there —
the two positions are dual-stable.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation (Call open paren `(`).
