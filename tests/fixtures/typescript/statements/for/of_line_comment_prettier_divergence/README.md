# of_line_comment_prettier_divergence

Prettier scatters the line comments authored inside a broken for-of header to
several different positions: it collapses the header inline, keeps `// before
const` trailing the `(` (`for (// before const`), trails `// after left` on the
header's `{` line, and pushes the remaining gap comments (`// after of`,
`// trailing`) into the body, merged onto one line. (Prettier needs two passes
to settle the body comments — pinned by `audit_signature.txt`.)

tsv: preserves each comment where the user placed it (inside the header)
Prettier: relocates the comments to different positions

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
