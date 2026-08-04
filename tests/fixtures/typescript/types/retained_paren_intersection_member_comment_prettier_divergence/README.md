# retained_paren_intersection_member_comment_prettier_divergence

Comment inside a **retained** parenthesized **intersection** member — a `(x & y)`
whose parens are kept because it nests in an outer union
(`(a & b /* c */) | c`, `a | (/* c */ b & c)`, `a | (b & c /* c */)`) — and the
same gaps on an intersection whose **last member is an object literal**
(`(a & { x: X } /* c */) | c`), which takes a distinct aligned-layout printer, in
a union member and an optional tuple element alike.

**Prettier**: relocates the comment out of the parens (a trailing comment after
`)`, a leading comment before `(`):
```
type A1 = (a & b) /* c */ | c;
type A2 = a | /* c */ (b & c);
type A4 = (a & { x: X }) /* c */ | c;
```

**tsv**: keeps the comment where the user wrote it, inside the parens:
```
type A1 = (a & b /* c */) | c;
type A2 = a | (/* c */ b & c);
type A4 = (a & { x: X } /* c */) | c;
```

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it out. This is the
intersection-member counterpart of `union_intersection_retained_paren_comment`
(retained paren _unions_). The plain intersection members (A1–A3) preserve
through the paren-unwrapping path; the **trailing-object** shell (A4–A7) prints
its `(`…`)` itself, so both of its gaps are invisible to every other emitter and
must be emitted there — the leading one is pinned by
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/),
the trailing one here.

A **line** comment in the trailing gap (A7) keeps its line and drops the `)` to
the next, the same expanded shape the retained-paren union shell takes — a `//`
can't share a line with the `)` that follows it. That `)` takes the `align(2)`
sub-tab offset, landing under the `(` — the same column as the object's own `})`
closer when it breaks, and as the union shell's `)`, so the shell's closer sits
in one place whether or not the object stayed flat. Every position is
dual-stable in our formatter.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
