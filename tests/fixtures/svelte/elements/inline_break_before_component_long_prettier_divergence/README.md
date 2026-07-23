# inline_break_before_component_long_prettier_divergence

Component parity for the break-before rule (see `inline_break_before_wrap_long` for the HTML
inline-element case). An inline component preceded by same-line text starts on a **fresh
line** rather than dangling its opening tag at the end of the text line; on its fresh line it
collapses back inline when it fits, else lays out block-style (both tags intact, content on
its own indented line). Output is byte-identical in shape to the HTML inline-element fixture —
the break-before fold is element-type agnostic.

tsv: opening tag never dangles after a space — the component moves to its own line.
Prettier: keeps the opening tag on the text line and dangles it — see
`prettier_variant_dangle.svelte` (prettier keeps that form; tsv normalizes it to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier → dangle).

## Reason

Design choice, render-free under Svelte 5 (the whitespace boundary before the component
collapses to one space at compile, so the break is render-equivalent).
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
