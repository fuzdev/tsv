# Computed key bracket comment in interface members

Prettier relocates comments between `]` and the next token to inside the
brackets: `[x] /* c */: number` becomes `[x /* c */]: number`.

Exception: for `set` accessors, prettier relocates the comment into the
parameter list instead: `set [x] /* c */(a)` becomes `set [x](/* c */ a)`.

tsv preserves the user's comment placement between `]` and the next token,
per the comment placement policy (preserve user intent, don't relocate).

Both `[x /* c */]` forms are dual-stable (variant_inside_brackets).
Note: variant uses inside-brackets for setter too (both stable), even though
prettier's relocation target for setter is different (inside parens).

Affects: property (`:` next), method (`()` next), getter, setter.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
