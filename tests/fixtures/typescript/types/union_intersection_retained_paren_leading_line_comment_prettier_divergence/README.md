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
comment ledger reports. Its comment hugs the `(` rather than taking its own line
below it, matching the other default-paren member shapes (function, constructor,
conditional, plain intersection); the paren-union `First` above is the one that
puts `(` and the comment on separate lines, because its layout gives `(` its own
line whenever the group breaks.

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
comment forces the paren group open, so a conditional breaks its branches and an
intersection its members — an expansion prettier's hoist avoids. Whether the paren
is *retained* is decided exactly as it is comment-free; only a **redundant** paren
(stripped) can't host the comment, and there it leads the member on its own line
instead — see
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/).

An *authored-trailing* comment (`| A // c`, written on the member's own line rather
than inside a following member's parens) is a different case and stays trailing in
both formatters — see
[union_paren_member_long_line_comment](../comments/union_paren_member_long_line_comment/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
