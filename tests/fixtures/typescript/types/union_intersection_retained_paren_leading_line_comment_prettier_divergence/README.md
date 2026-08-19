# union_intersection_retained_paren_leading_line_comment_prettier_divergence

Leading **line** comment inside a **retained** parenthesized union member — a
`(x | y)` whose parens are kept because it nests in an outer union — when that
member is the **first** member of the outer union (`(// c\n A | B) | C`).

**Prettier** moves the comment out of the parens to lead the member, keeping the
inner union inline when it fits (`| // c\n (A | B)`). **tsv** keeps the comment
where the user wrote it, inside the parens leading the inner union. Because a line
comment must end its line, the parens expand (`(` and `)` on their own lines) with
the comment on its own line above the inner union — but the inner union itself
stays inline (`A | B`) when it fits.

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it out.

`FirstIntersection` is the same shape with a **paren-intersection** member whose
trailing object supplies its own aligned layout (`(// c\n A & { … })`). It is a
separate case because that layout is built from the *already-unwrapped* inner type,
so the paren's own `(`→inner gap is invisible to it and the comment has to be
threaded in explicitly — without that it is silently **dropped**, which no
prettier comparison catches (the two forms differ anyway) and only the print-once
comment ledger reports. Its comment hugs the `(` because the author glued it there
— `FirstIntersectionOwnLine` is the same shell with the comment on its own line,
which it keeps. That is the opening-delimiter rule, and it is the whole rule at
every member here: **which line the comment takes is the author's, and the shape
around it is fixed** — one normalizing space after the `(` when it hugs, the value
indented into the shell, the `)` on its own line.

This mirrors the trailing-comment sibling
[union_intersection_retained_paren_line_comment](../union_intersection_retained_paren_line_comment_prettier_divergence/),
which likewise keeps a line comment inside the retained parens. The `Mid` case
shows this holds for a **later** member too: a leading line comment inside any
member's parens is kept inside, associated with the member it documents, not just
the first — tsv never hoists it out (whereas prettier hoists it onto its own line
above the member).

`MidFunction` / `MidConditional` / `MidIntersection` show the rule spans every
**retained**-paren member kind, not only unions: a later paren-function,
-conditional, or plain paren-intersection member keeps the comment inside too.
Prettier instead trails the comment on the *previous* member (`| A // c`) and keeps
the member inline (`| (() => B)`). Because tsv keeps the comment inside, the line
comment forces the paren group open — but only the paren: the conditional or
intersection inside stays inline when it fits, the same as the union arms above
(the comment's own line is supplied by the paren, so breaking the inner type too
would be a break its reparse has no cause to reproduce). All three are written with
the comment **glued** to the `(`, and keep that line — the same authorship
`FirstIntersection` above records — but the shape around it is the shared opener's:
one normalizing space, the value indented into the shell, the `)` on its own line.
They used to weld instead (`| (// c⏎↹↹  () => B)` — no space, the value never
indented, the `)` on its tail), which is the third form the shared opener exists to
prevent, and which made the own-line authoring of the same comment collapse onto it
rather than stand as its own fixed point. The gate that chose between the two shapes
stopped reading the caller's `ShellLeadingRun`: that licence is granted on an
upstream emitter having already placed the run, which is true of an intersection's
FIRST member and of a redundant member, and false of every later retained one.
Whether the paren
is *retained* is decided exactly as it is comment-free; only a **redundant** paren
(stripped) can't host the comment, and there it leads the member on its own line
instead — see
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/).

An *authored-trailing* comment (`| A // c`, written on the member's own line rather
than inside a following member's parens) is a different case and stays trailing in
both formatters — see
[union_paren_member_long_line_comment](../comments/union_paren_member_long_line_comment/).

`unformatted_ours_inside_parens.svelte` writes each comment inside the parens on the line
`input` gives it — its own for `First`, `FirstIntersectionOwnLine` and `Mid`, glued for
`FirstIntersection` and the three `Mid*` shells — with the surrounding layout flattened;
tsv normalizes it to `input`. It has to carry the same glue as `input`, because glue is
authorship: re-spelling a case here would send it to the *other* fixed point rather than
back to this one.
`unformatted_ours_extra_paren_layer.svelte` adds the author's **extra paren layers** to
two of those members (`((⏎// c⏎A | B))`, `(((…)))`): they collapse into the one emitted
pair and the run lands inside it just the same. The member ROUTER reads the shell through
every layer, so the emitter has to as well — matched on the paren's *direct* child, a
doubly-nested member found no emitter at all (the default path suppresses the run on the
grounds that an upstream one placed it, and none had) and the comment was DROPPED.

`variant_inside_parens.svelte` is prettier's answer from that source — the comment hoisted
out in front of the pair (`| // c⏎  (A | B)`) — which both formatters then hold stable,
since it is a different tree rather than a different layout of this one. The glue itself is
authorship, not layout, and every delimiter answers it the same way, per
[paren_shell_glued_leading_line_comment](../paren_shell_glued_leading_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
