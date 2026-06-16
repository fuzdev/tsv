# keyword → name line comment (continuation indent)

A line comment between a declaration's keyword and its name —
`function // c⏎f()`, `class // c⏎C {}`, `enum // c⏎E {}` — forces the name
onto a new line. tsv indents that continuation one level (a statement spanning
lines reads as a continuation, not a second statement), uniform with every
other declaration- and module-header line-comment gap.

Prettier keeps the comment in place but flat (the name stays at the statement's
own indent), so this is an indent-only divergence. Covers `function` (incl.
`async function` and `function*` generators), `class` (incl. `abstract class`),
`enum` (incl. `const enum`), and `declare function`.

Block comments and the no-comment case are byte-identical in both formatters and
stay in the regular [keyword_declaration_line_comment](../keyword_declaration_line_comment/),
[keyword_name_line_comment](../keyword_name_line_comment/), and
[keyword_name_line_comment_2](../keyword_name_line_comment_2/) fixtures.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
