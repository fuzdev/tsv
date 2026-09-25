# global_glued_both_sides_paragraph_long_prettier_divergence

A global `svelte:*` element glued to content on **both** sides, inside a paragraph. The compiler
hoists the element out of the fragment, so its two neighbours meet directly and the glued unit
(`text2<svelte:window …/>text3`) is one unbreakable word of the paragraph — it flows with the
prose exactly as a glued inline element does.

- **An authored newline beside the unit reflows.** `text3⏎<b>text4</b>` is a single newline
  between inline siblings in a run that holds prose, so it is a spelling of the one rendered
  space and the fill reflows it. Prettier keeps the authored line.
- **A unit that would end its line at 101 travels; one that ends it at exactly 100 stays.** The
  whitespace in front of the unit is the one break available, so tsv moves the whole unit to a
  fresh line. Prettier keeps the unit on the text line: it dangles a `<svelte:head>`'s closing
  `>` onto the following text, and wraps a self-closing element's attributes. At 100 both
  formatters keep the line.

`output_prettier.svelte` is prettier's output from `input.svelte`.
`unformatted_ours_newline.svelte` is the authoring with the newline and every paragraph on one
line: tsv normalizes it to `input.svelte`, prettier to `prettier_variant_newline.svelte`, which
tsv also normalizes to `input.svelte`. All of them render identically.

## Reason

Design choice, render-free under Svelte 5 — the inline element's own rules, reached by a global
element that lays out as one: the sibling-newline flow of
[inline_sibling_newline_flow](../../elements/inline_sibling_newline_flow_prettier_divergence/)
and the welded-run travel of
[inline_welded_run_travel_long](../../elements/inline_welded_run_travel_long_prettier_divergence/).
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
