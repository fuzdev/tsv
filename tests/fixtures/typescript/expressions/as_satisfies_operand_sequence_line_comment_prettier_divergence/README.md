# as_satisfies_operand_sequence_line_comment_prettier_divergence

A **sequence** operand of an `as` / `satisfies` cast, inside the grouping shell a leading
own-line comment forces the operand to keep (`(⏎// c⏎x, (y // c⏎)⏎) as A`).

**tsv** keeps the shell and both of its gaps: the leading run above the operands, and the
trailing run — including a comment written inside the shell the parser erased from the
sequence's **last** operand — after them, inside the one pair. **Prettier** hoists the
leading comment out to the `=` and floats the trailing one past the statement.

## Reason

The sequence supplies its own required parens, so the retained shell IS that pair rather
than a second one around it: the operands ride it bare. Both gaps are then the shell's to
emit — nothing else can see inside a pair the printer synthesizes — and the trailing one
opens where the operands stop, not at the sequence's span end, since the shell the parser
erased from the last operand closes inside the pair.

The operands stay on the line the layout gives them: a comment the SHELL prints, below
them, is not a reason to split them, and reading it as one would break the run on the
first pass and settle flat on the second.

The shell's keep-or-strip rule, the two-gap rule, and prettier's relocation are the
cast family's, stated once at
[as_satisfies_operand_line_comment](../as_satisfies_operand_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
