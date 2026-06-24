# fill_expr_break_boundary_long_prettier_divergence

When fill content includes a multiline expression (binary `+` that breaks across lines), subsequent text continues on the continuation line. At the width boundary, Prettier allows the continuation line to reach 101 chars while tsv breaks at 96 to stay under 100.

tsv (96 chars): `'ssss'} aaaa bbbb ... pppp qqqq`
Prettier (101 chars): `'ssss'} aaaa bbbb ... pppp qqqq rrrr`

## Reason

Strict print width. Prettier's fill algorithm allows the continuation line after a multiline fill element to exceed printWidth by 1 char. tsv enforces printWidth as a hard limit, breaking the last word to the next fill line.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_expr_break_continuation_long/` for matching behavior when continuation stays under 100.
