# union_paren_member_long_line_comment_prettier_divergence

A parenthesized union member with a **leading line comment inside the parens**
(`Z | (// c\n A | B)`). The comment relocates to trail the previous member
(both formatters agree), and because the inner union exceeds the print width it
breaks one member per line with the retained `(`/`)` on their own lines.

The `Fit` case (inner union stays inline) matches Prettier exactly. Only the
breaking `Brk` case diverges, and only on the closing `)`:

tsv: inner members at `4 tabs`, closing `)` at `3 tabs`
Prettier: inner members at `4 tabs`, closing `)` at `2 tabs + 2 spaces`

The inner content sits one level past the `| (` member offset in both; only the
closing `)` representation differs — the same tabs-only divergence as
[nested_generic_member_long](../../nested_generic_member_long_prettier_divergence/)
and [union_member_long_line_comment](../union_member_long_line_comment_prettier_divergence/).
A paren-union member receives the per-member offset just like any other union
member, so its inner content and `)` line up one level past the `| `.

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. See
[docs/conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
