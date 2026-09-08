# space_separated_leading_comment_long_prettier_divergence

A space-separated value opening with a comment run whose head — `prop: /* c */ word` — does not
fit the print width.

tsv: the run is a fill item like every other part of the value, so the value breaks after it onto
a continuation line (`prop: /* c */⏎\t\t\tword 2px;`), every line ≤100
Prettier: the run is postcss `raws.between` material, printed with the colon and glued to the
first word, so that first line is unbreakable and overruns (101 here; by as much as the word is
long)

## Reason

Print width. The space after the run is a real boundary, and tsv breaks there rather than let
the head run past 100 — the same shape the broken comma list already gives a leading run (the
run stays on the colon's line, the value beneath it). Where the head fits, the two agree — the
100-column cells here and in [space_separated_comment_long](../space_separated_comment_long/).
See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Leading comment run before an over-long first word").

## Related

- [space_separated_comment_long_wrap](../space_separated_comment_long_wrap_prettier_divergence/) — the
  `;`-overage boundary of the same comment-bearing fill
- [comma_comment_interior](../comma_comment_interior/) — the broken comma list's hoisted leading run
