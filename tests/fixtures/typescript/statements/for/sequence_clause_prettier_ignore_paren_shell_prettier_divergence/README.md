# sequence_clause_prettier_ignore_paren_shell_prettier_divergence

An own-line `prettier-ignore` written inside the grouping shell the parser **erases**
around a `for` header sequence clause's operand — at the **leading edge** (ahead of
the first operand) and in a **frozen** last operand's own trailing gap.

Both edges lie inside the sequence's span, which opens at that erased `(` and closes
at the last operand's erased `)`, so no enclosing gap can see them and the clause
itself has to answer.

**Leading edge.** tsv freezes THAT operand — Rule A's child scope — keeps the
directive on the line the author gave it, and supplies the `[~In]` pair the header
needs outside the slice (`('aaa'  in  bbb)`, and nothing for an operand with no `in`
under it). The operands stay on one line, which is where the reparse reads the
directive: ahead of the whole clause, leading the sequence node.
**Prettier** drops every operand after the first onto a continuation line
(`('aaa'  in  bbb),⏎\tg;`) and keeps that break, so the authored form normalizes to
`input.svelte` for tsv only, and prettier's own stable form is a third one tsv
rewrites (`divergent_variant_paren_shell` — reading the directive from the clause's
leading gap, tsv freezes the whole clause there and holds prettier's break verbatim).

**Frozen operand's trailing gap.** The freeze does not take the operand out of its
shell: the slice is the operand's own node span, so the gap between it and the erased
`)` is still the shell's, answered exactly as the unfrozen form answers it — a block
strips inline outside the pair (`('aaa'  in  bbb) /* t1 */`, matching prettier), and
a line comment RETAINS the shell, where prettier floats it past the header's `;`
(the sibling divergence
[sequence_clause_paren_line_comment](../sequence_clause_paren_line_comment_prettier_divergence/),
whose README carries the separator-vs-terminator reason).

Everything *inside* the slice is the slice's, including the shell a nested sequence
operand erased from its own last operand: the gap the shell reads opens where the slice
stops printing, never at a node position inside it, so the comment there prints once,
from the verbatim source (`(bbb,  (ccc /* t3 */))`, matching prettier).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy,
[conformance_prettier_ignore.md](../../../../../../docs/conformance_prettier_ignore.md) §On delimiter-owned value heads, and on sequence operands
and its entry for a directive inside the grouping parens the parser erases ahead of a
construct's leftmost operand
([left_spine_paren_prettier_ignore_interior](../../../expressions/left_spine_paren_prettier_ignore_interior_prettier_divergence/)).
