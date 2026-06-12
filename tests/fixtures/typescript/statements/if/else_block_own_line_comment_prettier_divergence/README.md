# else_block_own_line_comment_prettier_divergence

Prettier moves block comments on their own line between `}` and `else` into the else block body.

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the else block

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
