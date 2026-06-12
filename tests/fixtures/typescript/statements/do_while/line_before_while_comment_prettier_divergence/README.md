# line_before_while_comment_prettier_divergence

Prettier moves line comments between `}` and `while` in do-while loops inside the while condition.

tsv: preserves comments where the user placed them
Prettier: relocates comments to a different position

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
