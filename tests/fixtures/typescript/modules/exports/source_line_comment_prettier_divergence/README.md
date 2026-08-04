# export source line comment

A line comment in a re-export's `from`→source gap (`export … from // c⏎'x'`)
forces the source onto a new line; tsv indents it one level, uniform with every
other module-header line-comment gap.

Prettier diverges by shape (tsv preserves each in place + indented):

- **empty re-export** (`export {} from // c⏎'x'`) and **export-all**
  (`export * from // c⏎'x'`): prettier keeps the comment in place but flat
  (indent-only divergence).
- **named specifiers** (`export { a } from // c⏎'x'`): prettier relocates the
  comment into the braces as the last specifier's trailing comment, expanding them.

The matching block-comment cases stay flat in both formatters and live in the
regular [keyword_comment](../keyword_comment/) fixture.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
