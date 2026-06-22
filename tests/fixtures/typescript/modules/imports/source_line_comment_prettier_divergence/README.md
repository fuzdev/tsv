# import source line comment

A line comment in an import header gap immediately before the source — the bare
`import // c⏎'x'` keyword→source gap, or the `from // c⏎'x'` from→source gap —
forces the source onto a new line. tsv indents that source one level (a statement
spanning lines reads as a continuation), uniform with every other module-header
line-comment gap.

Prettier's handling of the `from`→source gap varies by binding shape, so tsv
diverges differently per case (all preserved in place + indented by tsv):

- **bare** (`import // c⏎'x'`) and **empty braces** (`import {} from // c⏎'x'`):
  prettier keeps the comment in place but flat (indent-only divergence).
- **named specifiers** (`import { a } from // c⏎'x'`): prettier relocates the
  comment into the braces as the last specifier's trailing comment, expanding them.
- **default binding** (`import Foo from // c⏎'x'`): prettier floats the comment
  past the `;` (the before-semicolon/float-out rule).

The matching block-comment cases stay flat in both formatters and live in the
regular [keyword_comment](../keyword_comment/) fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
