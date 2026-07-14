# Unary conditional/arrow-operand leading-comment divergence

A conditional or arrow as a unary operand needs parens (`!(cond ? b : c)`,
`!(() => x)`). A leading block comment glued inside those parens
(`!(/* c */ cond ? b : c)`) is preserved where the author wrote it — inside the
operand's single required pair. tsv does **not** add a second, comment-holder
pair around it: the operand's own parens already enclose the comment, so one
pair suffices. A redundant double-paren authoring (`!((/* c */ cond ? b : c))`)
collapses to the single pair — see `unformatted_ours_double_paren`.

tsv: `!(/* c */ cond ? b : c)` (comment inside the single paren)
Prettier: `!(/* c */ (cond ? b : c))` (comment relocated out, before the operand's paren)

This is the same rule as the unary [assignment operand](../assignment_operand_leading_comment_prettier_divergence/):
every glued block comment binds to the operand it leads
(`Comment::owned_by_node`), and any operand that prints its own value-position
pair encloses the comment in that one pair rather than taking a second. Sequence,
assignment, conditional, and arrow operands all follow it.

A **type-assertion** operand is the non-divergent counterpart of this rule: there
prettier keeps the single pair too (`!(/* c */ x as T)`), so tsv matches — see the
regular fixture [type_assertion_operand_leading_comment](../type_assertion_operand_leading_comment/).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Unary conditional/arrow-operand leading comment) and
§Comment Position Philosophy.
