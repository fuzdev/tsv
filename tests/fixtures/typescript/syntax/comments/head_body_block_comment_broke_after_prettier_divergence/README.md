# head_body_block_comment_broke_after_prettier_divergence

A block comment trailing a header→body anchor with the body's `{` dropped to the next
line by the author (`if (a) /* b */⏎{`). One gap, two questions, and only the second
diverges:

| question | tsv | prettier |
| --- | --- | --- |
| does the `{` keep the line the author gave it? | **yes** | yes |
| does a blank the author left above that `{` survive? | **no** | yes |

`input.svelte` is the first row: the break is preserved at every block-bodied anchor —
`if`, `while`, the C-style `for`, for-of, for-in, for-await-of, `do`, and `else` — and
a run the author glued keeps its line with the `{` still below it. The controls are the
mirror authorings: a `{` glued to the `*/` stays glued, and a **non-block** body has no
line of its own to keep, so the break collapses and the run leads the body inline.

⚠️ The **declaration** head→`{` gap answers this same authoring the other way, on purpose:
`let a = function () /* c */⏎{}` collapses to `function () /* c */ {}` even though prettier
keeps the break there too, because a function body sits in *value* position and an unforced
break there is layout rather than authoring
([declaration_head_body_own_line_block_break_kept](../declaration_head_body_own_line_block_break_kept_prettier_divergence/),
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).
A control-flow body is not in value position, so that argument does not reach it. The two
gaps look like one rule and are two.

## Divergence

`prettier_variant_blank.svelte` pins the second row — prettier's stable form, with a
blank line above each `{`, which tsv normalizes back to `input`. That is the standing
brace rule, not a rule about this authoring: a body block's `{` never sits below a
blank, whatever put it there. Its line-comment twin is
[while/line_before_body_comment](../../../statements/while/line_before_body_comment_prettier_divergence/),
whose `prettier_variant_spaces` pins the same drop after a `// c` run.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §"No blank above a body block's `{`".
