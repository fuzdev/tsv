# in_of_own_line_comment_prettier_divergence

Prettier moves own-line comments from inside for-in/for-of headers to different positions: block comments move before the `for` statement, line comments move to trailing after `)`.

tsv: preserves comments where the user placed them
Prettier: relocates comments before statement (block) or after `)` (line)

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
