# empty_clauses_block_comment_prettier_divergence

Block comments attached to the empty clauses of a `for (;;)` header (before the
init `;`, between the two `;`, after the test `;`, and the `for`→`(` keyword
comment) are preserved inline where the author wrote them, and the trivially-short
header stays on one line. Prettier moves them all outside the parentheses,
collapsing the header to `for (;;)` and stranding the comments between `)` and the
body `{` (`for (;;) /* a */ {`).

tsv: preserves block comments inline inside the parens, header stays on one line
Prettier: relocates the comments outside the for header

## Reason

tsv treats user comment placement as intentional. Prettier itself keeps these
comments inline when any clause is non-empty (`for (/* a */ let i = 0; …)`) and
only relocates them once every clause is empty — an internal inconsistency tsv
declines to mirror. Preserving inline is also lossless: the leading comment (the
`(`-adjacent `/* a */`) would otherwise be dropped entirely. Consistent with
tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled
statements, and call chains, and with the sibling line-comment case
([empty_clauses_comment](../empty_clauses_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
