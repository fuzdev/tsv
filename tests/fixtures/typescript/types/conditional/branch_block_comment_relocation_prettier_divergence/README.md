# Block comment in a conditional-type branch gap

The type-level twin of
[`operand_block_comment_relocation`](../../../expressions/ternary/operand_block_comment_relocation_prettier_divergence/):
a single-line block comment written **after** `?` or `:` in a conditional type,
in the gap between the operator and its branch.

- **tsv**: keeps the comment after the operator, where it was written
  (`B extends C ? /* c1 */ D : E`), collapsing to that form in one pass from
  every authoring.
- **prettier**: relocates it to the *other* side of the operator
  (`B extends C /* c1 */ ? D : E`), changing the comment's association from the
  branch it precedes to the type before it.

`input.svelte` is dual-stable, so there is no `output_prettier.svelte`. The two
authorings neither formatter keeps pin the two distinct prettier behaviors:

| variant | tsv | prettier |
| --- | --- | --- |
| `unformatted_ours_trailing_line` (comment trails the operand's line, newline after) | → `input` | → `variant_trailing`, one stable pass |
| `unformatted_ours_own_line` (comment on its own line) | → `input` | → `prettier_intermediate_own_line`, then → `input` |

The **trailing-line** authoring is the relocation proper — prettier's
comment-attachment only claims a comment as leading of the following branch when
it is not on the preceding node's line, so a comment sharing that line attaches
as trailing of the operand instead, across the operator. `variant_trailing` pins
that relocated form; it is dual-stable, which is why it is a `variant_*` rather
than a `divergent_variant_*`. The `?`-slot half of it —
`B extends C /* c1 */ ? D : E` — is kept flat by the flat conditional-type
path's `extends_type`→`?` seam; the plain fixture
[`extends_question_block_comment`](../extends_question_block_comment/) pins that
gap directly.

The **own-line** authoring is a pass-count gap, not a relocation: prettier
reaches the same fixed point tsv does, but its first pass breaks the whole
conditional on the comment's authored newline.
`prettier_intermediate_own_line` pins that unstable first pass, which
reconverges to `input`.

The line-comment form of the `:` slot is pinned separately by
[`comment_after_colon`](../comment_after_colon_prettier_divergence/) — same
association change, different comment kind.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
