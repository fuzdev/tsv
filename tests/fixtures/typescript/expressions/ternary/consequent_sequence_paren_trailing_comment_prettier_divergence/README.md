# consequent_sequence_paren_trailing_comment_prettier_divergence

A block comment inside the grouping shell the parser erased from a ternary
**consequent** sequence's last operand (`cond ? (x, (y /* t */)) : z`).

Both formatters settle on the same form — the comment behind the pair, `(x, y) /* t */`
— and part on the way there: **tsv** reaches it in one pass, **prettier** takes two, its
first pass holding the comment inside the pair.

## Reason

A consequent's own paren-comment scan is bounded at the consequent's span end, because
the comment between the consequent and the `:` is emitted by the ternary's own gap
printer and a wider boundary would print it twice. That bound leaves no room for a
grouping `)`, which is the tell that the region behind the operands is the **sequence's
own tail** rather than an enclosing shell's gap — so the sequence's doc prints it, after
the pair it supplies.

Reading it as a shell's gap instead hands the comment to the pair on pass 1 and to the
consequent→`:` emitter on pass 2, which is prettier's chain here, not a fixed point. The
sibling branch parts on exactly this: an **alternate**'s gap runs to the statement `;`,
so the shell's `)` IS in it, the operand's pair stands in for that shell, and the comment
stays inside — [alternate_paren_trailing_comment](../alternate_paren_trailing_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
