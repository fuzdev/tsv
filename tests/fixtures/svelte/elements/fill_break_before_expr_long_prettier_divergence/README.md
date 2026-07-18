# fill_break_before_expr_long_prettier_divergence

An overflowing inline-element fill whose next segment is an `{expression}` tag. tsv breaks *before*
the tag to keep the line within printWidth, so the tag and its trailing text lead the continuation
line together. Prettier keeps the tag on the line — overflowing to 101 chars — and breaks before the
trailing text instead.

tsv (breaks before the tag, line ≤ 100): `... ({hhhhhh} iiii),` / `{jjjjjjjj} kkkk`
Prettier (keeps the tag, 101 chars): `... ({hhhhhh} iiii), {jjjjjjjj}` / `kkkk`

The second `<p>` is one character shorter, so the whole fill stays on a single block-style line under
both formatters — the boundary just under the wrap.

`divergent_variant_overflow` is the same document authored the way Prettier settles it — the tag kept
on the line at 101 with its trailing text dangling below. Prettier keeps that form; tsv rewrites it to
a distinct third stable form (the tag and its trailing text each on their own line, since the authored
newline between them is a fill boundary tsv preserves).

## Reason

Strict print width. Prettier's fill algorithm lets the line reach 101 chars before breaking; tsv
enforces printWidth as a hard limit and breaks earlier, before the expression tag. Because the tag
leads the continuation line, the break point is the collapsible space in front of it — which must be
consumed by the line break, so the continuation carries no leading space and the layout is stable on
every pass.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_expr_break_boundary_long/` (text continuation after a multiline expression) and
`fill_expr_break_continuation_long/` (matching behavior when the continuation stays under 100).
