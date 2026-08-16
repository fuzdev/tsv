# init_paren_line_comment_prettier_divergence

A line comment in the trailing gap of a **`for`-header init declarator's** grouping
shell (`for (let i = (a // c); ;)`).

**tsv** retains the shell and keeps the comment inside it, on the operand's line.
**Prettier** strips the shell and floats the comment past the header's first `;`.

## Reason

The `;` a for-header init clause meets is a **clause separator**, not a statement
terminator, so the shell is not terminator-adjacent and the comment has nowhere to
defer to. Deferring it anyway carries it out of the construct it was written in —
which is exactly what prettier's own output does on its **second** pass: the run's
second comment (`// c3`) leaves the header entirely and lands between the header's
`)` and the body `{`, pinned in `audit_signature.txt`. tsv keeps the comment inside
the shell, the same answer the statement-level twin gives for a line comment in this
gap
([init_assignment_paren_line_comment](../../variable/init_assignment_paren_line_comment_prettier_divergence/)).

The **block**-comment case in the same gap is the opposite rule and is *not* a
divergence — the shell strips and the comment stays inline before the `;`, matching
prettier
([init_paren_block_comment](../init_paren_block_comment/)). That asymmetry is the
same one the `;`-terminated value position draws, where the block defers past the
terminator and the line comment retains the shell
([value_paren_trailing_block_comment](../../../syntax/comments/value_paren_trailing_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
