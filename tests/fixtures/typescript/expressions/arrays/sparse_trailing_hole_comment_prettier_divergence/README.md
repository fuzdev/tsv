# sparse_trailing_hole_comment_prettier_divergence

The trailing-hole face of
[sparse_hole_after_comma_comment](../sparse_hole_after_comma_comment_prettier_divergence/): a
block comment in the region after the last real element, where a trailing elision leaves no next
element for it to lead. Prettier pulls it back before that element's comma
(`[x, ,/* c */]` → `[x /* c */, ,]`), flipping the binding onto `x`.

We keep it past the comma, in the array's own trailing position — the same slot
[sparse_block_comment_inline](../sparse_block_comment_inline/) already pins for an array with no
real element at all (`[, , , ,/* c */]`, where prettier agrees). A real element earlier in the
array does not change where the comment was written.

An own-line comment there takes its own line and the array expands (`b`), which is the ordinary
own-line block rule; prettier expands too and differs only in ordering the comment against the
elision comma.

Both authorings in `unformatted_ours_authored` reach our form: written before `x`'s comma
(`[x⏎/* c */, , ]`) or between the two commas (`[x, /* c */ , ]`), the comment slides *forward*
past the anonymous elision comma, never backward across `x`'s. Prettier normalizes each to
`output_prettier`.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`Array element end-of-line block comment`).
