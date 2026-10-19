# selector_list_long_prettier_divergence

Long selector list with an inline comment. tsv keeps the print width, measuring the list's
last line through the ` {` behind it; Prettier prints a comment-bearing selector verbatim.

tsv: breaks the list at its combinators at 101+ columns, and breaks a pseudo-class's args
when the line ending in ` {` would pass 100 — a 99- or 100-column head line ending `:not(`
stays whole, since only the last line carries the ` {`
Prettier: keeps whatever form is authored — the flat one overruns
(`prettier_variant_flat`), the wrapped one (`input.svelte`) stays wrapped

## Reason

Print width (the comment is incidental; the divergence is the unwrapped over-width line), as
in the sibling [media_long](../media_long_prettier_divergence/). See
[conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors)
for the catalog entry.
