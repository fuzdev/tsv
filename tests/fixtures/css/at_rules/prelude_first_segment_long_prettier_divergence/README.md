# prelude_first_segment_long_prettier_divergence

A single `@media` / `@supports` query whose **first** segment — everything before its first
`and` — is itself wider than the print width, and one whose first segment ends in a
**multi-line** comment.

tsv: the first segment stays on the at-rule's line and the query wraps at the `and` after it
(`@media (min-width: 999…px) and⏎\t\t(max-width: 2px)`); a segment ending in a multi-line
comment keeps the next segment beside the comment's last line, measured from where it ends
Prettier: never wraps a single query — the over-wide line stands (for `@supports` it breaks
inside the segment's own parens instead), and the multi-line comment is re-spelled onto one
line (its newline folded to a space)

## Reason

Print width. The wrap point tsv takes is the `and`/`or` boundary
([media_long](../media_long_prettier_divergence/), [supports_long](../supports_long_prettier_divergence/));
what this fixture pins is the **head**: the at-rule's name and its space are written ahead of
the query, nothing in the query separates the first segment from them, so the segment renders
in place however wide it is — the fill's fresh-line drop may not land there, since it would
leave the name's space stranded as trailing whitespace on a line of its own
(`@media ⏎\t\t(min-width: …`). The comment cell is the same reading as
[space_separated_multiline_comment_first_line_long](../../values/lists/space_separated_multiline_comment_first_line_long_prettier_divergence/):
a multi-line comment's interior newline restarts the column, and the segment after it is
measured from the comment's real end; prettier's re-spelling of the comment is a content
rewrite tsv does not make (§CSS: At-Rules, "Media feature split trivia" and
[media_list](../../tokens/comments/media_list_prettier_divergence/) carry the media reader's
comment handling).
See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
("Over-wide first segment of a single query").

## Related

- [media_long](../media_long_prettier_divergence/) — the `and`/`or` wrap itself, at the 100/101 boundary
- [supports_long](../supports_long_prettier_divergence/) — the same wrap on `@supports`
