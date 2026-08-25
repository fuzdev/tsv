# inline_sibling_newline_run_bounded_prettier_divergence

The sibling-newline flow rule's prose count is taken per **run**, and this fixture pins where a
run ends: a comment, a block element, a control-flow block, a `<br />`, and an authored blank
line — in a whitespace-only node between two siblings *or* at either edge of a content text.
Every case puts a two-word run on one side of the boundary and a one-word run on the other: the
prose run flows right up to the boundary, and the label beyond it holds
([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/));
the same nodes with nothing between them are one run, and the label flows with the prose.

## Reason

Design choice. The count is what decides whether there is a fill to reflow into, and a fill
cannot cross a line-owning sibling or a blank line — prose on one side of a comment says nothing
about the label on the other. The blank-line boundary is the same one the fill predicate reads
for an element's interior, and it is a boundary wherever the blank is spelled: the parser puts
a blank between two elements in a whitespace-only node but a blank beside a text in that text's
edge whitespace, and one document must not count two ways by that accident. Bounding is not
sterilizing: the prose run flows up to the boundary in every case, which is what separates "the
boundary ends the run" from "a run near a boundary keeps every line".

Prettier holds every authored newline here, so the held halves are agreement and the flowing
halves are the divergence: `prettier_variant_newline.svelte` is their isolated authoring —
prettier keeps it, tsv normalizes it to `input.svelte`. The unbounded control is the twin that
shows the boundary is what held the label: with nothing between them the nodes are one run, and
the one-word tail flows with the two-word head.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
