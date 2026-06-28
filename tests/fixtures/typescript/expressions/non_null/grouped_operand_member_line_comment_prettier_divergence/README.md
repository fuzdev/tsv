# Non-null grouped-operand-then-member line-comment divergence

The line-comment sibling of
[grouped_operand_member_comment](../grouped_operand_member_comment_prettier_divergence/).
When a parenthesized operand of a non-null assertion followed by a member
access carries a **line** comment inside its required parens, tsv keeps the
comment where the author wrote it — inside the parens, forcing the multiline
paren layout (the same shape tsv uses for a unary line-comment operand,
`!(\n\tx + y // c\n)`).

Prettier relocates the comment outside, after `)!`, and breaks the chain
differently (`(x +\n\ty)! // c\n.foo`).

A line comment can't trail inline before `)` (the `//` would swallow the
`)`), so unlike the block-comment case (which stays inline,
`(x + y /* c */)!.foo`) it forces the operand onto its own line.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Non-null grouped operand) and §Comment Position
Philosophy.
