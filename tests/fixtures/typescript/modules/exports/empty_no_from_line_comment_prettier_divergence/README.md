# no-from empty export line comment

A line comment between `export` and an empty `{}` with no `from`
(`export // c⏎{}`) forces the braces onto a new line; tsv indents the `{}`
continuation one level, uniform with every other module-header line-comment gap.

Prettier keeps the comment in place but flat (`export // c⏎ {}`, a continuation
space rather than an indent), so this is an indent-only divergence. The second
case confirms brace-like glyphs inside the comment don't confuse brace detection.

The matching block-comment case (`export /* c */ {}`) stays flat in both
formatters and lives in the regular [keyword_comment](../keyword_comment/) fixture.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
