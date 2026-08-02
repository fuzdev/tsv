# fill_expr_travel_middle_before_long_prettier_divergence

Middle text between a traveled expression tag and a second **wide** tag. The first tag is
too wide to pack after `qqqq`, so the text→tag boundary breaks and it starts the
continuation line, collapsing flat (see `fill_spaced_tag_travel_long_prettier_divergence`
for the rule's own boundary cases). The **second** tag sits after a text run that follows a
tag — a hard-width context whose separator is a plain space, not a fill `line` (a `line`
there would short-circuit the preceding expression group) — so it has no break opportunity
in front of it: it stays welded to the line and breaks internally, a form **both**
formatters keep. The second case's shorter tag packs flat in the continuation instead.

Prettier's boundary measurement stops at the first tag's first internal break, so it keeps
that tag on the text line and opens it mid-line — a stable form pinned as
`prettier_variant_midline.svelte`. Prettier also keeps the traveled form, so `input.svelte`
is a fixed point of both formatters and the divergence is one of normalization only.

`unformatted_ours_compact.svelte` is the one-line authoring: tsv → `input.svelte`, prettier
→ the mid-line-open form.

## Reason

Design choice — the wide-element rule's tag analog: a tag that cannot fit flat after the
text starts on a fresh line whole (expression intact) rather than opening mid-line. Same
class as `fill_spaced_tag_travel_long_prettier_divergence`.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
