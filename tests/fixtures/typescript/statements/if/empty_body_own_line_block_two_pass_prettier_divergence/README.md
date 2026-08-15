# Divergence: empty-body `;` gap, own-line block, two passes

An **own-line** block comment in a gap whose body is an empty statement (`if (a)⏎/* c */⏎;`,
and the same gap after `else`, `while (…)` and a for-in/of header). Both formatters end at
the same form — `if (a) /* c */ ;` — so this is not a position or layout divergence; it is a
**normalization** one. tsv reaches the fixed point in a single pass, while prettier needs
two: its first pass pulls the comment onto the `)` line but keeps the author's break before
the `;` (`prettier_intermediate_own_line`), and its second collapses that break, because by
then the comment is no longer own-line.

That first-pass form is not one prettier itself holds, so pinning the chain is what keeps the
agreement honest — a one-pass comparison reads as a divergence here when there is none.

## Reason

The run is emitted on the anchor's line by construction, so the newline the author wrote
before it is erased by tsv's own output. Asking whether the comment is own-line would force a
break the next pass removes — non-idempotent, which is exactly why prettier takes two passes.
tsv answers that question `false` for the run's first comment
(`LeadingGlue::AdjacentAnchorLine`) and lands on the fixed point directly. Every later comment
keeps the source reading, since the separator that gave it its own line reproduces it —
pinned by the plain sibling [empty_body_comment_run](../empty_body_comment_run/).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
