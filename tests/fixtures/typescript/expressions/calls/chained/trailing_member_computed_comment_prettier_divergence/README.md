# trailing_member_computed_comment_prettier_divergence

Prettier requires 2 passes to reach stable output for line comments before computed access (`// comment\n[0]`). tsv normalizes to the same stable output in a single pass.

tsv: `items.filter((x) => x)[0]; // comment` (1 pass, stable)
Prettier: `items.filter((x) => x)[// comment\n0]` (1st pass) -> same as tsv (2nd pass)

Both formatters produce identical stable output. The divergence is only in normalization — tsv reaches stability in one pass.

## Reason

Stable quirk. tsv normalizes consistently. Prettier's intermediate form is a "stable quirk" — it takes multiple passes to converge.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
