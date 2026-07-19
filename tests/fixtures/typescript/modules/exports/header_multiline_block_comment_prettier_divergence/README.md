# export header multiline block comment

The export-all counterpart of
[imports/header_multiline_block_comment](../../imports/header_multiline_block_comment_prettier_divergence/)
— a **multiline** block comment in an `export * from` header gap hangs what follows it,
and tsv indents that continuation one level.

A multiline block is one of the two comment shapes that genuinely force a break (the
other is a line comment): the author broke after it, and reflowing would swallow the
`*/` line into the header. Once the break is forced, [§Uniform Forced-Continuation
Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
applies, uniformly with the same gap's line comment
([all_keyword_comment](../all_keyword_comment_prettier_divergence/)) and with every
value gap. A *single-line* block forces nothing and reflows inline instead.

The comment's own interior lines are left flush — tsv never re-indents a comment body.

Prettier keeps the break but relocates every export-all header comment to after `from`,
before the source, and leaves the continuation flat.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
