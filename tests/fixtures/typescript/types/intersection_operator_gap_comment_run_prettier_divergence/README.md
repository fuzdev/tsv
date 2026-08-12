# intersection_operator_gap_comment_run_prettier_divergence

A **run** of line comments straddling an intersection's `&` — one or more on their own
line *before* the operator, plus one trailing the operator itself
(`A⏎// x⏎& // c⏎B`). The `&` is pulled back onto the previous member's line, so every
comment in the gap follows it; the question is what happens to their **order** and to the
line each one gets.

**tsv** keeps them in the order the author wrote them, each on its own line
(`A &⏎// x⏎// c⏎B`). **Prettier** keeps the operator-trailing comment on the `&`'s line and
drops the earlier ones below it (`A & // c⏎// x⏎B`) — lossless, but it **reorders**: `// c`
was written after `// x` and prints above it.

Per Comment Position Philosophy, a comment's position is information, and so is its order
relative to its neighbours — tsv preserves both rather than re-binding one comment to the
operator. The forms are dual-stable: prettier holds its own output, and `input.svelte` here
is a fixed point for prettier too, so the divergence is only in how the straddling
authoring **normalizes** — pinned by `unformatted_ours_straddling.svelte`, whose prettier
chain needs `audit_signature_straddling.txt`: prettier is **non-idempotent** from that
source (its first pass leaves one comment at a shallower indent than its own second pass),
so no single-form marker claims it.

⚠️ This shape is why the run needs a separator per comment rather than a space: emitted
back-to-back, the operator-trailing `// c` welded onto the previous comment's line
(`// x // c`), which reparses as ONE comment whose text contains the second — content loss
that is idempotent, comment-count-clean to the print-once ledger, and therefore invisible
to every audit but a prettier differential. See
[comments.md](../../../../../docs/comments.md) §A run at the END of a container takes its
separator BEFORE each comment — the same rule, at the intersection's operator gap.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
