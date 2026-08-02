# fill_expr_travel_boundary_long_prettier_divergence

The continuation line after a traveled expression tag, at the width boundary. The wide tag
starts the continuation line (see `fill_spaced_tag_travel_long_prettier_divergence`) and the
following text packs greedily after it; tsv breaks before the word that would cross 100,
while prettier packs it onto the continuation line and overflows (104 here).

tsv (≤ 100): `{'rrrr' + 'ssss'} aaaa … oooo pppp` / `qqqq rrrr ssss tttt`
Prettier (104): `{'rrrr' + 'ssss'} aaaa … pppp qqqq` / `rrrr ssss tttt`

## Reason

Strict print width. Prettier's fill lets the continuation line after a fill element exceed
printWidth; tsv enforces printWidth as a hard limit, breaking the last word to the next
fill line.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_expr_travel_continuation_long_prettier_divergence/` for matching behavior
when the continuation stays under 100.
