# Own-line line comment in a conditional type's extends-type→`?` gap

A **line** comment the author put on its own line between a conditional type's
extends-type and its `?`.

- **tsv**: keeps it on its own line, above the `?` — the same shape the
  value-level conditional already produces for the identical authoring
  (`a instanceof B⏎// c⏎? x : y`). Consecutive comments each keep their line.
- **prettier**: relocates the whole run across the `?`, indenting it with the
  true branch (`? // c⏎\tD`).

The gap's **same-line** authoring is unchanged and non-divergent: a comment the
author left on the extends-type's line still trails it (`B extends C // c`, the
control case here and case H of
[extends_question_block_comment](../extends_question_block_comment/)). Only the
own-line authoring is at issue, and the rule that decides it is the shared
same-line-trails / later-line-breaks classification the chain-dot, call-argument
and value-level conditional gaps already use — so the two conditional printers
give one answer instead of two.

Preserving the line is what keeps the run lossless. Pulling an own-line run up
onto the extends-type's line merges it (`B extends C // c1 // c2`): the second
`//` becomes text of the first and a comment is gone. The `?`→branch gap states
the same rule for its own run —
[consecutive_branch_comment](../consecutive_branch_comment/).

Reason: Comment relocation. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
