# Non-null grouped-operand comment divergence

When a parenthesized operand of a non-null assertion carries a trailing
comment inside its **required** parens (`(x + y /* c */)!`), tsv keeps the
comment where the author wrote it — inside the parens. The parens are
required: `!` binds tighter than `+`/`? :`, so `x + y!` means `x + (y!)`.
Both formatters keep the parens; only the comment position differs.

Prettier relocates the comment **outside** the parens, between `)` and
`!` (`(x + y) /* c */!`).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Non-null grouped operand) and §Comment Position
Philosophy.
