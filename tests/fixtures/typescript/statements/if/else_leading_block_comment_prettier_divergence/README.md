# else_leading_block_comment_prettier_divergence

A block comment leading the `else` keyword. When the author writes it on its
**own line** before `else` (`}\n/* b */\nelse`), both formatters keep it there,
so `prettier(input) == input` and there is no `output_prettier` oracle.

The divergence survives in the **same-line** authoring, pinned by
`unformatted_ours_leading.svelte` (`/* b */ else if`): tsv splits the comment
onto its own line before `else` (normalizing to `input`), while prettier keeps it
cuddled on the `else` line, so prettier does **not** normalize that variant to
`input`. Prettier's cuddled output is itself prettier-stable, pinned as
`prettier_variant_leading.svelte` (tsv normalizes it back to `input`).

## Reason

tsv treats user comment placement as intentional, keeping a leading `else`
comment on its own line. Consistent with tsv's handling of own-line comments
before `else` ([else_block_own_line_comment](../else_block_own_line_comment/)) and
across other control flow statements (try/catch, while, do-while, switch).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
