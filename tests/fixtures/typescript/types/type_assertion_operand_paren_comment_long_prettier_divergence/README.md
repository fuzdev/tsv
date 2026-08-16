# Angle-bracket assertion operand, retained shell at the print width

The width boundary of
[type_assertion_operand_paren_comment](../type_assertion_operand_paren_comment_prettier_divergence/).
The retained shell is a *breaking* paren, so when the operand's own chain has to
wrap, its continuation lines take the indent one level in — inside the parens
being kept, not snapped back outside them.

The pair straddles the width the operand's chain group measures, which is the
whole assertion line:

- **`a`** — 100 columns, the chain fits and the shell stays on one line.
- **`b`** — 101 columns, the chain breaks and its continuation is indented.

## Formatter divergence (`_prettier`)

Same divergence as the parent fixture — prettier strips the shell and floats the
comment out, past the operand on pass 1 and past the statement `;` on pass 2
(`audit_signature.txt`). At `b` it then needs a break of its own and reaches it
the other way round: the *cast* opens (`<T>(⏎…⏎)`) with a second, `needsParens`
pair around the chain kept flat inside it, where tsv breaks the chain inside the
one pair the author wrote.

See
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Angle-bracket assertion operand shell) and
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
