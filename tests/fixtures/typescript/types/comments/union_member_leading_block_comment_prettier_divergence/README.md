# union_member_leading_block_comment_prettier_divergence

A multi-line leading block comment before a broken union member that breaks onto
its own line (`type A =\n\t| /*\n\t * doc\n\t */\n\ta\n\t| b;`).

Both formatters offset the member — and the block comment's continuation lines —
past the `|` by prettier's per-member `align(2)` (`union-type.js`). **Prettier**
renders that 2-column offset as `tabs + 2 spaces` under `--use-tabs`; **tsv**
rounds it up to one whole tab. At `tabWidth = 2` the two are the same visual
width — the member sits at column 6 and the comment's continuation `*` at column 7
in both — so only the representation differs. Both forms are stable under their
respective formatters.

## Reason

Per the Tabs-Only Indentation Philosophy, tsv never mixes tabs with alignment
spaces: it renders the `align(2)` offset as a whole tab rather than prettier's
sub-tab `··`. The offset itself is *not* the divergence — dropping it would put the
member two columns shallower than prettier, a different layout rather than a
different encoding of the same one.

The offset covers the leading-comment run together with the member, because when
that run ends in a break it is the run — not the `| ` prefix — that places the
member's own first line. Prettier's `align(2, print())` has the same shape:
`print()` carries the leading comments. tsv spells it by indenting the **run**;
the member keeps whatever `build_union_member_offset_doc` gives it, so the object
literal and default-paren members that supply their own indent still decline the
offset. The line-comment sibling
([union_intersection_parens_leading_line_comment](../../union_intersection_parens_leading_line_comment_prettier_divergence/))
is the same shape with a `//`.

Because the run is indented, the comment's continuation lines land at the offset
too, which is what aligns its `*` under the opening `/*`'s `*` — the same
re-alignment prettier applies to a `*`-styled block comment.

The fixture also covers a parenthesized member, a member glued on the `*/` line (no
source newline, stays glued in both), and a single-line leading block comment
(stays inline in both).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Tabs-Only Alignment.
