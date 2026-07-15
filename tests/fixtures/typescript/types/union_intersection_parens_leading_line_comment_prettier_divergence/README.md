# union_intersection_parens_leading_line_comment_prettier_divergence

A **leading** line comment on a broken union member
(`type A =\n\t| // leading\n\ta\n\t| b;`), where the comment forces the member
onto its own line.

Both formatters offset the member past the `|` by prettier's per-member
`align(2)` (`union-type.js`). **Prettier** renders that 2-column offset as `tabs +
2 spaces` under `--use-tabs`. **tsv** rounds it up to one whole tab. At
`tabWidth = 2` the two are the same visual width — only the representation
differs. Both forms are stable under their respective formatters.

## Reason

Per the Tabs-Only Indentation Philosophy, tsv never mixes tabs with alignment
spaces: it renders the `align(2)` offset as a whole tab rather than prettier's
sub-tab `··`. The offset itself is *not* the divergence — dropping it would make
the member sit two columns shallower than prettier, which is a different layout
rather than a different encoding of the same one.

The offset covers the leading-comment run **together with** the member, because the
member's own first line is placed by that run's `hardline` rather than by the `| `
prefix — offsetting only the member would leave its first line one level shallower
than its own internal breaks. Prettier's `align(2, print())` has the same shape:
`print()` carries the leading comments, so the run and the type share one offset.

What takes the offset is the **run**; the member keeps whatever
`build_union_member_offset_doc` gives it. Cases D and E are why that distinction
matters: an object literal and a default-paren member each supply their own indent
and decline the offset, so wrapping the *member* in it would push their bodies two
columns past prettier and leave their closing delimiter out of line with its opener
— a layout difference, not an encoding one.

## Cases

Every case here diverges: A is the plain member, B multiple leading comments, C a
leading line plus a trailing block, D an object-literal member, E a default-paren
member (an intersection with a trailing object). D and E break on **source**
multiline, not width, so this fixture is not width-sensitive.

The cases that **match** prettier — a later member's in-paren comment relocating to
trail the previous member, and the conditional `extends` form — live in the
non-divergence sibling
[union_intersection_parens_leading_line_comment](../union_intersection_parens_leading_line_comment/).
They show no offset because their members have no internal breaks, so there is no
continuation line for the encoding to differ on;
[union_paren_member_long_line_comment](../comments/union_paren_member_long_line_comment_prettier_divergence/)
pins the case where a later member *does* break internally and the offset surfaces.

Case E is authored with the comment **outside** the parens. The form with the
comment *inside* retained parens is a distinct stable form — tsv keeps it there,
pinned by
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Tabs-Only Alignment.
