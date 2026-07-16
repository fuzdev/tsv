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
which likewise keeps a line comment inside the retained parens for the first
member. The asymmetry shown by the `Mid` case: a leading line comment inside a
**later** member's parens relocates to trail the previous member (both formatters
agree — see
[union_paren_member_long_line_comment](../comments/union_paren_member_long_line_comment_prettier_divergence/));
only the first member, with no previous member to relocate onto, keeps the
comment inside.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
