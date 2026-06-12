# union_member_long_line_comment_prettier_divergence

A union member that is a nested generic (`Outer<Inner<…>>`) whose type
arguments wrap, with an own-line line comment between members forcing the
multiline (line-comment) layout. Same tabs-only alignment divergence as
[nested_generic_member_long](../../nested_generic_member_long_prettier_divergence/),
exercised through the line-comment path instead of the main union path.

tsv: closing `>` at a whole tab (`3 tabs`)
Prettier: closing `>` at `2 tabs + 2 spaces`

Both are the same visual width at `tab_width = 2`; only the representation
differs. The inner row (`Inner<…>`) sits at `4 tabs` in both — the member's
internal break is offset one level past the `| ` prefix regardless of the
adjacent comment, matching how the main union path lays the same member out.

The `Short` and `Fit` cases (member stays inline) match Prettier exactly; only
the breaking `Brk` member diverges, and only on the closing `>` representation.

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. This keeps
indentation tab-width-agnostic. See
[docs/conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
