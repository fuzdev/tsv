# Computed key bracket comment: `[x] /* c */` vs `[x /* c */]`

Prettier relocates comments between `]` and the next token (`:`, `=`, `(`) to
inside the brackets: `[x] /* c */ = 1` becomes `[x /* c */] = 1`.

tsv preserves the user's comment placement between `]` and the next token, per
the comment placement policy (preserve user intent, don't relocate).

Both forms are dual-stable: `[x /* c */]` is idempotent in both formatters.

Affects: property (`=`), method (`()`), async method, generator (`*`), getter, setter.
