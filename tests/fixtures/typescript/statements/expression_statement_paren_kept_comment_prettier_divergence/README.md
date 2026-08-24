# expression_statement_paren_kept_comment_prettier_divergence

A comment after the `(` of an expression statement whose parens are **required** —
an object / class / function expression at statement start, or a bare string that
would otherwise read as a directive. The comment breaks the parens open and keeps
its place inside them.

tsv: keeps the comment inside the parens the author wrote it in
Prettier: hoists it out onto its own line before `(`

## Reason

The comment sits after the opening `(`, so tsv keeps it there rather than hoisting
it before the parens (Comment Position Philosophy). Both spellings behave alike —
a line comment, an own-line block, and a block glued to the `(` all take their own
line inside the broken parens; a block glued to the *expression* is owned by it and
stays inline (`(/* c */ { a: 1 })`, not a divergence).

Collapsing the parens inline would **drop** the paren-glued block (content loss): the
comment has no place to go there and nothing else prints it.

The [decorated class expression](../../expressions/class/decorated_expr_open_paren_comment_prettier_divergence/)
is the same divergence on its own layout path; when the paren is *redundant* tsv
drops it and the comment leads the statement, matching prettier
([expression_statement_paren_comment](../expression_statement_paren_comment/)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
