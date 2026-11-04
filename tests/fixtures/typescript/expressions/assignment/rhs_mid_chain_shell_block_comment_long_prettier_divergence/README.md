# Value's mid-chain shell block comment, at the width boundary

The block-only twin of
[rhs_mid_chain_shell_comment](../rhs_mid_chain_shell_comment_prettier_divergence/): a block
comment in a shell the chain prints mid-chain (`x = (/* c */⏎a.y).q()`) prints on the head's
line, so the value is laid out as the paren-free spelling `x = a /* c */.y.q()` is. At
exactly 100 columns the `=` line holds (`x = a… /* c */.y⏎\t.q();`); one column more and the
value hangs under the operator.

tsv reaches that form in one pass from the shell spelling
(`unformatted_ours_stripped_shell.svelte`), and from the paren-free spelling with the block
opening the next line (`unformatted_ours_own_line_block.svelte`, `x = a…⏎/* c */ .y.q()`), which
takes the same layout: with no `//` in the gap the chain prints the block on the head's line.
Prettier lands on the same form in two passes from each. Its first glues the block to the head
and breaks before `.y` from the shell spelling (`prettier_intermediate_stripped_shell.svelte`),
before `.q()` from the paren-free one (`prettier_intermediate_own_line_block.svelte`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
