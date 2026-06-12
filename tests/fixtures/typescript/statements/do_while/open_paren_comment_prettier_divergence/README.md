# open_paren_comment_prettier_divergence

Prettier moves comments after `} while (` to after the semicolon. This behavior is unique to do-while — other constructs (if, while, for, switch) keep the comment inside the parens.

tsv: preserves comments where the user placed them
Prettier: `} while (x); // comment` (relocated from inside parens)

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, labeled statements, and call chains.
