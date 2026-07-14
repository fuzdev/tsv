# Unary assignment-operand leading-comment divergence

An assignment as a unary operand needs parens (`!(x = y)`). A leading block
comment glued inside those parens (`!(/* c */ x = y)`) is preserved where the
author wrote it — inside the operand's single required pair. tsv does **not**
add a second, comment-holder pair around it: the operand's own parens already
enclose the comment, so one pair suffices (the same rule a sequence operand
follows, `!(/** @type {A} */ (x), y)`). A redundant double-paren authoring
(`!((/* c */ x = y))`) collapses to the single pair — see
`unformatted_ours_double_paren`.

tsv: `!(/* c */ x = y)` (comment inside the single paren)
Prettier: `!(/* c */ (x = y))` (comment relocated out, before the operand's paren)

Every glued block comment binds to the operand it leads
(`Comment::owned_by_node`), so the comment stays glued rather than hoisting
across the paren boundary — the unary-operand counterpart of the ternary and
grouped-operand leading-comment cases.

A **trailing** comment on the same operand is a separate, non-divergent case:
`!((x = y) /* c */)` keeps both pairs in both formatters — see the regular
fixture [operand_paren_comment](../operand_paren_comment/).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Ternary operand leading comment) and §Comment Position Philosophy.
