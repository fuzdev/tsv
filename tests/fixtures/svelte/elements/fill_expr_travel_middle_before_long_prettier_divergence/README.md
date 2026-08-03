# fill_expr_travel_middle_before_long_prettier_divergence

Middle text between a traveled expression tag and a second **wide** tag. The first tag is
too wide to pack after `qqqq`, so the text→tag boundary breaks and it starts the
continuation line, collapsing flat (see `fill_spaced_tag_travel_long_prettier_divergence`
for the rule's own boundary cases). The **second** tag takes the same boundary break
mid-run: the whitespace boundary in front of it breaks, it travels whole, and — too wide
even for a fresh line — it breaks internally there (never opening mid-line at the end of
the text line). The second case's shorter tag packs flat in the continuation instead.

Prettier's boundary measurement stops at the first tag's first internal break, so it keeps
that tag on the text line and opens it mid-line — a stable form pinned as
`prettier_variant_midline.svelte`. Prettier also keeps the traveled form, so `input.svelte`
is a fixed point of both formatters and the divergence is one of normalization only.

`unformatted_ours_compact.svelte` is the one-line authoring: tsv → `input.svelte`, prettier
→ the mid-line-open form.

## Reason

Design choice — the wide-element rule's tag analog: a tag that cannot fit flat after the
text starts on a fresh line whole (expression intact) rather than opening mid-line, and the
rule holds for every tag in the run, not just the first (see
`fill_multi_expr_travel_long_prettier_divergence` for the multi-tag rule itself). Same
class as `fill_spaced_tag_travel_long_prettier_divergence`.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
