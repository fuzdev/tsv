# discriminant_trailing_comment_prettier_divergence

Prettier moves trailing comments from switch discriminant parens into the switch body (`switch (x) {\n\t// comment` instead of `switch (\n\tx\n\t// comment\n) {`).

tsv: preserves comments where the user placed them
Prettier: relocates comments to a different position

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, for, while, do-while, labeled statements, and call chains.
