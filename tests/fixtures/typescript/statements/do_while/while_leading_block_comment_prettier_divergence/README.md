# while_leading_block_comment_prettier_divergence

Block comments on the same line as `while` keyword (leading the keyword, e.g., `/* c */ while`) are preserved before `while` on their own line. Prettier moves the comment inside the condition parentheses.

tsv: preserves comments before the `while` keyword
Prettier: relocates comments inside the while condition

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling of own-line comments before `while` ([line_before_while_comment](../line_before_while_comment_prettier_divergence/)) and other control flow statements.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
