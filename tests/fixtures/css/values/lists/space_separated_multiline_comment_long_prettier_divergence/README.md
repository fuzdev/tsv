# space_separated_multiline_comment_long_prettier_divergence

A space-separated value holding a **multi-line** block comment, where the word after the comment's
last line fills that line to exactly 100 columns.

tsv: keeps the word on the comment's last line (`b */ aaaa…`, 100 columns) — the comment's interior
newline restarts the column, and the fit is measured from where the text actually is
Prettier: wraps the word — its `fits` subtracts the comment's **whole** string width
(`getStringWidth` over both lines, the newline counted as content), so a comment that ended at
column 4 is measured as if it ran on from column 100 and nothing fits after it

## Reason

Print width, measured correctly. Prettier's measure is a conservative accident of counting a
newline-bearing string as one run of characters; tsv's fit walk reads a newline-bearing text as
ending its line and continues from the real column. One column more and both wrap.
See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Multi-line comment width").

## Related

- [space_separated_comment_long](../space_separated_comment_long/) — the single-line comment fill
- [space_separated_multiline_comment_wrap_long](../space_separated_multiline_comment_wrap_long/) — one
  column more: the word drops to a fresh line and the tail fills beside it, on both formatters
- [multi_line_gap](../../../tokens/comments/multi_line_gap_prettier_divergence/) — the interior stays
  verbatim at its authored column
