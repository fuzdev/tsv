# inline_component_fill_long_prettier_divergence

Same fill boundary behavior as `inline_element_fill_long`, but with a component element (`<Comp>`) instead of an HTML inline element (`<a>`). At 101 chars, Prettier keeps everything on one line while tsv breaks at the whitespace before the trailing word — keeping the closing tag intact.

tsv: breaks at the whitespace before the trailing word (closing tag stays intact, ≤100)
Prettier: keeps the trailing word on the line (101, 1 over printWidth) — see `prettier_variant_inline.svelte`

At 100 chars both formatters match.

## Reason

tsv treats printWidth as a hard limit. The break falls at the collapsible whitespace between the
element and the trailing word, so the closing `>` is never split off on its own. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
