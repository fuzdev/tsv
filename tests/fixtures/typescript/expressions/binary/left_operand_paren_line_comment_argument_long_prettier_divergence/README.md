# Binary chain's leftmost operand paren, line comment, at the width boundary

The `long` twin of
[left_operand_paren_line_comment_argument](../left_operand_paren_line_comment_argument_prettier_divergence/):
with the comment hoisted ahead of the chain, the chain's own line decides the break. At
exactly 100 columns it stays flat (`(a… || b) && c`); one column more and it breaks after
`&&`.

tsv reaches each form in one pass from the shell spelling
(`unformatted_ours_stripped_shell.svelte`). Prettier's first pass breaks both chains, the
comment bound inside the chain's group (`prettier_intermediate_stripped_shell.svelte`); its
second pass rejoins the one that fits.

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
