# inline_svelte_element_wide_long_prettier_divergence

The special-element counterpart of `inline_component_wide_long`: a wide inline
`<svelte:element>` whose opening tag is too wide to share a line with the preceding text.
tsv drops the whole element to its own line and keeps the preceding word (`and`) hugged on
the text line. Prettier instead hugs the element onto the text line (101+) and breaks its
attributes / dangles the closing `>` (the inline content hug).

tsv: word stays hugged, the whole element moves to its own line (each line ≤100)
Prettier: keeps `and <svelte:element` on the text line and breaks the element internally —
see `prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes back to
`input.svelte`).

This pins that the flow boundary applies to **special elements** (`<svelte:element>`,
`<svelte:component>`, …), not only HTML inline elements (`inline_element_wide_long`) and
components (`inline_component_wide_long`) — the shape is the same across all three.

## Reason

Print width. tsv treats printWidth as a hard limit and keeps the element intact rather than
splitting its attributes/closing `>`, so an over-wide element goes to its own line. The boundary
before the element is a collapsible space, so the word before it stays on the text line.
See [conformance_prettier.md §Svelte: Elements (Wide inline child own-line)](../../../../../docs/conformance_prettier.md#svelte-elements).
