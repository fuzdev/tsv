# grid_template_wrap_long_prettier_divergence

A `grid` / `grid-template*` value authored on one line past the print width, at the 100/101
boundary.

tsv: breaks after the colon and packs the value greedily into rows beneath it, each row at most
100 columns (`prop:⏎\t[a] … 1fr [b] 2fr;`); a leading comment run stays on the colon's line
Prettier: never wraps a grid value — its grid rule joins same-line nodes with a literal space, so
the line overruns by as much as the value is wide

## Reason

Print width as a hard limit, in the shape prettier's own grid rule dictates. Prettier lays a grid
value out by the author's source lines (consecutive nodes on different lines take a hardline), and
tsv takes that rule — so the wrap must land on a form the row read reproduces. A plain fill
(`prop: item1 item2⏎\titem3`) is not one: on the next pass both formatters read its line break as
a row and move the first row beneath the colon (`unformatted_ours_row_on_colon_line`). Breaking
the head with the value puts the wrap straight onto that row layout, one pass. The one-line
authoring is `prettier_variant_inline`: prettier keeps it as it is, over the width.
See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values)
("Grid value wrap").

## Related

- [grid_template_tracks_multirow](../grid_template_tracks_multirow/) — the row read on an authored
  multi-row track list
- [space_separated_long_wrap](../../values/lists/space_separated_long_wrap_prettier_divergence/) —
  the ordinary space-separated wrap, first item kept on the colon's line
