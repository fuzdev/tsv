# close_paren_comment_prettier_divergence

Comments between the do-while condition's closing `)` and the terminating `;`
(e.g. `} while (x) /* c */;`) are preserved after `)`. Prettier relocates them
inside the condition parentheses — a block comment before `)`
(`} while (x /* c */);`), a line comment forcing the condition to break
(`} while (\n\ty // c\n);`).

tsv: preserves comments where the user placed them (after `)`, before `;`)
Prettier: relocates comments inside the while condition

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([line_before_while_comment](../line_before_while_comment_prettier_divergence/),
[while_leading_block_comment](../while_leading_block_comment_prettier_divergence/))
and after the open paren
([open_paren_comment](../open_paren_comment_prettier_divergence/)), and with
if/else, try/catch, switch, for, while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
