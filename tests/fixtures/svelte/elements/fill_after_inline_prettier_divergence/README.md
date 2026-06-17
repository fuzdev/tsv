# fill_after_inline_prettier_divergence

When text follows an inline element (here a `<Link>` with a long attribute), tsv wraps at the
collapsible whitespace boundaries — keeping the element intact and every line ≤100 — while Prettier
packs the trailing text on, exceeding printWidth (109 chars here). See `output_prettier.svelte`.

tsv: breaks at the whitespace boundaries (element intact, ≤100)
Prettier: packs trailing text onto the line (109 chars, over printWidth)

## Reason

tsv treats printWidth as a hard limit, and the break falls at the collapsible whitespace around the
element rather than splitting its closing `>`. Prettier's fill algorithm doesn't account for the
accumulated width when packing text after inline element wrappers. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
