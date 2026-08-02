# fill_leading_tag_pack_long_prettier_divergence

Text packing after a **leading tag** holds printWidth. The fill after a `{expr}` tag leads
with a collapsible `line`, and tsv measures that separator *together* with the word it
stands before, so a word packs onto the tag's line only while the line stays ≤ 100.
Prettier packs one word past the limit at these widths.

The two cases probe the exact boundary of the pack decision:

- **at 100** (tag line 94, the first `ccccc` lands exactly at 100): tsv packs the word;
  prettier packs a *second* word, overflowing to 106.
- **at 101** (tag line 95, the first `ccccc` would land at 101): tsv breaks the leading
  `line` and packs nothing; prettier still packs the word, overflowing to 101.

Contrast `fill_leading_line`, where the tag line sits at exactly 100 and both formatters
break the leading line — the divergence is confined to these mid widths, where prettier's
pack decision admits one extra word.

The break tsv takes is the fill's own leading `line` — inter-node whitespace that collapses
to one space at compile — so every layout here is render-equivalent.

Prettier is additionally **authoring-dependent** on the second case: from tsv's broken form
it keeps the break (`output_prettier.svelte` matches input there), but from the compact
authoring it packs the word — `prettier_variant_packed.svelte` pins that second
prettier-stable form (tsv normalizes it back to input).
`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier
→ `prettier_variant_packed.svelte`).

## Reason

Print width is a hard limit wherever a render-free break exists; the leading `line` is one.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
