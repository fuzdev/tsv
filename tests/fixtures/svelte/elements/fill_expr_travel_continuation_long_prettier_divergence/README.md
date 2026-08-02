# fill_expr_travel_continuation_long_prettier_divergence

Text continuing after a spaced expression tag that traveled to the continuation line. The
text→tag boundary measures the tag as a whole flat unit (see
`fill_spaced_tag_travel_long_prettier_divergence` for the rule's own boundary cases): too
wide to pack after `qqqq`, the tag starts the continuation line, collapses flat there, and
the following text — including a second, short expression tag — flows greedily after it in
the same fill.

Prettier's boundary measurement stops at the expression's first internal break, so it keeps
the tag on the text line and opens it mid-line (`… qqqq {'rrrr' +` / `'ssss'} tttt …`) — a
stable form pinned as `prettier_variant_midline.svelte`. Prettier also keeps the traveled
form, so `input.svelte` is a fixed point of both formatters and the divergence is one of
normalization only.

## Reason

Design choice — the wide-element rule's tag analog: a tag that cannot fit flat after the
text starts on a fresh line whole (expression intact) rather than opening mid-line. Same
class as `fill_spaced_tag_travel_long_prettier_divergence`.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_expr_travel_boundary_long_prettier_divergence/` (the continuation line's own
100-boundary) and `fill_expr_travel_middle_long_prettier_divergence/` (middle text between
the traveled tag and a second tag).
