# export → declaration line comment

A line comment between `export` (or `export default`) and the declaration it
modifies — `export // c⏎const a = 1`, `export default // c⏎function f() {}` —
forces the declaration onto a new line. tsv indents it one level (a statement
spanning lines reads as a continuation), uniform with every other module-header
line-comment gap.

Prettier keeps the comment in place but flat (the declaration stays at the
statement's own indent), so this is an indent-only divergence. Covers `const`,
`function`, `type`, `interface`, and `export default function`.

The sibling `function`→name gap (`function // c⏎f() {}`) is a different
construct (not a module header) and stays flat in both formatters — it lives in
the regular [keyword_declaration_line_comment](../keyword_declaration_line_comment/)
fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
