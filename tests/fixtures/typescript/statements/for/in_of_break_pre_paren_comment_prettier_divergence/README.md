# in_of_break_pre_paren_comment_prettier_divergence

When a line comment forces a for-in/for-of header into the multi-line (breaking)
layout, tsv keeps the `for`-to-`(` region intact: a pre-paren block comment
(`for /* a */ (`) stays where the author wrote it, and `for await` is emitted
from the AST.

tsv: preserves the pre-paren comment in place; `for await` from the AST
Prettier: moves the pre-paren comment inside `(` and the line comment after `)`

## Reason

tsv treats user comment placement as intentional, consistently across the inline
and breaking for-in/for-of header layouts. A comment that merely contains the
word `await` stays a comment — it is never promoted to a `for await` keyword (a
for-in can never be `for await`). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
