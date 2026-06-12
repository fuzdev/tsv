# of_line_comment_prettier_divergence

Prettier misplaces line comments in for-of loops, moving them outside or into incorrect positions (e.g., `// before const` moves before the entire `for` statement).

tsv: preserves comments where the user placed them
Prettier: relocates comments to different positions

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
