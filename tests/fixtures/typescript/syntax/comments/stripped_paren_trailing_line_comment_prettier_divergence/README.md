# Stripped-paren trailing line comment: convergence, flushed at the list's comma

A `//` between an operand and the `)` of a grouping pair the printer strips
(`x + (y * z // c⏎)`, `a instanceof (b // c⏎)`, the redundant outer layer of
`x - ((y - z) // c⏎)`), and the run a sequence's own pair floats out past its `)`
(`a + (b, c // c⏎)`, a conditional branch's `(c, d // c⏎)`), with the operand an element of
a parameter or argument list. The pair is gone from the output, so the run defers to
wherever the enclosing line ends. Both formatters reach the **same fixed point** — `input` —
and differ on the **first** pass.

- **tsv**: one pass. The list breaks, and the comment flushes at the element's comma —
  behind the operand, before the next element — which is where the reparse finds it. The
  operand itself stays flat.
- **prettier**: attaches the comment to the inner operand, so its `breakParent` also
  splits the operand's own chain (`x +⏎\t\ty * z, // c1`); its second pass rejoins it
  (`prettier_intermediate_inside_parens`).

`unformatted_ours_inside_parens` carries the authoring. The last cell is the null control:
outside a list the comment trails the statement, in both formatters' fixed points.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
