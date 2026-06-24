# Computed key bracket comment: `[x] /* c */` vs `[x /* c */]`

Prettier relocates comments between `]` and the next token (`:`, `=`, `(`) to
inside the brackets: `[x] /* c */ = 1` becomes `[x /* c */] = 1`.

tsv preserves the user's comment placement between `]` and the next token, per
the comment placement policy (preserve user intent, don't relocate).

Both `[x /* c */]` forms are dual-stable (`variant_inside_brackets`, identical to
prettier's `output_prettier`).

Affects: property (`=`), method (`()`), async method, generator (`*`), getter, setter.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
