# comment_prettier_divergence

Prettier moves comments after a label colon to before the entire labeled statement (`// comment\nlabel: for` instead of `label: // comment\nfor`).

tsv: preserves comments where the user placed them
Prettier: relocates comments to a different position

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
