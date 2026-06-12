# Arrow body paren comment divergence

When grouping parens around arrow body expressions contain trailing
comments (`() => (x /* c */)`), tsv preserves the parens to keep the
comment in its original position (same approach as unary expressions).

Prettier strips the parens and relocates the comment:
- Simple body: `(/* c */) => x` (moves to params)
- Call arg: `f((/* c */) => x)` (moves to params)
- Curried: `(a) => (b) => z /* c */;` (strips parens, comment trails)
- Line/block: `() =>\n  x; // c` (strips parens, different structure)

Reason: comment preservation. See conformance_prettier.md §Comment Position Philosophy.
