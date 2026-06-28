# intersection_leading_line_comment_prettier_divergence

A leading line comment on the first member of an intersection type, written
trailing the `=` (`type C = // leading\n\ta & b;`).

**tsv** keeps the comment trailing the `=`, with the intersection on a
continuation line indented one level. **Prettier** relocates the comment to its
own line after the `=`. Both forms are stable under their respective formatters.
The same pattern applies when the inner type is a parenthesized union
(`(a | b) & c`).

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after the `=`) and indents the intersection continuation rather than floating it
to its own line.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation.
