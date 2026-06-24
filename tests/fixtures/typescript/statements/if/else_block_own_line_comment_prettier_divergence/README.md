# else_block_own_line_comment_prettier_divergence

Prettier cuddles `} else {` and moves a block comment on its own line between
`}` and `else` into the else block body. The blank-line-before-comment case
behaves the same way (the blank line is carried into the relocated position).

tsv: preserves comments where the user placed them (on their own line before `else`)
Prettier: relocates the comment inside the else block

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of block comments leading `else`
([else_leading_block_comment](../else_leading_block_comment_prettier_divergence/))
and across if/else, try/catch, switch, for, while, do-while, labeled statements,
and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
