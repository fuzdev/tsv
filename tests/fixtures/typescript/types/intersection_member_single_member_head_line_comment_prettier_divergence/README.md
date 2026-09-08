# intersection_member_single_member_head_line_comment_prettier_divergence

A **line comment after a sole union / intersection member's authored operator** —
`(| // c⏎A)`, `(& // c⏎A)` — as a **later member of an intersection**. A one-member
composite prints transparently as its member (prettier drops the node in postprocess), so
the `//` is the **enclosing member gap's** and takes that gap's own line-comment answer:
the comment leads the member on its own line under the `&`, at the continuation's indent —
exactly the form the operator-less authoring `B & (// c⏎A)` lands on
([intersection_redundant_paren_member_line_comment](../intersection_redundant_paren_member_line_comment_prettier_divergence/)).
One pass; the operator never survives.

- `unformatted_ours_pipe.svelte` / `unformatted_ours_amp.svelte` — the pipe and ampersand
  authorings; tsv normalizes both to `input.svelte`. Prettier does not: it lifts the comment
  onto the `&` line (`B & // c⏎\tC`) and past the statement for the object member
  (`B & { a: 1 }; // c3`) — `variant_lifted.svelte`, one form both
  formatters then hold stable, so the two authorings are dual-stable rather than one fixed
  point. Prettier keeps `input.svelte` itself stable (no `output_prettier`).

## Reason

Transparency: the composite has no operator to print, so a layout it chose for its own
head gap (the union's leading-pipe layout `| // c⏎  A`, which does not reparse after a
`&`; the intersection's indent shell, read back one level shallower) was a second fixed
point — or none — for one program. The member gap's answer is the one already cataloged
for the operator-less spelling, the same claim the value and list seams make in
[union_single_member_head_line_comment](../union_single_member_head_line_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
