# element_single_member_head_line_comment_prettier_divergence

A **line comment after a sole union / intersection member's authored operator** at a
**tuple element** — `[| // c⏎A]`, `[& // c⏎A]`, and the same run one redundant shell in
(`[& (// c⏎| A)]`). A one-member composite prints transparently as its member (prettier
drops the node in postprocess), so the `//` is the **tuple's own gap's** and takes that
gap's line-comment answer — exactly the form the operator-less authoring lands on:

- on the `[` line it keeps that line, the opening-delimiter rule
  ([open_bracket_comment](../open_bracket_comment_prettier_divergence/): `[ // c⏎\tA⏎]`);
- after a comma it trails the comma (`D, // c⏎E`), where prettier agrees.

One pass; the operator never survives.

- `unformatted_ours_pipe.svelte` / `unformatted_ours_amp.svelte` /
  `unformatted_ours_amp_shell.svelte` — the three authorings; tsv normalizes each to
  `input.svelte`.
- `output_prettier.svelte` — prettier's form from `input`: the `[`-line comment un-glued
  onto its own line (`[⏎// c⏎A⏎]`), the comma-trailing one kept.

## Reason

Transparency, and the delimiter-line rule read at one point: the tuple's gap already
answers the operator-less authoring with the `[`-line form, and answers a comment reaching
it through a stripped shell (`[( // c⏎A)]`, `open_bracket_comment`'s `unformatted_ours_parens`)
the same way — so a comment reaching it through a dropped operator must land there too,
or one program has two fixed points keyed on a token the output does not contain. The same
claim the value and list seams make in
[union_single_member_head_line_comment](../../union_single_member_head_line_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
