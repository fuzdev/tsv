# union_paren_union_member_long_prettier_divergence

A parenthesized union member (`Z | (A | B)`) whose inner union exceeds the print
width and breaks one member per line. No comments — this is the plain main-path
counterpart to
[union_paren_member_long_line_comment](comments/union_paren_member_long_line_comment_prettier_divergence/).

The `Short` and `Fit` cases (inner stays inline) match Prettier exactly. Only the
breaking `Brk` case diverges, and only on the closing `)`:

tsv: inner members at `4 tabs`, closing `)` at `3 tabs`
Prettier: inner members at `4 tabs`, closing `)` at `2 tabs + 2 spaces`

A paren-union member receives the per-member offset like any other union member,
so its inner content sits one level past the `| (` and the closing `)` lands at
the offset. The inner content is at `4 tabs` in both; only the `)` representation
differs.

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. See
[docs/conformance_prettier.md](../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
