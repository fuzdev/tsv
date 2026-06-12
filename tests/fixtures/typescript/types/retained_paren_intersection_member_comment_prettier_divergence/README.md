# retained_paren_intersection_member_comment_prettier_divergence

Block comment inside a **retained** parenthesized **intersection** member — a
`(x & y)` whose parens are kept because it nests in an outer union
(`(a & b /* c */) | c`, `a | (/* c */ b & c)`, `a | (b & c /* c */)`).

**Prettier**: relocates the comment out of the parens (a trailing comment after
`)`, a leading comment before `(`):
```
type A1 = (a & b) /* c */ | c;
type A2 = a | /* c */ (b & c);
```

**tsv**: keeps the comment where the user wrote it, inside the parens:
```
type A1 = (a & b /* c */) | c;
type A2 = a | (/* c */ b & c);
```

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it out. This is the
intersection-member counterpart of `union_intersection_retained_paren_comment`
(retained paren _unions_); both already preserved here through the
paren-unwrapping path, this fixture pins the behavior. Both positions are
dual-stable in our formatter.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
