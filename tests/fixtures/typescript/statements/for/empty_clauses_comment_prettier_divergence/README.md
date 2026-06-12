# empty_clauses_comment_prettier_divergence

Prettier misplaces comments in empty for loop clauses, moving them outside the parentheses entirely (`for (;;) // comment\n{`).

tsv: preserves comments where the user placed them
Prettier: relocates comments outside the for statement

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
