# type_operator_keyword_own_line_block_comment_prettier_divergence

An **own-line block** comment after a prefix type operator
(`keyof`/`typeof`/`readonly`), before the operand, with the operand authored on a
later line (`type A = keyof⏎/* c */⏎B`).

**tsv** keeps the comment where the author wrote it — on its own line after the
operator, the operator still on the `=` line — and hangs the operand on the next
(indented) line:

```
type A = keyof
	/* a */
	B;
```

**Prettier** pulls the comment up onto the operator line, operand flush
(`keyof /* a */⏎B`).

This is the own-line-block sibling of the
[line-comment form](../type_operator_keyword_line_comment_prettier_divergence/),
sharing the keyword→value layout (`append_keyword_value_line_comments`) with the
`as`/`satisfies` cast gap
([as_satisfies_value_own_line_block_comment](../../expressions/as_satisfies_value_own_line_block_comment_prettier_divergence/));
tsv preserves the author's operator-associated placement and the uniform one-level
operand hang. A blank line after the comment is preserved; a same-line comment
with the operand below (`keyof /* e */⏎B`) likewise keeps the operand hanging
(prettier collapses it inline). A **same-line** block comment glued to the operand
(`keyof /* f */ B`) stays inline in both formatters and is *not* a divergence (the
regular [type_operator_keyword_comment](../type_operator_keyword_comment/)
fixture). Covers `keyof`, `typeof`, and `readonly`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
