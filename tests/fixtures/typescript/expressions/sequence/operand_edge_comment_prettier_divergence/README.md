# Sequence operand edge comment divergence

A redundantly-parenthesized sequence-expression operand whose parens hold a
comment anchored to the *outer edge* of the operand — a leading comment on the
first operand (`((/* c */ x), y)`) or a trailing comment on the last operand
(`(x, (y /* d */))`) — has its parens stripped and the comment floated out of
the sequence parens, matching prettier's fixed point:

- `fn(((/* c */ x), y))` → `fn(/* c */ (x, y))`
- `fn((x, (y /* d */)))` → `fn((x, y) /* d */)`

The float preserves the comment's source line-treatment (own-line when the
source has a newline between the comment and the operand, inline otherwise), so
it stays idempotent even when the sequence is nested inside surrounding
comments.

Prettier reaches this same fixed point but is **non-idempotent** getting there —
it needs two passes:

- pass 1: `fn(((/* c */ x), y))` → `fn((/* c */ x, y))` (comment still inline)
- pass 2: `fn((/* c */ x, y))` → `fn(/* c */ (x, y))` (floated)

tsv reaches the fixed point in one pass. Because the validator checks prettier's
*first* pass, the user's paren form is documented as `unformatted_ours_paren`
(our formatter normalizes it to `input` directly) paired with
`prettier_intermediate_paren` (prettier's unstable first-pass output, which
converges to `input` on the second pass).

Unlike an interior operand comment (between two operands, e.g.
`(x /* c */, /* c */ y)`), which is stable inline without parens and matches
prettier — see [operand_comments](../operand_comments/).

Reason: comment normalization (stable quirk) — tsv reaches prettier's fixed
point in one pass where prettier needs two. See
[conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
