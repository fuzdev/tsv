# template_literal_interp_single_member_head_line_comment_prettier_divergence

A **line comment after a sole union / intersection member's authored operator** —
`` `x${| // c⏎A}` ``, `` `x${& // c⏎A}` `` — in a **template-literal type's interpolation**.
A one-member composite prints transparently as its member (prettier drops the node in
postprocess), so the `//` is the **interpolation's** and takes its own line-comment answer:
the comment stays trailing the `${`, the type dropped below it and `}` on its own line —
exactly the form the operator-less authoring `` `x${// c⏎A}` `` lands on (`` `x${ // c⏎\tA⏎}` ``, the
opening-delimiter rule's one space)
([template_literal_interp_trailing_comment](../template_literal_interp_trailing_comment_prettier_divergence/)).
One pass; the operator never survives.

- `unformatted_ours_pipe.svelte` / `unformatted_ours_amp.svelte` — the pipe and ampersand
  authorings; tsv normalizes both to `input.svelte`.
- `output_prettier.svelte` — prettier's form from `input`: the interpolation expanded with
  the comment moved down onto its own line (`` `x${⏎// c⏎A⏎}` ``).

## Reason

Transparency: the composite has no operator to print, so a layout it chose for its own
head gap (the union's leading-pipe layout, `${| // c⏎  A}`, which keeps a pipe the reparse
drops; the intersection's indent shell, read back one level shallower and re-broken) was a
second fixed point for one program. The interpolation's answer is the one already cataloged
for the operator-less spelling — the same claim the value and list seams make in
[union_single_member_head_line_comment](../union_single_member_head_line_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
