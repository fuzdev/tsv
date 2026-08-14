# open_paren_comment_prettier_divergence

A comment after a do-while condition's open `(` (`} while (// c⏎x⏎);`) is kept
after the `(`. Prettier moves it **out of the parens**, and where it lands depends
on the kind: a `//` goes after the terminating `;` (`} while (x); // c`), a block
comment ahead of the `while` keyword (`} /* c */ while (x);`). This relocation is
unique to do-while — other constructs (if, while, for, switch) keep the comment
inside the parens.

The divergence is specific to the form where the condition does **not** follow the
comment on the `(` line. A block-comment run the condition does follow stays inside
the parens in both formatters, which is what gives that shape its spacing oracle —
nothing after the `(`, one space per gap
([open_paren_comment_run](../open_paren_comment_run/)).

Here, where prettier relocates and leaves no spacing oracle, tsv writes the space it
writes at every other opening delimiter it keeps a comment on (`fn( // c`,
`new Foo( /* paren */`, `[ // c`, `{ // c`). The author's own spacing is normalized in
both forms — it is never preserved.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([line_before_while_comment](../line_before_while_comment_prettier_divergence/),
[while_leading_block_comment](../while_leading_block_comment_prettier_divergence/)),
and with if/else, try/catch, switch, for, while, labeled statements, and call
chains.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
