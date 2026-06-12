# case_block_comment_prettier_divergence

When a line comment appears between a case label and its block body, Prettier relocates the comment through 2-3 passes to reach stability. tsv preserves the comment in a single pass.

tsv: `case 'a': // comment\n{` (preserved, 1 pass)
Prettier: `case 'a': // comment` -> `case 'a': { // comment` -> `case 'a': {\n  // comment` (2-3 passes)

A comment before the block (`case 'a': // why this case`) reads differently than inside the block — the position is semantically meaningful.

## Reason

tsv treats user comment placement as intentional. Prettier's multi-pass instability for this case indicates the behavior is not well-defined. Consistent with tsv's handling across if/else, try/catch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
