# union_intersection_retained_paren_comment_prettier_divergence

Block comment inside a **retained** parenthesized union member — a `(x | y)`
whose parens are kept because it nests inside an outer union or intersection
(`a | (b | c /* c */)`, `(a | b /* c */) | c`, `a & (b | c /* c */)`).

**Prettier**: relocates the comment out of the parens — a trailing comment moves
after `)`, a leading comment moves before `(`:
```
type A1 = a | (b | c) /* c */;
type A2 = a | /* c */ (b | c);
```

**tsv**: keeps the comment where the user wrote it, inside the parens:
```
type A1 = a | (b | c /* c */);
type A2 = a | (/* c */ b | c);
```

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it to the surrounding
union/intersection. (Without this, the comment was dropped entirely — a
content-loss bug.) Contrast `union_intersection_parens_comment`, where the parens
are redundant and stripped, so both formatters agree.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
