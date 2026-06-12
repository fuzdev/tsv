# trailing_comment_prettier_divergence

Prettier moves trailing comments from after for statement parens to inline with the update clause (`i++ // comment` instead of `i++\n) // comment`).

tsv: preserves comments where the user placed them
Prettier: relocates comments to a different position

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
