# special_element_flow_content_prettier_divergence

Every special element kind — `<svelte:element>` with a static and a dynamic `this`,
`<svelte:component>`, `<svelte:self>` (in a block, where Svelte allows it), `<svelte:boundary>`,
`<svelte:fragment>` (as a component's child) and `<slot>` — outside any whitespace-sensitive
element. Its content is flow content: a whitespace run collapses (`text1   text2` →
`text1 text2`), and content too wide for the line goes **block-style**, both tags whole and the
content on its own indented line. The whitespace-sensitive rule for these elements inside `<pre>`
([elements/pre_special_element_kinds](../../elements/pre_special_element_kinds/)) does not reach
here.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every element on one line; tsv normalizes it to
  `input.svelte`.
- `unformatted_spaces.svelte` — spaced runs and tags; both formatters normalize it to `input.svelte`.
- `prettier_variant_compact.svelte` — prettier's form of the compact authoring: the text stays
  glued to the tags and both delimiters dangle (`<svelte:boundary>` alone goes block-style). tsv
  normalizes it to `input.svelte`.

## Reason

The content boundary's whitespace is render-free here, so it does not select the layout. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
