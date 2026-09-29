# Stripped operand paren, line comment, in a template island's argument list

The template twin of
[stripped_paren_trailing_line_comment](../../../typescript/syntax/comments/stripped_paren_trailing_line_comment_prettier_divergence/):
a `//` inside a grouping pair the printer strips, in an operand that is an argument — of a
`{@render}` call, of a call in an expression tag, of a call in an attribute value.

- **tsv**: one pass. The argument list breaks, and the comment flushes at the argument's
  comma or before the list's `)`, inside the expression, where the reparse finds it.
- **prettier**: its first pass also splits the operand's own chain (`x +⏎\t\ty * z // c1`);
  its second pass rejoins it (`prettier_intermediate_inside_parens`).

See
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
