# Non-null operand, line comment before the `!`

The general form of
[grouped_operand_member_line_comment](../grouped_operand_member_line_comment_prettier_divergence/),
which pinned only the case where the parens are *required* and a member access
follows. The rule holds at every non-null operand: a **line** comment written
between the operand and its `!` keeps its own line **inside the operand's
parens**, and the parens are retained when they were otherwise redundant
(`b`, `c`).

- **tsv**: retains the shell and keeps the comment inside it.
- **prettier**: strips the redundant parens and defers the comment to end of
  line, past the `)`, the `!` and the `;`.

**Deferring here loses information, which is why tsv diverges rather than
matching.** A `//` carried past its own statement lands on a line that may
already hold one, and the two **merge**: `(x + y // c1⏎)!; // c2` becomes
`(x + y)!; // c1 // c2`, which reparses as a *single* comment whose text is
` c1 // c2` — the second comment stops existing. That is the canonical
information-losing relocation
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names as its deciding test, and the comment census measures it directly
(`MISSING "c1"` · `MISSING "c2"` · `EXTRA "c1 // c2"`).

A block comment in the same gap is unaffected — it trails inline without ending
the line, so it needs no shell and matches prettier
([operand_paren_comment](../operand_paren_comment/),
[chain_operand_comment](../../calls/chained/non_null_comment/)).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Non-null grouped operand).
