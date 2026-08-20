# column_prettier_divergence

Prettier removes spaces around the column combinator (`||`), which breaks Svelte's CSS parser.

tsv: `col.selected || td` (spaces preserved, Svelte-compatible)
Prettier: `col.selected||td` (spaces removed, Svelte parse error)

## Reason

Parser compat. Svelte's CSS parser requires spaces around `||` — without them it
fails with "Expected a valid CSS identifier". tsv prioritizes Svelte
compatibility. The column combinator is specified in CSS Selectors Level 5 (Level 4
moved it out), where it is still unimplemented by browsers.
See
[conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).
