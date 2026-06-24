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

The absorbed form (`variant_absorbed.svelte`) is dual-stable: both formatters keep it as-is, so it is a `variant_*`, not the canonical input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
