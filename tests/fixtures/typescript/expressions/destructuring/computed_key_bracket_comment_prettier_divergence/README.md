# Computed key bracket comment in destructuring

Prettier relocates comments between `]` and `:` to inside the brackets:
`{ [x] /* c */: a }` becomes `{ [x /* c */]: a }`.

tsv preserves the user's comment placement between `]` and `:`, per the
comment placement policy (preserve user intent, don't relocate).

Both `[x /* c */]` forms are dual-stable (variant_inside_brackets).

Affects: object destructuring assignment and function parameter destructuring.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
