# Angle-bracket assertion operand, line comment before the `)`

The line-comment spelling of
[type_assertion_operand_paren_comment](../type_assertion_operand_paren_comment_prettier_divergence/),
at the same gap: a `//` written between the assertion's operand and the closing
paren of its grouping shell.

- **`a`** — redundant parens, retained for the comment's sake.
- **`b`** — parens that are required anyway; the same single pair.
- **`c`** — a block glued behind the `//`, which is part of the same run and
  keeps its authored order.

## Formatter divergence (`_prettier`)

- **tsv**: retains the shell and keeps the comment inside it, with the operand on
  its own indented line — a `//` cannot trail inline before the `)` without
  swallowing it.
- **prettier**: strips the shell and defers the comment past the operand, the
  `)` and the `;` (`<T>p; // c`). With a block ahead of it prettier is
  non-idempotent on its own output, moving that block past the `;` on the second
  pass too (`audit_signature.txt`).

**Deferring here loses information, which is why tsv diverges rather than
matching.** A `//` carried past its own statement lands on a line that may
already hold one, and the two **merge**: `<T>(x // c1⏎); // c2` becomes
`<T>x; // c1 // c2`, which reparses as a *single* comment whose text is
` c1 // c2` — the second comment stops existing. That is the deciding test
[§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names, and the identical gap one construct over answers it identically
([non_null/operand_line_comment](../../expressions/non_null/operand_line_comment_prettier_divergence/)).

The retained shape is not exotic: it is prettier's *own* output for this content
whenever the operand is wide enough to break the cast, so the divergence is
confined to the widths at which prettier chooses to strip.

See
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Angle-bracket assertion operand shell) and
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
