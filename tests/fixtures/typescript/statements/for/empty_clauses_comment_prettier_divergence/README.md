# empty_clauses_comment_prettier_divergence

Comments attached to the empty clauses of a `for (;;)` header (after the init
`;`, after the test `;`, before the update) are preserved inside the parens where
the author wrote them. Prettier moves them all outside the parentheses entirely,
collapsing the header to `for (;;)` and stranding the comments before the body
`{` (`for (;;) // after empty init⏎…⏎{`).

tsv: preserves comments where the user placed them (inside the parens)
Prettier: relocates the comments outside the for header

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
across if/else, try/catch, switch, for, while, do-while, labeled statements, and
call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
