# Non-null operand, own-line comments before the `!`

The own-line spelling of
[operand_line_comment](../operand_line_comment_prettier_divergence/), at the
same gap: comments that hold their own line between the operand and the `)!` —
the block half of a mixed run `// c⏎/* d */` (a block following a `//` is
always own-line, since the `//` owns the rest of its line), and a lone
own-line `//`, whose redundant parens are retained for its sake.

- **tsv**: keeps each comment inside the parens on its own line below the
  operand, in authored order — an own-line comment in this gap trails the
  operand, it never leads it.
- **prettier**: strips the parens where it can and defers the `//` past the
  statement, dropping the own-line block below the `;`.

Reason: comment preservation — order is position. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Sealed optional chain / non-null operand, own-line
comments) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
