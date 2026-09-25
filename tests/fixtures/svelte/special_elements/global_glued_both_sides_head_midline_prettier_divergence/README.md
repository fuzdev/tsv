# global_glued_both_sides_head_midline_prettier_divergence

A `<svelte:head>` whose content the author wrote on its own lines, glued to text on **both**
sides and preceded by more text on its line. The compiler hoists the element, so `text1` and
`text3` meet directly and neither glued boundary may take a break: the element lays out as a
glued inline element does.

- **tsv:** an opening tag never dangles after a space, so the break is spent on the space in
  front of the glued unit — `text1` moves to a fresh line with the element.
- **Prettier:** keeps `text0 text1<svelte:head>` on one line, the opening tag at the line's end.

`output_prettier.svelte` is prettier's output from `input.svelte`: it reflows `text0` back onto
`text1`'s line. `unformatted_ours_midline.svelte` is that authoring, which prettier keeps and tsv
normalizes to `input.svelte`; all of them render identically (the break replaces a space inside the
text).

## Reason

Design choice, render-free under Svelte 5 — the inline element's rule that an opening tag never
dangles after a space, reached by a global element that lays out as one.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
