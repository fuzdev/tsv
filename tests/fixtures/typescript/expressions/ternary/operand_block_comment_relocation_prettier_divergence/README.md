# Block comment in a ternary operator→branch gap

A single-line block comment written **after** `?` or `:`, in the gap between the
operator and its branch. Both slots behave identically.

- **tsv**: keeps the comment after the operator, where it was written
  (`cond ? /* c1 */ x : y`), collapsing to that form in one pass from every
  authoring.
- **prettier**: relocates it to the *other* side of the operator
  (`cond /* c1 */ ? x : y`), changing the comment's association from the branch
  it precedes to the operand before it.

`input.svelte` is dual-stable — both formatters keep it verbatim — so there is
no `output_prettier.svelte`. The divergence lives in the two authorings neither
formatter keeps:

| variant | tsv | prettier |
| --- | --- | --- |
| `unformatted_ours_trailing_line` (comment trails the operand's line, newline after) | → `input` | → `variant_trailing`, one stable pass |
| `unformatted_ours_own_line` (comment on its own line) | → `input` | → `prettier_intermediate_own_line`, then → `input` |

Two distinct prettier behaviors, so two pinning shapes. The **trailing-line**
authoring is the relocation proper: prettier's
`handleConditionalExpressionComments` (`src/language-js/comments/handle-comments.js`)
only claims the comment as leading of the following branch when it is *not* on
the same line as the preceding node; here it shares that line, so default
end-of-line attachment makes it trailing of the operand — across the operator.
`variant_trailing` pins that relocated form, which is dual-stable.

The **own-line** authoring is not a relocation at all: prettier reaches the same
fixed point tsv does, just non-idempotently (its first pass breaks the whole
ternary on the comment's authored newline; its second collapses back).
`prettier_intermediate_own_line` pins that unstable first pass — it reconverges
to `input`, so there is no variant. tsv lands on the fixed point in one pass,
deliberately: see the note on prettier's own-line `hardline` in
`build_branch_comment_run`.

This is the block-comment face of the line-comment divergence pinned by
[`comment_after_colon`](../../../types/conditional/comment_after_colon_prettier_divergence/)
(same association change, type level) and the operator-gap mirror of
[`consecutive_operand_comment`](../consecutive_operand_comment_prettier_divergence/).
Its type-level twin is
[`branch_block_comment_relocation`](../../../types/conditional/branch_block_comment_relocation_prettier_divergence/).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
