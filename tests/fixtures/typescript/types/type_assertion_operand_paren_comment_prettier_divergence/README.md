# Angle-bracket assertion operand, block comment before the `)`

A block comment the author wrote between the assertion's operand and the closing
paren of its grouping shell (`<T>(x /* c */)`). The parser strips such a shell,
so the region belongs to no node — the assertion's own span is what ends at that
`)`, and nothing else can reach the comment.

Four cases, one gap:

- **`a`** — a single glued block.
- **`b`** — a run of two, which keeps its authored order.
- **`c`** — an operand whose parens are *required* anyway (`m + n`); the retained
  shell is that pair, not a second one.
- **`d`** — the empty-gap control: with nothing in the shell the redundant parens
  strip as usual, so the retention below is the comment's doing and not a change
  to how the assertion prints.

## Formatter divergence (`_prettier`)

- **tsv**: keeps the comment where it was written and **retains the operand's
  parens for its sake**.
- **prettier**: strips the shell and floats the comment out — past the operand on
  pass 1 (`<T>x /* c */;`), then past the statement `;` on pass 2
  (`<T>x; /* c */`), which is its fixed point (`audit_signature.txt`).

Reason: comment preservation, on the criterion the catalog already states for the
`as`/`satisfies` operand shell — *a shell is redundant only when the stripped form
can still express the comment's position*. Here it cannot. The assertion's span
ends at that `)`, so once the shell is gone the comment falls to the statement's
terminator gap, which puts it **after the `;`** — no longer trailing the operand
it was written against, and on a line that may already hold a deferred comment.
Stripping is also not a fixed point (`<T>(x /* c */)` → `<T>x /* c */;` →
`<T>x; /* c */`), so matching prettier's first pass would trade a dropped comment
for a two-pass reflow.

The line-comment spelling of the same gap takes the same retention for the
stronger reason that a `//` would swallow the `)` —
[type_assertion_operand_line_comment](../type_assertion_operand_line_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Angle-bracket assertion operand shell) and
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
