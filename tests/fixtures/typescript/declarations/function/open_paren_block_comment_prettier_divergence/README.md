# open_paren_block_comment_prettier_divergence

The **block**-comment spelling of
[open_paren_line_comment](../open_paren_line_comment_prettier_divergence/): a comment the
author left on a value function-definition's `(` line, with the first parameter below it.

Both formatters agree on the two settled forms — glued to the parameter
(`fn(/* c */ a)`) and alone on its own line — so the divergence is entirely in how the
`( /* c */⏎a)` authoring **normalizes**. tsv keeps the comment inside the parameter list,
leading the first parameter: on the parameter's line when the list fits, and above it when
the list breaks, since prettier's own leading-comment separator is a soft `line` there
(`unformatted_ours_paren_line` → `input`). Prettier instead hoists it **out of the
parentheses entirely**, to between the function name and the `(`
(`function fn /* c */(a) {}`, `variant_prettier_hoisted`, dual-stable) — a move across the
delimiter the comment was written inside, and the position where an author reading the
result can no longer tell it was about the parameters.

A function **type** is the control: prettier does not hoist there, and the same authoring
collapses to `(/* c */ a: string) => void` in both.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
