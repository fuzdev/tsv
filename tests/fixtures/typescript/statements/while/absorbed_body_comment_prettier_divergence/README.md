# absorbed_body_comment_prettier_divergence

Prettier absorbs comments between `)` and `{}` in while loops into the block body.

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the block

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

The absorbed form (`variant_absorbed.svelte`) is dual-stable: both formatters keep it as-is, so it is a `variant_*`, not the canonical input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
