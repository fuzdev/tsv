# inline_adjacent_component_space_prettier_divergence

The **space** spelling of the adjacent separator before a component, in a run with no prose to reflow
into. tsv keeps the authored space, exactly as it keeps the space between two inline elements: a
component is inline flow content like a `<span>`, and the element twin beside each case lays out
identically. The cases put the space in a prose-free pair, after a comment, after a control-flow block,
at the root, and — as a control — after a block element, whose own line wins for a component exactly as
for an element (the block supplies the break after itself for both).

Prettier's `isInlineElement` admits only a `RegularElement`, so the whitespace-only node before a
component is printed as a plain `line` that breaks with the container: `output_prettier.svelte`
splits every component pair onto its own lines while holding the element pairs — the same document,
two answers keyed on the sibling's kind.

`variant_newline.svelte` is the newline spelling of the same cases. With no prose in the run there is
no fill to reflow into, so the authored newlines are the author's only structure and **both**
formatters hold that form: it is dual-stable, not a normalization claim. The newline-spelled *prose*
runs, which tsv does reflow, are the sibling fixture
[inline_adjacent_component_flow](../inline_adjacent_component_flow_prettier_divergence/).

Every boundary the two formatters disagree on is inter-node whitespace that renders as one space either
way.

## Reason

Design choice: tsv answers a boundary by what stands on each side of it, and a component stands
there as the inline sibling it is; prettier's `isInlineElement` splits a component off a line that fits.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
