# of_in_keyword_binding_line_comment_prettier_divergence

A line comment between a for-of / for-in declaration keyword and its binding
(`for (const // c⏎x of y)`). The keyword→binding gap comment is preserved (a
same-gap block comment stays inline and *matches* prettier — see the plain
[decl_keyword_binding_comment](../decl_keyword_binding_comment/) fixture), but a
`//` runs to end-of-line, so the whole for-of/for-in header breaks.

tsv: preserves the comment after `const` and breaks the header, each of the
binding, keyword, and right on its own line (the uniform for-of/for-in
line-comment layout).
Prettier: keeps the comment trailing `const` but pulls the rest of the header
(`x of y`) back onto one indented line.

## Reason

tsv treats user comment placement as intentional and applies one for-of/for-in
header layout whenever a line comment forces the header open — the same
divergence as [of_line_comment](../of_line_comment_prettier_divergence/), here
at the keyword→binding position. The C-style `for` and standalone declarations
*agree* with prettier at this position (both indent the declarator continuation),
so only for-of/for-in diverges.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
