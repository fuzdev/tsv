# union_member_leading_block_comment_prettier_divergence

A multi-line leading block comment before a broken union member that breaks onto
its own line (`type A =\n\t| /*\n\t * doc\n\t */\n\ta\n\t| b;`).

**tsv** keeps the member (and the block comment's continuation lines) flush under
the `|`, using whole-tab indentation only. **Prettier** renders the union
member's 2-column `align(2)` offset as `tabs + 2 spaces` under `--use-tabs`, so
the member and the comment continuation lines sit two columns past the `|`. Both
forms are stable under their respective formatters.

## Reason

Per the Tabs-Only Indentation Philosophy, tsv never mixes tabs with alignment
spaces — it keeps the broken member at the `|`'s own indent level rather than
emitting prettier's `align(2)` sub-tab offset. At `tabWidth = 2` the visual result
is equivalent; only the representation differs. The fixture also covers a
parenthesized member, a member glued on the `*/` line (no source newline, stays
glued in both), and a single-line leading block comment (stays inline in both).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Tabs-Only Alignment.
