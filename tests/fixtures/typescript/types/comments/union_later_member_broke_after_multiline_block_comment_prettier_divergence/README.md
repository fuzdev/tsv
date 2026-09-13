# union_later_member_broke_after_multiline_block_comment_prettier_divergence

A multi-line block comment the author broke after, glued after a later union
member's `|` (`A | /* c⏎d */⏎B`).

**tsv** keeps the block leading its member: the break after the `*/` is kept and
the member drops to its offset below the block, exactly as a first member's
leading block does (`union_member_leading_block_comment`).
**Prettier** binds an in-union comment to the preceding member
(`handleUnionTypeComments`) and prints it trailing that member (`| A /* c⏎d */⏎| B`).

## Reason

Per Comment Position Philosophy: the block sits after the `|`, ahead of the member
it describes, so tsv keeps it there rather than re-binding it across the operator.
The break after the `*/` is prettier's own `printLeadingComment` rule — a block
with a newline after it takes a soft `line`, which a multi-line block forces open —
the rule prettier itself applies to a first member. A multi-line block glued to
the member keeps that line in both formatters.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
