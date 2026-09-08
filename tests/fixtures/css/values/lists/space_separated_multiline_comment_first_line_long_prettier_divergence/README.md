# space_separated_multiline_comment_first_line_long_prettier_divergence

A space-separated value holding a **multi-line** block comment whose **first** line reaches the
print width on the colon's line.

tsv: charges the comment's first line to the line it starts on — at 100 columns it stays there
(`1px /* aaa…`), at 101 it drops to a fresh line; after the drop the next word is measured from the
comment's real last column and stays beside it (`b */ 2px`)
Prettier: measures the comment as one string (`getStringWidth` over both lines, the newline counted
as content), so it breaks ahead of the comment at 100 columns already, and after the drop wraps the
word beneath the comment whenever that whole string does not fit on the fresh line; a LEADING
comment it never moves at all — it is `raws.between` material, copied verbatim after the colon —
so the inline authoring stands at 105 columns (`prettier_variant_leading_inline`) and tsv's own-line
form is stable for it too

## Reason

Print width, measured correctly — the same reading as the last-line sibling, at the comment's other
end. The comment's first line is where the text actually is on the colon's line, so that is what the
fit charges there; the interior newline restarts the column, so the word after the last line is
measured from where the comment ends. The 101-column cell agrees on both formatters (the comment
drops and the word stays beside its last line, whose whole string fits the fresh line). A leading
comment whose first line overruns the colon's line is the wide first item every tsv fill drops to a
fresh line (`space_separated_wide_first_item_long`), print width being a hard limit; the word after
it is placed from the comment's last line as everywhere else.
See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Multi-line comment width").

## Related

- [space_separated_multiline_comment_long](../space_separated_multiline_comment_long_prettier_divergence/) —
  the last-line half: the word after the comment fills its last line to exactly 100 columns
- [space_separated_multiline_comment_wrap_long](../space_separated_multiline_comment_wrap_long/) — the
  word after the last line does not fit and drops to a fresh line, on both formatters
- [space_separated_leading_comment_long](../space_separated_leading_comment_long_prettier_divergence/) —
  the single-line leading run, parted from the first word when the head does not fit
- [space_separated_wide_first_item_long](../space_separated_wide_first_item_long_prettier_divergence/) —
  the wide-first-item drop the leading comment takes
