# type_assertion_single_member_head_line_comment_prettier_divergence

A **line comment after a sole union / intersection member's authored operator** in an
**angle-bracket assertion** — `<| // c⏎A>x`, `<& // c⏎A>x`, and the same run one redundant
shell in (`<& (// c⏎| A)>x`). A one-member composite prints transparently as its member
(prettier drops the node in postprocess), so the `//` is the **cast's own `<`→type gap's**
and takes that gap's line-comment answer — exactly the form the operator-less authoring
lands on: it keeps the `<` line, the opening-delimiter rule (`< // c⏎\tA⏎>x`).

One pass; the operator never survives. That matters most for a **function-type** member:
it needs its shell only under the operator, so the composite's own leading-pipe layout
(`|⏎// c⏎() => R`, the shell stripped) is a document no parser reads back.

- `unformatted_ours_pipe.svelte` / `unformatted_ours_amp.svelte` /
  `unformatted_ours_amp_shell.svelte` — the three operator authorings, and
  `unformatted_ours_shell.svelte`, the same run reaching the gap through a stripped shell
  alone (`<( // c⏎A)>x`); tsv normalizes each to `input.svelte`.
- `output_prettier.svelte` — prettier's form from `input`: the `<`-line comment un-glued
  onto its own line (`<⏎// c⏎A⏎>x`).

## Reason

Transparency, and the delimiter-line rule read at one point: the same claim the tuple
element makes in
[tuple/element_single_member_head_line_comment](../tuple/element_single_member_head_line_comment_prettier_divergence/),
and the value and list seams in
[union_single_member_head_line_comment](../union_single_member_head_line_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
