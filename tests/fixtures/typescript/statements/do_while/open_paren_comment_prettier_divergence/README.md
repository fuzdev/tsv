# open_paren_comment_prettier_divergence

A comment after a do-while condition's open `(` (`} while ( // c\nx\n);`) is kept
after the `(`. Prettier relocates it to after the terminating `;`. This
relocation is unique to do-while — other constructs (if, while, for, switch) keep
the comment inside the parens.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([line_before_while_comment](../line_before_while_comment_prettier_divergence/),
[while_leading_block_comment](../while_leading_block_comment_prettier_divergence/)),
and with if/else, try/catch, switch, for, while, labeled statements, and call
chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
