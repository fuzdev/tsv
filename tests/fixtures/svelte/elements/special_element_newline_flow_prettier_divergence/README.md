# special_element_newline_flow_prettier_divergence

The sibling-newline flow rule at a `svelte:*` **special element** — the one sibling kind the
rule's own fixtures never reach. tsv: an inline special flows back onto the content line like a
`<span>`, and a block one owns its line. Prettier: keeps each authoring as its own stable form,
and classifies **every** `svelte:*` as inline.

## Reason

Design choice — the same authoring-independence
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/) argues for.
Svelte 5 collapses inter-sibling whitespace to one whitespace, so a newline and a space between
two siblings render identically; the newline's *spelling* carries no signal and the fill reflows
it.

## What this fixture adds — the special-element arm, both halves

The flow rule asks each neighbour one question — does it flow? — and a special element answers by
the same block/inline classification a plain element does. Neither half of that answer was pinned:
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/)'s cases are an
HTML element, a component and two tags, and every other fixture in the family uses the same three
kinds.

- **inline specials flow** — `<svelte:element>`, `<slot>` and `<svelte:boundary>`, each isolated by
  an authored newline in `prettier_variant_newline.svelte`, all three flowing back onto the content
  line. Prettier keeps that authoring, which is the divergence.
- **the control: a block special does NOT flow.** The four global elements (`<svelte:head>`,
  `<svelte:window>`, `<svelte:body>`, `<svelte:document>`) are block-classified, so each owns its
  line and the run does not flow through it. Without this half, "a special element flows" is
  equally satisfied by a rule that never asks the classification at all. It is identical in
  `input.svelte` and `prettier_variant_newline.svelte` — the one sibling in this fixture whose
  authored lines survive under both formatters.

The block half is **root-only**: Svelte's parser rejects those four inside an element or a block
(`svelte_meta_invalid_placement`), so the control cannot be nested beside the other cases.

`prettier_variant_block_special_spaced.svelte` carries the other side of that classification, and
is a divergence prettier does not share: the control **space**-authored. tsv gives the block
special its own line from either spelling; prettier, which has no `svelte:*` in its block list,
keeps it on the text line. That is the same block-classification tsv states in
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
("a node that owns its own line keeps it") — here pinned from the layout side rather than the
hoist side.

Every boundary tsv collapses is inter-node whitespace that renders as one space either way, so the
output renders identically to the input.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
