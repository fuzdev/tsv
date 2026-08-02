# fill_expr_travel_middle_long_prettier_divergence

Middle text between a traveled expression tag and a second tag. The first tag is too wide
to pack after `qqqq`, so the text→tag boundary breaks and the tag starts the continuation
line, collapsing flat (see `fill_spaced_tag_travel_long_prettier_divergence` for the rule's
own boundary cases); the middle text and the second, short tag flow after it in the same
fill.

Prettier's boundary measurement stops at the expression's first internal break, so it keeps
the first tag on the text line and opens it mid-line — a stable form pinned as
`prettier_variant_midline.svelte`. Prettier also keeps the traveled form, so `input.svelte`
is a fixed point of both formatters and the divergence is one of normalization only.

`unformatted_ours_compact.svelte` is the one-line authoring: tsv → `input.svelte`, prettier
→ the mid-line-open form.

## Reason

Design choice — the wide-element rule's tag analog: a tag that cannot fit flat after the
text starts on a fresh line whole (expression intact) rather than opening mid-line. Same
class as `fill_spaced_tag_travel_long_prettier_divergence`.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_expr_travel_middle_before_long_prettier_divergence/` (a second wide tag in
the hard-width run after the traveled one).
