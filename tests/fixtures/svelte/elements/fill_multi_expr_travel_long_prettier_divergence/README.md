# fill_multi_expr_travel_long_prettier_divergence

A run holding **multiple** breakable expression tags reflows like any other fill: every
welded word+tag unit shares one fit check, a unit that fits at its position stays flat, and
the first unit that does not fit travels whole to the whitespace boundary in front of it —
no expression is torn open while a boundary could still absorb the overflow. The multi-tag
sibling of `fill_spaced_tag_travel_long_prettier_divergence` (one spaced tag travels alone)
and `fill_glued_tag_travel_long_prettier_divergence` (one glued pair travels): here the run
carries two breakable ternaries, so the layout must *choose a boundary*, and it chooses the
one in front of the first unit that no longer fits — never a tear inside a unit that does.

Prettier's boundary measurement stops at an expression's first internal break, so it keeps
every unit on the text line and opens the **last** ternary mid-line, overshooting printWidth
(`… word2a{count2abcdefghi !== 1` at 101+ / `? 's'` / `: ''}`). That form is prettier-stable
— see `prettier_variant_midline.svelte` — and prettier also keeps the traveled form, so
`input.svelte` is a fixed point of **both** formatters and the divergence is one of
normalization: which form the other authorings converge to.

The first and second cases pin the exact boundary: at exactly 100 the whole run packs on one
line (a form both formatters keep); at 101 the last welded unit travels and collapses flat
while the mid-run unit `word1{…},` holds flat where it stands. The third case is glued
ternary+tag pairs around a spaced `=` (`{cond ? '+' : ''}{expr} = …%`): the run splits at
the boundary after the `=`, each glued pair intact — the first pair holds flat at line
start, the second travels whole. The fourth is a unit too wide even for a fresh line, so it
travels first and breaks internally there.

`unformatted_ours_compact.svelte` is the one-line authoring: tsv → `input.svelte`, prettier
→ the mid-line-open forms of `prettier_variant_midline.svelte`.

## Reason

Design choice — the travel doctrine applied uniformly to runs with more than one breakable
expression: printWidth is a hard limit, the whitespace boundaries in the run are render-free,
and spending one costs nothing where tearing a ternary open costs the widest column. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_multiple_expr_long_prettier_divergence/` (the same run family at an indent
where prettier instead dangles the parent's opening tag) and
`fill_competing_expr_prettier_divergence/` (the padded-boundary authorings that must
converge without oscillating).
