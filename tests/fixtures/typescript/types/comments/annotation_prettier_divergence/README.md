# annotation_prettier_divergence

A line comment between a property signature's `:` and a multi-member **union**
type (`{ b: // c\n\t\tX | Y }`).

**tsv** keeps the comment trailing the `:`, with the union dropped to a
continuation line indented one level. **Prettier** relocates the comment to its
own line after the `:`. Both forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after the `:`) and applies the uniform forced-continuation indent to the union,
rather than floating it to its own line. The contrast fixtures pin the boundary:
prettier **relocates** the comment for a simple type
([annotation_simple](../annotation_simple_prettier_divergence/)) and keeps an
intersection **flush**
([annotation_continuation_indent](../annotation_continuation_indent_prettier_divergence/)),
while tsv applies one continuation layout everywhere.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
