# Non-null grouped-operand-then-member comment divergence

When a parenthesized operand of a non-null assertion that is **followed by a
member access** carries a trailing comment inside its required parens
(`(x + y /* c */)!.foo`), tsv keeps the comment where the author wrote it —
inside the parens. The parens are required (`!` binds tighter than `+`/`? :`),
and the trailing `.foo` routes the whole thing through the member-chain
printer. Both formatters keep the parens; only the comment position differs.

Prettier relocates the comment **outside** the parens, between `)` and `!`
(`(x + y) /* c */!.foo`).

This is the member-followed sibling of
[non_null/grouped_operand_comment](../grouped_operand_comment_prettier_divergence/)
(the bare `(x + y /* c */)!`, which goes through the non-chain path).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Non-null grouped operand) and §Comment Position
Philosophy.
