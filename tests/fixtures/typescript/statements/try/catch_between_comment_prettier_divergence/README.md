# catch_between_comment_prettier_divergence

Prettier moves comments between try/catch/finally blocks into the subsequent block body (`catch (e) {\n\t// comment` instead of `} // comment\ncatch (e) {`).

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the next block

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.
