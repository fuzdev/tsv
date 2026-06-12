# empty_comment_prettier_divergence

Prettier moves comments from an empty switch body into the discriminant parens (`switch (\n\tx\n\t// comment\n) {}` instead of `switch (x) {\n\t// comment\n}`).

tsv: preserves comments where the user placed them (in the switch body)
Prettier: relocates comments to the discriminant parens

## Reason

tsv treats user comment placement as intentional. A comment in the switch body reads as "this switch is empty because..." — moving it to the discriminant changes the meaning. Consistent with tsv's handling across if/else, try/catch, for, while, do-while, labeled statements, and call chains.
