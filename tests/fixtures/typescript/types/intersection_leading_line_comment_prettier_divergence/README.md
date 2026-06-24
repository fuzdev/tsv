# intersection_leading_line_comment_prettier_divergence

Prettier requires 2 passes to reach stable output for a leading line comment on the first member of an intersection type. tsv normalizes to the same stable output in a single pass.

tsv: `type C = // leading\n  a & b;` (1 pass, stable)
Prettier: `type C = // leading\n  a &\n    b;` (1st pass) -> same as tsv (2nd pass)

The same pattern applies when the inner type is a parenthesized union (`(// leading\n a | b) & c`).

Both formatters produce identical stable output. The divergence is only in normalization — tsv reaches stability in one pass.

## Reason

Comment normalization (stable quirk). tsv normalizes consistently. Prettier's
intermediate form is a stable quirk — it takes multiple passes to converge, while
tsv reaches the same fixed point in one pass.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
