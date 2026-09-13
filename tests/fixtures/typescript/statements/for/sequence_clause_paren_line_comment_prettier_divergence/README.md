# sequence_clause_paren_line_comment_prettier_divergence

A line comment — or an own-line block — in the trailing gap of a **`for`-header
sequence clause's LAST operand's** grouping shell (`for (a, (b // c); ;)`).

**tsv** retains the shell and keeps the comment inside it, on the operand's line.
**Prettier** strips the shell and floats the comment past the header's first `;`.

## Reason

The `;` a `for` header's clause meets is a **clause separator**, not a statement
terminator, so the shell is not terminator-adjacent and the comment has nowhere to
defer to. Deferring it anyway carries it out of the construct it was written in —
which is what prettier's own output does on its **second** pass: the run's second
comment (`// c3`) and the own-line block (`/* c4 */`) leave the header entirely and
land between the header's `)` and the body `{`, pinned in `audit_signature.txt`.

The last operand is the one that needs saying: every earlier operand's shell gap is
claimed by the comma that follows it, while the last operand's runs to the clause's
own end. tsv answers it with the same shell builder every other `for`-clause
position uses, so the sequence clause and the header declarator give one answer
([init_paren_line_comment](../init_paren_line_comment_prettier_divergence/)).

An operand that is itself a **sequence** prints a required pair of its own, and the
shell collapses into it — but that pair's `Aligned` layout has no line left before its
`)`, so handing it the comment would ride the `line_suffix` out past the clause's `;`
again. The pair is the EXPANDED shell instead, with the operands riding it bare, so
both nested authorings reach the one form the non-sequence operand's shell already
prints.

The **block**-comment case in the same gap is the opposite rule and is *not* a
divergence — the shell strips and the comment stays inline before the `;`, matching
prettier
([sequence_clause_paren_block_comment](../sequence_clause_paren_block_comment/)) —
except where the operand is a nested sequence, whose collapsed pair keeps it
([sequence_clause_nested_paren_block_comment](../sequence_clause_nested_paren_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
