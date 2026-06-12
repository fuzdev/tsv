# line_comment_absorbed_prettier_divergence

Prettier absorbs line comments between keyword/paren and block body into the block (or into catch parens).

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the block or catch parameter list

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

## Cases

- `try // comment {}` → Prettier absorbs into try block
- `catch (e) // comment {}` → Prettier absorbs into catch parens (`catch (\n\te\n\t// comment\n)`)
- `finally // comment {}` → Prettier absorbs into finally block
