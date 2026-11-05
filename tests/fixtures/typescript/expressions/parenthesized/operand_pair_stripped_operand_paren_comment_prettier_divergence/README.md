# Operand pair, comment in a stripped operand pair

The operand twin of
[as_operand_shell_stripped_operand_paren_comment](../as_operand_shell_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of an operand whose
pair the printer emits — a unary operator's (`-`, `!`, `typeof`, `void`), `await`'s, `yield`'s,
`export =`'s, a binary chain operand's (its pair required or not), a spread argument's — stays
after the operand, ahead of the block and the comment written after the inner pair, which follow
on their own line (`-(a + (b // c⏎) /* x */ // d⏎)` prints `-(⏎\ta + b // c⏎\t/* x */ // d⏎)`). A
binary chain operand keeps the run ahead of its operator, which drops below it, and a spread
argument keeps the comment after its pair, ahead of the run written after the inner pair. Nested
pairs, an own-line `//` in the inner pair and a block alone after the pair — at a unary operand, a
binary chain's operand and a spread's argument — take the same order.

tsv: `-(⏎\ta + b // c⏎\t/* x */ // d⏎)`, the comments in the order written — the form tsv gives the
bare spelling (`-(a + b // c⏎/* x */ // d⏎)`)
Prettier: from the input, moves the run past the `;` at `await`, `yield` and `export =`, and
past the operator of a binary chain (`(a + b) * // c`, `output_prettier`); from the stripped-pair
spelling it puts the comments on one line, the block ahead of the `//`s, where the two `//` weld
into one comment (`a + b /* x */ // c // d`, `audit_signature_shell`); with a block alone after the
pair it puts the block ahead of the `//` (`k = a + b /* x */ && q; // c`)

A `//` alone after the pair already follows on its own line, and with nothing written after the
pair the comment trails the statement (a spread's stays after its element, `...(a + b) // c`), a
unary operand's pair holding a leading comment included — those are controls.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
