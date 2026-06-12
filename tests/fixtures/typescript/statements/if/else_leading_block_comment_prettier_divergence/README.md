# else_leading_block_comment_prettier_divergence

Block comments on the same line as `else` keyword (leading the keyword, e.g., `/* b */ else if`) are preserved before `else` on their own line. Prettier cuddles `} else` and relocates the comment inside the else body.

tsv: preserves comments before the `else` keyword
Prettier: relocates comments inside the else block body

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling of own-line comments before `else` ([else_block_own_line_comment](../else_block_own_line_comment_prettier_divergence/)) and across other control flow statements (try/catch, while, do-while, switch).

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
