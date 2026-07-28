# root_with_text_prettier_divergence

tsv: an inline sibling isolated by an authored newline flows back onto the content line.
Prettier: keeps the authored newline, so the isolated form is a second stable form.

## Reason

Svelte 5 collapses inter-sibling whitespace to one whitespace, so a newline and a space between
two siblings render identically — the newline's *spelling* carries no signal and the fill
reflows it. Here two root-level components and the text between them join one line. The separator's **presence** still decides layout (a glued boundary is never
split, since breaking there would inject a rendered space); only space↔newline is reshaped.

`prettier_variant_newlines.*` pins the isolated authoring: prettier keeps it stable, tsv
normalizes it to `input`. The element's own multiline-ness is a separate axis and is
preserved — see
[inline_sibling_newline_flow](../../elements/inline_sibling_newline_flow_prettier_divergence/) for the
rule's own fixture, including the control-flow-block, comment, blank-line and block-sibling
controls.

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
