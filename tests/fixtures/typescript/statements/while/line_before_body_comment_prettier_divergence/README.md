# line_before_body_comment_prettier_divergence

Prettier absorbs line comments between `)` and `{` in while loops into the block body.

tsv: preserves comments where the user placed them (trailing, own-line, blank-line-separated)
Prettier: relocates comments inside the block

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
