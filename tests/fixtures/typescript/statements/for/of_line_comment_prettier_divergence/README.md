# of_line_comment_prettier_divergence

Prettier scatters the line comments authored inside a broken for-of header to
several different positions: `// before const` floats before the entire `for`
statement, `// after left` trails the header's `{` line, and the remaining
gap comments (`// after of`, `// trailing`) are pushed into the body and merged
onto one line.

tsv: preserves each comment where the user placed it (inside the header)
Prettier: relocates the comments to different positions

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
