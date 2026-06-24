# keyword_comment_prettier_divergence

Prettier relocates a comment between the `for` keyword and `(` to inside the
parens, before the init clause (`for (/* k */ a; b; c)`).

tsv: preserves the comment where the user placed it (`for /* k */ (a; b; c)`)
Prettier: moves the comment inside the parens before the init clause

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
across if/else, try/catch, switch, for, while, do-while, labeled statements, and
call chains.

The body layout itself matches Prettier: an empty body attaches directly (`);`),
a block body hugs (`) {`), and a non-block body stays inline when the header fits.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation (Keyword-paren comments).
