# spread_paren_comment_before_elision_prettier_divergence

A **normalization-path** divergence: the fixed point is shared, and only the number of passes
it takes differs. `input.svelte` is idempotent under both formatters.

An own-line block comment inside a spread's redundant grouping parens (`...(b⏎/* c */⏎)`) has no
line of its own once the parens are erased, so the array prints it as a sibling — the parent's
share of a stripped-paren interior
([docs/comments.md §A stripped-paren interior is a partition too](../../../../../../docs/comments.md)).
When an **elision** follows the spread, that share slides *forward* past the anonymous elision
commas, exactly like every other comment in a hole region
([sparse_hole_after_comma_comment](../sparse_hole_after_comma_comment_prettier_divergence/)) —
which is what the reprint reads it as, the parens being gone by then.

Prettier emits the share against the spread's *own* comma instead (`prettier_intermediate_authored`)
and then moves it on the next pass, so it needs two passes to reach the form it agrees with. We
reach it in one.

The array used to emit its share the same way prettier does, which made our own output disagree
with our reprint of it — two fixed points for one document.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`across an elision`).
