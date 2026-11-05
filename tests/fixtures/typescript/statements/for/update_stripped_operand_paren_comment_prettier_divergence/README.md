# For update and header items, comment in a stripped operand pair

The update-clause twin of
[while/condition_stripped_operand_paren_comment](../../while/condition_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a `for`
header's update clause (`a + (b // c⏎) // d`, an own-line `//`, a multi-line block, nested
pairs, a sequence's last operand) stays in the header after the update, ahead of a comment
the author wrote after the pair, which follows on its own line, as it does in a `while`
header. The update has no `;` behind it, so the header's `)` is what ends its line.

tsv: `a + b // c⏎// d`, the two comments in the order written, each on its own line
Prettier: `a + b // c // d`, the two comments on one line, where they weld into one comment
(`audit_signature_shell`), and a block written after the pair lands ahead of the `//`
(`a + b /* d */ // c`)

With nothing written after the pair the comment trails the update in both formatters — that
case is a control. In an operand of an update or init sequence, or a declarator, that
another follows, the comment trails the item's comma and the clause breaks per item
(`i++,⏎a + b, // c⏎j++ // d`), the form both formatters give the bare spelling
(`a + b // c⏎, j++`), rather than riding past every later item and the clause's own
comments. A pair around a whole declarator value is the header's own shell, which
keeps the comment inside it (§Comment relocation's *Redundant paren shell in a `for`-header
init declarator*). A
pair the printer adds ahead of `as`, or a computed index, holds the stripped pair's
comment inside the update, and it still prints ahead of a `//` written after it, each on
its own line. On the input itself prettier moves the own-line comment out of the
header, before the body (`output_prettier`), the relocation
[update_trailing_comment](../update_trailing_comment_prettier_divergence/) pins.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
