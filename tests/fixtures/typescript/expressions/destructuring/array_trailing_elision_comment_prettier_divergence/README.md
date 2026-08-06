# array_trailing_elision_comment_prettier_divergence

The array-pattern face of
[sparse_trailing_hole_comment](../../arrays/sparse_trailing_hole_comment_prettier_divergence/):
a block comment in the region after the last binding, where a trailing elision leaves no next
element for it to lead. Prettier pulls it back before that binding's comma
(`const [a, ,/* c */] = arr` → `const [a /* c */, ,] = arr`), flipping the binding onto `a`.

We keep it past the comma, in the pattern's own trailing position. An elision is a **slot**: it
binds nothing and its comma is re-emitted structure, so a comment slides *forward* past it and
never backward across the binding's own comma.

A comment authored before the elision comma (`const [a, /* c */ , ] = arr`) and one authored
after it reach the same place — the region has one emitter, not one per comma
(`unformatted_ours_authored`).

An own-line comment takes its own line and the pattern expands. **No blank line is preserved
ahead of it**: the elision's own line break is structure, not authorship, which is the same rule
`has_blank_line_after_slot` states for the array literal (prettier's `node &&`) — and reading it
as a blank made the form non-idempotent, the reprint measuring a blank the first pass had not
written.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`Array trailing-elision block comment`).
