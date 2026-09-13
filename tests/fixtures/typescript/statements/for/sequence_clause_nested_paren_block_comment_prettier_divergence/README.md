# sequence_clause_nested_paren_block_comment_prettier_divergence

A block comment inside a grouping shell around a **`for` header sequence clause's LAST
operand**, where that operand is itself a **sequence** (`for (a, (b, (c /* c */)); ;)`).

**tsv** keeps the comment inside the one pair the operand prints for itself — every
redundant shell collapses into that pair, so the comment stays inside the parens that
hold it. **Prettier** agrees on its first pass and walks the comment out one pair per
pass on the next, landing behind the operand's pair (`(b, c) /* c */`).

## Reason

The operand prints a required pair of its own, and the shell the parser erased around it
closes at the clause's end: the single emitted `)` stands in for every closer the source
held, so a comment before the last of them is inside the parens the output prints. A value
position collapses the same way where the shell wraps the WHOLE sequence
(`const x = ((a, b) /* c */);` → `const x = (a, b /* c */);`, where prettier keeps the
comment inside too).

The scope stops there, and this cell's structural twin at a value position marks the edge:
an outer sequence whose LAST OPERAND is a parenthesized sequence hands the comment OUT one
pair (`const x = (aa, (bb, cc /* c */));` → `const x = (aa, (bb, cc) /* c */);`, tsv's fixed
point and prettier's answer too), where the clause operand keeps it in. Only a `for` header
routes its last operand through a shell builder; at a value position that operand is an
operand like any other, so the shell erased inside it has no claimant and the sequence's own
float-out envelope carries the comment past the pair. Deeper nesting climbs the same way,
at least one pair per pass and never to a drop:
`const x = (aa, (bb, (cc, (dd /* c */))));` converges in two
(`(aa, (bb, (cc, dd) /* c */))`, then `(aa, (bb, (cc, dd)) /* c */)`), a fifth level in three.

Two authorings therefore reach one form: the comment written in the shell erased from the
inner sequence's own last operand, and the comment written in the shell around that
sequence's pair. A run split across both collapses in source order.

Prettier's chain from those authorings is pinned in `audit_signature_paren_shell.txt`: its
first pass keeps the comment inside the pair (tsv's form for two of the three cells) and
its second moves it out, which is the "a deferred run must not leave the construct it was
written in" rule failing in the oracle rather than a form tsv could adopt — the outward
walk has no fixed point until every pair is behind it.

A comment past the operand's own `)` was never inside a shell, and both formatters leave it
there ([sequence_clause_paren_block_comment](../sequence_clause_paren_block_comment/)). A
`//` in the same gap takes the expanded shell instead
([sequence_clause_paren_line_comment](../sequence_clause_paren_line_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
